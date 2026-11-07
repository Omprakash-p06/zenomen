//! Performance and memory benchmarking instrumentation.
//!
//! Tracks memory consumption (RSS, PSS, Private Dirty) to uphold the <=40 MB peak
//! RSS constraint (`PERF-01`, `PERF-02`).
//!
//! Reads `/proc/self/smaps_rollup` with fallback to `/proc/self/status`.

use std::fs;
use std::io::{self, BufRead, BufReader};
use std::time::{Duration, Instant};

/// Memory consumption metrics measured from the OS.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MemoryMetrics {
    /// Resident Set Size in kilobytes.
    pub rss_kb: u64,
    /// Proportional Set Size in kilobytes.
    pub pss_kb: u64,
    /// Private Dirty memory in kilobytes.
    pub private_dirty_kb: u64,
}

impl MemoryMetrics {
    /// Resident Set Size in megabytes (as f64).
    pub fn rss_mb(&self) -> f64 {
        self.rss_kb as f64 / 1024.0
    }

    /// Proportional Set Size in megabytes (as f64).
    pub fn pss_mb(&self) -> f64 {
        self.pss_kb as f64 / 1024.0
    }

    /// Private dirty memory in megabytes (as f64).
    pub fn private_dirty_mb(&self) -> f64 {
        self.private_dirty_kb as f64 / 1024.0
    }
}

/// Read memory consumption for the current process.
///
/// Attempts `/proc/self/smaps_rollup` first; falls back to `/proc/self/status`
/// if `smaps_rollup` is unavailable.
pub fn read_memory_metrics() -> Result<MemoryMetrics, io::Error> {
    if let Ok(metrics) = read_from_smaps_rollup() {
        return Ok(metrics);
    }
    read_from_status()
}

fn parse_kb(rest: &str) -> Option<u64> {
    rest.split_whitespace()
        .next()
        .and_then(|val| val.parse::<u64>().ok())
}

fn read_from_smaps_rollup() -> Result<MemoryMetrics, io::Error> {
    let file = fs::File::open("/proc/self/smaps_rollup")?;
    let reader = BufReader::new(file);

    let mut metrics = MemoryMetrics::default();
    let mut found_any = false;

    for line in reader.lines() {
        let line = line?;
        if let Some(kb) = line.strip_prefix("Rss:").and_then(parse_kb) {
            metrics.rss_kb = kb;
            found_any = true;
        } else if let Some(kb) = line.strip_prefix("Pss:").and_then(parse_kb) {
            metrics.pss_kb = kb;
        } else if let Some(kb) = line.strip_prefix("Private_Dirty:").and_then(parse_kb) {
            metrics.private_dirty_kb = kb;
        }
    }

    if found_any {
        Ok(metrics)
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "smaps_rollup contained no Rss field",
        ))
    }
}

fn read_from_status() -> Result<MemoryMetrics, io::Error> {
    let file = fs::File::open("/proc/self/status")?;
    let reader = BufReader::new(file);

    let mut metrics = MemoryMetrics::default();
    let mut found = false;

    for line in reader.lines() {
        let line = line?;
        if let Some(kb) = line.strip_prefix("VmRSS:").and_then(parse_kb) {
            metrics.rss_kb = kb;
            found = true;
            break;
        }
    }

    if found {
        Ok(metrics)
    } else {
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            "VmRSS field not found in /proc/self/status",
        ))
    }
}

/// Measures execution duration of an operation or tick.
#[derive(Debug)]
pub struct BenchmarkTimer {
    start: Instant,
    label: &'static str,
}

impl BenchmarkTimer {
    pub fn start(label: &'static str) -> Self {
        Self {
            start: Instant::now(),
            label,
        }
    }

    pub fn elapsed(&self) -> Duration {
        self.start.elapsed()
    }

    pub fn stop_and_log(&self) -> Duration {
        let elapsed = self.elapsed();
        tracing::debug!(
            label = self.label,
            elapsed_us = elapsed.as_micros(),
            "benchmark timer"
        );
        elapsed
    }
}

/// Standard Linux USER_HZ default (100 ticks per second).
pub const USER_HZ: u64 = 100;

/// Dynamically query system clock ticks per second via sysconf(_SC_CLK_TCK) on Linux.
pub fn get_clock_ticks_per_second() -> u64 {
    #[cfg(target_os = "linux")]
    {
        let ticks = unsafe { libc::sysconf(libc::_SC_CLK_TCK) };
        if ticks > 0 {
            return ticks as u64;
        }
    }
    USER_HZ
}

/// CPU timing sample taken from `/proc/self/stat`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CpuSample {
    /// Combined user and system CPU ticks (utime + stime).
    pub jiffies: u64,
    /// Exact moment the sample was taken.
    pub at: Instant,
}

/// Read CPU ticks for the current process from `/proc/self/stat`.
///
/// Handles process names (`comm`) that contain spaces and parentheses by
/// splitting after the closing parenthesis `)`.
pub fn read_cpu_sample() -> Result<CpuSample, io::Error> {
    let stat = fs::read_to_string("/proc/self/stat")?;
    let rparen_idx = stat.rfind(')').ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "Malformed /proc/self/stat: closing parenthesis missing",
        )
    })?;

    // Fields after ')' start from field 3 (state)
    let rest = &stat[rparen_idx + 1..];
    let fields: Vec<&str> = rest.split_whitespace().collect();

    // utime is field 14 (index 11 in post-comm fields)
    // stime is field 15 (index 12 in post-comm fields)
    if fields.len() < 13 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "Malformed /proc/self/stat: expected >= 13 post-comm fields, found {}",
                fields.len()
            ),
        ));
    }

    let utime: u64 = fields[11].parse().map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Invalid utime field: {e}"),
        )
    })?;
    let stime: u64 = fields[12].parse().map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Invalid stime field: {e}"),
        )
    })?;

    Ok(CpuSample {
        jiffies: utime + stime,
        at: Instant::now(),
    })
}

/// Calculate average CPU utilization percentage between two samples.
pub fn cpu_percent_between(start: &CpuSample, end: &CpuSample, clock_ticks: u64) -> f64 {
    let elapsed = end.at.duration_since(start.at).as_secs_f64();
    if elapsed <= 0.0 || clock_ticks == 0 {
        return 0.0;
    }

    let delta_jiffies = end.jiffies.saturating_sub(start.jiffies);
    let cpu_seconds = delta_jiffies as f64 / clock_ticks as f64;
    let percent = (cpu_seconds / elapsed) * 100.0;
    percent.max(0.0)
}

/// Combined performance stage report.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StageReport {
    pub stage: &'static str,
    pub rss_mb: f64,
    pub cpu_percent: f64,
}

/// Measure CPU and RSS over a specified time window.
///
/// NOTE: Blocks for `window` duration. MUST NEVER be called from Slint UI thread.
pub fn measure_stage(stage: &'static str, window: Duration) -> Result<StageReport, io::Error> {
    let cpu_start = read_cpu_sample()?;
    std::thread::sleep(window);
    let cpu_end = read_cpu_sample()?;

    let memory = read_memory_metrics()?;
    let cpu_percent = cpu_percent_between(&cpu_start, &cpu_end, get_clock_ticks_per_second());

    Ok(StageReport {
        stage,
        rss_mb: memory.rss_mb(),
        cpu_percent,
    })
}

/// Helper to log current RSS at a given execution stage.
pub fn log_benchmark(stage: &str) {
    match read_memory_metrics() {
        Ok(metrics) => {
            let is_benchmark_env = std::env::var("ZENOMEN_BENCHMARK").as_deref() == Ok("1");
            if is_benchmark_env {
                println!(
                    "zenomen-benchmark stage={stage} rss_mb={:.2}",
                    metrics.rss_mb()
                );
                tracing::info!(
                    stage,
                    rss_mb = format!("{:.2}", metrics.rss_mb()),
                    pss_mb = format!("{:.2}", metrics.pss_mb()),
                    private_dirty_mb = format!("{:.2}", metrics.private_dirty_mb()),
                    "memory benchmark"
                );
            } else {
                tracing::debug!(
                    stage,
                    rss_mb = format!("{:.2}", metrics.rss_mb()),
                    pss_mb = format!("{:.2}", metrics.pss_mb()),
                    private_dirty_mb = format!("{:.2}", metrics.private_dirty_mb()),
                    "memory benchmark"
                );
            }
        }
        Err(err) => {
            tracing::warn!(stage, %err, "could not read memory metrics");
        }
    }
}
