//! Linux `/proc/meminfo` parser implementing accurate memory telemetry.
//!
//! Complies with MON-07 by calculating `used_mb = MemTotal - MemAvailable`,
//! avoiding the naive `MemFree` calculation that excludes reclaimable page cache
//! and buffer memory.

/// System memory metrics in mebibytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MemorySnapshot {
    pub total_mb: u64,
    pub available_mb: u64,
    pub used_mb: u64,
}

impl MemorySnapshot {
    /// Calculate utilization percentage (0.0..=100.0).
    pub fn utilization_pct(&self) -> f32 {
        if self.total_mb == 0 {
            0.0
        } else {
            ((self.used_mb as f32 / self.total_mb as f32) * 100.0).clamp(0.0, 100.0)
        }
    }
}

/// Parse `/proc/meminfo` contents into a [`MemorySnapshot`].
///
/// Returns `None` if either `MemTotal` or `MemAvailable` is absent, preventing
/// synthesized zeros (MON-08).
pub fn parse_proc_meminfo(content: &str) -> Option<MemorySnapshot> {
    let mut mem_total_kb: Option<u64> = None;
    let mut mem_avail_kb: Option<u64> = None;

    for line in content.lines() {
        if line.starts_with("MemTotal:") {
            mem_total_kb = parse_meminfo_kb_value(line);
        } else if line.starts_with("MemAvailable:") {
            mem_avail_kb = parse_meminfo_kb_value(line);
        }

        if mem_total_kb.is_some() && mem_avail_kb.is_some() {
            break;
        }
    }

    let total_kb = mem_total_kb?;
    let avail_kb = mem_avail_kb?;

    let total_mb = total_kb / 1024;
    let available_mb = avail_kb / 1024;
    let used_mb = total_mb.saturating_sub(available_mb);

    Some(MemorySnapshot {
        total_mb,
        available_mb,
        used_mb,
    })
}

/// Extract numeric kB value from a line like `MemTotal:       23915156 kB`.
fn parse_meminfo_kb_value(line: &str) -> Option<u64> {
    let (_, val_part) = line.split_once(':')?;
    let num_str = val_part.split_whitespace().next()?;
    num_str.parse().ok()
}

/// Read and parse `/proc/meminfo` from the filesystem.
pub fn read_memory_snapshot() -> Result<MemorySnapshot, String> {
    let content = std::fs::read_to_string("/proc/meminfo")
        .map_err(|e| format!("failed to read /proc/meminfo: {e}"))?;
    parse_proc_meminfo(&content)
        .ok_or_else(|| "MemTotal or MemAvailable not found in /proc/meminfo".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_meminfo_computes_used_from_memavailable() {
        let sample = "\
MemTotal:       16082444 kB
MemFree:         1000000 kB
MemAvailable:    8942120 kB
Buffers:          120944 kB
Cached:          5000000 kB
";
        let snapshot = parse_proc_meminfo(sample).expect("must parse");
        assert_eq!(snapshot.total_mb, 16082444 / 1024); // 15705 MB
        assert_eq!(snapshot.available_mb, 8942120 / 1024); // 8732 MB
        assert_eq!(snapshot.used_mb, (16082444 / 1024) - (8942120 / 1024)); // 6973 MB

        // Verify that MemFree (1000000 kB -> ~976 MB) was NOT used
        assert_ne!(snapshot.available_mb, 1000000 / 1024);
    }

    #[test]
    fn parse_meminfo_returns_none_if_keys_missing() {
        let sample = "Buffers: 120944 kB\nCached: 5000000 kB\n";
        assert!(parse_proc_meminfo(sample).is_none());
    }
}
