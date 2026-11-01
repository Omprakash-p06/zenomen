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

/// Helper to log current RSS at a given execution stage.
pub fn log_benchmark(stage: &str) {
    match read_memory_metrics() {
        Ok(metrics) => {
            tracing::info!(
                stage,
                rss_mb = format!("{:.2}", metrics.rss_mb()),
                pss_mb = format!("{:.2}", metrics.pss_mb()),
                private_dirty_mb = format!("{:.2}", metrics.private_dirty_mb()),
                "memory benchmark"
            );
        }
        Err(err) => {
            tracing::warn!(stage, %err, "could not read memory metrics");
        }
    }
}
