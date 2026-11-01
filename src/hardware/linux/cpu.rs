//! Linux procfs and sysfs CPU telemetry parser.
//!
//! Calculates aggregate and per-core CPU utilization via tick-to-tick jiffies
//! deltas without thread sleeps (MON-01, MON-02), reads current CPU frequency
//! from cpufreq or cpuinfo (MON-03), and extracts the processor model string.

use std::path::Path;

/// Raw jiffies read from one line of `/proc/stat`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CpuJiffies {
    pub user: u64,
    pub nice: u64,
    pub system: u64,
    pub idle: u64,
    pub iowait: u64,
    pub irq: u64,
    pub softirq: u64,
    pub steal: u64,
    pub guest: u64,
    pub guest_nice: u64,
}

impl CpuJiffies {
    /// Parse a single line from `/proc/stat` (e.g. `cpu ...` or `cpu0 ...`).
    ///
    /// Returns `Some((identifier, jiffies))` on success.
    pub fn parse_stat_line(line: &str) -> Option<(String, Self)> {
        let mut parts = line.split_whitespace();
        let name = parts.next()?;
        if !name.starts_with("cpu") {
            return None;
        }

        let user = parts.next()?.parse().ok()?;
        let nice = parts.next()?.parse().ok()?;
        let system = parts.next()?.parse().ok()?;
        let idle = parts.next()?.parse().ok()?;
        let iowait = parts.next().and_then(|v| v.parse().ok()).unwrap_or(0);
        let irq = parts.next().and_then(|v| v.parse().ok()).unwrap_or(0);
        let softirq = parts.next().and_then(|v| v.parse().ok()).unwrap_or(0);
        let steal = parts.next().and_then(|v| v.parse().ok()).unwrap_or(0);
        let guest = parts.next().and_then(|v| v.parse().ok()).unwrap_or(0);
        let guest_nice = parts.next().and_then(|v| v.parse().ok()).unwrap_or(0);

        Some((
            name.to_string(),
            Self {
                user,
                nice,
                system,
                idle,
                iowait,
                irq,
                softirq,
                steal,
                guest,
                guest_nice,
            },
        ))
    }

    /// Sum of idle-related jiffies (`idle + iowait`).
    pub fn idle(&self) -> u64 {
        self.idle.saturating_add(self.iowait)
    }

    /// Total sum of all CPU jiffies.
    pub fn total(&self) -> u64 {
        let system_all = self
            .system
            .saturating_add(self.irq)
            .saturating_add(self.softirq);
        let virtual_all = self.guest.saturating_add(self.guest_nice);
        let busy = self
            .user
            .saturating_add(self.nice)
            .saturating_add(system_all)
            .saturating_add(self.steal)
            .saturating_add(virtual_all);

        busy.saturating_add(self.idle())
    }

    /// Calculate percentage utilization between two consecutive samples.
    ///
    /// Returns a value clamped to `0.0..=100.0`.
    pub fn delta_utilization(prev: &Self, curr: &Self) -> f32 {
        let delta_total = curr.total().saturating_sub(prev.total());
        let delta_idle = curr.idle().saturating_sub(prev.idle());

        if delta_total == 0 {
            return 0.0;
        }

        let idle_ratio = (delta_idle as f32 / delta_total as f32).clamp(0.0, 1.0);
        ((1.0 - idle_ratio) * 100.0).clamp(0.0, 100.0)
    }
}

/// Stateful CPU sampler retaining previous tick jiffies for delta calculation.
///
/// Operates without blocking sleeps across worker loop ticks (MON-01, MON-02).
#[derive(Debug, Clone, Default)]
pub struct CpuSampler {
    prev_aggregate: Option<CpuJiffies>,
    prev_cores: Vec<(String, CpuJiffies)>,
}

impl CpuSampler {
    pub fn new() -> Self {
        Self::default()
    }

    /// Sample `/proc/stat` contents and return `(aggregate_utilization, per_core_utilizations)`.
    ///
    /// On the very first tick (no previous baseline), returns `0.0` aggregate and empty or
    /// zero-utilization per core, while recording the baseline for tick 2.
    pub fn sample_proc_stat(&mut self, stat_content: &str) -> (f32, Vec<f32>) {
        let mut new_aggregate = None;
        let mut new_cores = Vec::new();

        for line in stat_content.lines() {
            if let Some((name, jiffies)) = CpuJiffies::parse_stat_line(line) {
                if name == "cpu" {
                    new_aggregate = Some(jiffies);
                } else if name.starts_with("cpu") {
                    new_cores.push((name, jiffies));
                }
            }
        }

        let aggregate_pct = match (&self.prev_aggregate, &new_aggregate) {
            (Some(prev), Some(curr)) => CpuJiffies::delta_utilization(prev, curr),
            _ => 0.0,
        };

        let mut per_core_pct = Vec::with_capacity(new_cores.len());
        for (name, curr_jiffies) in &new_cores {
            let core_util = self
                .prev_cores
                .iter()
                .find(|(prev_name, _)| prev_name == name)
                .map(|(_, prev_jiffies)| CpuJiffies::delta_utilization(prev_jiffies, curr_jiffies))
                .unwrap_or(0.0);
            per_core_pct.push(core_util);
        }

        self.prev_aggregate = new_aggregate;
        self.prev_cores = new_cores;

        (aggregate_pct, per_core_pct)
    }
}

/// Read the CPU model string from `/proc/cpuinfo` (e.g. `model name: AMD Ryzen 5 5600H`).
pub fn read_cpu_model(cpuinfo_content: &str) -> Option<String> {
    cpuinfo_content.lines().find_map(|line| {
        let (key, val) = line.split_once(':')?;
        if key.trim() == "model name" {
            let trimmed = val.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
        None
    })
}

/// Read CPU frequency in MHz from cpufreq `scaling_cur_freq` file (path in kHz).
pub fn read_cpu_freq_mhz(sysfs_path: &Path) -> Option<u32> {
    let content = std::fs::read_to_string(sysfs_path).ok()?;
    let khz: u64 = content.trim().parse().ok()?;
    Some((khz / 1000) as u32)
}

/// Fallback to read CPU frequency in MHz from `/proc/cpuinfo` `cpu MHz` line.
pub fn read_cpu_freq_from_cpuinfo(cpuinfo_content: &str) -> Option<u32> {
    cpuinfo_content.lines().find_map(|line| {
        let (key, val) = line.split_once(':')?;
        if key.trim() == "cpu MHz" {
            val.trim().parse::<f32>().ok().map(|mhz| mhz.round() as u32)
        } else {
            None
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_stat_line_correctly_extracts_jiffies() {
        let line = "cpu  1150062 8 366422 38566522 20825 0 19743 0 0 0";
        let (name, jiffies) = CpuJiffies::parse_stat_line(line).expect("must parse");
        assert_eq!(name, "cpu");
        assert_eq!(jiffies.user, 1150062);
        assert_eq!(jiffies.nice, 8);
        assert_eq!(jiffies.system, 366422);
        assert_eq!(jiffies.idle, 38566522);
        assert_eq!(jiffies.iowait, 20825);
        assert_eq!(jiffies.irq, 0);
        assert_eq!(jiffies.softirq, 19743);
    }

    #[test]
    fn parse_stat_line_skips_non_cpu_lines() {
        assert!(CpuJiffies::parse_stat_line("intr 184248579 144").is_none());
        assert!(CpuJiffies::parse_stat_line("btime 1793511178").is_none());
    }

    #[test]
    fn delta_utilization_calculates_correct_percentage() {
        let sample1 = CpuJiffies {
            user: 100,
            nice: 0,
            system: 50,
            idle: 850,
            ..Default::default()
        }; // total = 1000, idle = 850

        let sample2 = CpuJiffies {
            user: 200,
            nice: 0,
            system: 100,
            idle: 900,
            ..Default::default()
        }; // total = 1200, idle = 900. delta_total = 200, delta_idle = 50 -> busy = 150/200 = 75%

        let util = CpuJiffies::delta_utilization(&sample1, &sample2);
        assert!((util - 75.0).abs() < 0.01, "Expected 75.0%, got {util}");
    }

    #[test]
    fn read_cpu_model_extracts_name() {
        let cpuinfo =
            "processor\t: 0\nmodel name\t: AMD Ryzen 5 5600H with Radeon Graphics\nstepping\t: 0\n";
        let model = read_cpu_model(cpuinfo).expect("must extract model name");
        assert_eq!(model, "AMD Ryzen 5 5600H with Radeon Graphics");
    }

    #[test]
    fn read_cpu_freq_from_cpuinfo_extracts_mhz() {
        let cpuinfo = "processor\t: 0\ncpu MHz\t\t: 4066.717\n";
        let freq = read_cpu_freq_from_cpuinfo(cpuinfo).expect("must extract freq");
        assert_eq!(freq, 4067);
    }
}
