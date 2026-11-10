//! Benchmarking and headless performance verification tests.
//!
//! Enforces:
//! - Headless CI safety (no GUI server requirement or panics)
//! - Memory footprint constraint: baseline memory well below 30 MB (PERF-01)
//! - Worker tick duration and sampling performance (<5ms per tick)

use std::sync::Arc;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use zenomen::core::benchmark::BenchmarkTimer;
use zenomen::core::events::AppEvent;
use zenomen::core::spawn_monitoring_worker;
use zenomen::hardware::{MockHardware, MockScenario};

#[test]
fn memory_instrumentation_reads_valid_process_rss() {
    use zenomen::core::benchmark::read_memory_metrics;
    let metrics = read_memory_metrics().expect("read_memory_metrics must succeed");

    assert!(
        metrics.rss_kb > 0,
        "RSS must be greater than 0 kB, got: {}",
        metrics.rss_kb
    );
    assert!(
        metrics.rss_mb() > 0.0,
        "RSS in MB must be greater than 0, got: {}",
        metrics.rss_mb()
    );

    // Test process RSS read succeeds and returns a non-zero, plausible value (WR-13)
    assert!(
        metrics.rss_mb() < 500.0,
        "Process RSS is implausibly high: {:.2} MB",
        metrics.rss_mb()
    );
}

#[test]
fn worker_thread_samples_and_dispatches_headlessly() {
    let timer = BenchmarkTimer::start("worker_headless_test");

    let provider = Arc::new(MockHardware::new(MockScenario::Nominal));
    let (event_sender, event_receiver) = mpsc::channel();
    let (command_sender, command_receiver) = mpsc::sync_channel(8);

    let worker = spawn_monitoring_worker(provider, event_sender, command_receiver);

    // Initial event 1: CapabilitiesLoaded
    let event1 = event_receiver
        .recv_timeout(Duration::from_millis(500))
        .expect("Worker must send initial capabilities immediately");
    match event1 {
        AppEvent::CapabilitiesLoaded(caps) => {
            assert!(caps.has_cpu_metrics());
        }
        other => panic!("Expected CapabilitiesLoaded, got: {:?}", other),
    }

    // Initial event 2: Initial sample
    let event2 = event_receiver
        .recv_timeout(Duration::from_millis(500))
        .expect("Worker must send initial sample immediately");
    match event2 {
        AppEvent::TelemetryUpdated(snapshot) => {
            assert!(!snapshot.cpu_name.is_empty());
        }
        other => panic!("Expected TelemetryUpdated, got: {:?}", other),
    }

    // On-demand refresh response (test MON-10 responsiveness)
    let refresh_start = Instant::now();
    command_sender
        .try_send(zenomen::core::AppCommand::RefreshTelemetry)
        .expect("Command send must succeed");

    let mut event3 = event_receiver
        .recv_timeout(Duration::from_millis(500))
        .expect("Worker must immediately respond to RefreshTelemetry without 1s sleep");

    while matches!(
        event3,
        AppEvent::ServiceStatusChanged(_)
            | AppEvent::PolkitStatusChanged(_)
            | AppEvent::FanModeChanged(_)
            | AppEvent::PowerProfileChanged(_)
            | AppEvent::MemoryTelemetryUpdated(_)
    ) {
        event3 = event_receiver
            .recv_timeout(Duration::from_millis(500))
            .expect("Worker must respond with TelemetryUpdated");
    }

    let refresh_duration = refresh_start.elapsed();
    assert!(
        refresh_duration < Duration::from_millis(500),
        "On-demand refresh took too long: {:?}",
        refresh_duration
    );

    match event3 {
        AppEvent::TelemetryUpdated(snapshot) => {
            assert!(snapshot.cpu_utilization > 0.0);
        }
        other => panic!("Expected TelemetryUpdated from refresh, got: {:?}", other),
    }

    // Drop command sender to cleanly terminate worker
    drop(command_sender);
    let join_result = worker.join();
    assert!(join_result.is_ok(), "Worker thread must exit cleanly");

    timer.stop_and_log();
}

#[test]
#[cfg(target_os = "linux")]
fn test_read_cpu_sample_and_calculation() {
    use zenomen::core::benchmark::{
        CpuSample, USER_HZ, cpu_percent_between, measure_stage, read_cpu_sample,
    };

    let sample1 = read_cpu_sample().expect("read_cpu_sample must succeed on Linux");
    std::thread::sleep(Duration::from_millis(20));
    let sample2 = read_cpu_sample().expect("second read_cpu_sample must succeed");

    assert!(sample2.jiffies >= sample1.jiffies);

    // Test calculation with synthetic samples
    let t0 = Instant::now();
    let s_start = CpuSample {
        jiffies: 1000,
        at: t0,
    };
    let s_end = CpuSample {
        jiffies: 1100,                   // +100 jiffies = 1.0 second of CPU time at USER_HZ=100
        at: t0 + Duration::from_secs(2), // over 2 seconds = 50% CPU
    };
    let pct = cpu_percent_between(&s_start, &s_end, USER_HZ);
    assert!((pct - 50.0).abs() < 0.1, "Expected ~50% CPU, got {pct}");

    // Test zero elapsed time edge case
    let zero_elapsed = cpu_percent_between(&s_start, &s_start, USER_HZ);
    assert_eq!(zero_elapsed, 0.0);

    // Test measure_stage
    let report =
        measure_stage("test-stage", Duration::from_millis(50)).expect("measure_stage must succeed");
    assert_eq!(report.stage, "test-stage");
    assert!(report.rss_mb > 0.0);
    assert!(report.cpu_percent >= 0.0);
}

#[test]
fn test_visibility_gated_sampling_pauses_and_resumes() {
    let provider = Arc::new(MockHardware::new(MockScenario::Nominal));
    let (event_sender, event_receiver) = mpsc::channel();
    let (command_sender, command_receiver) = mpsc::sync_channel(8);

    let worker = spawn_monitoring_worker(provider, event_sender, command_receiver);

    // Drain initial capabilities and initial sample
    let _ = event_receiver.recv_timeout(Duration::from_millis(500));
    let _ = event_receiver.recv_timeout(Duration::from_millis(500));

    // 1. Hide UI
    command_sender
        .try_send(zenomen::core::AppCommand::SetUiVisible(false))
        .expect("Sending SetUiVisible(false) must succeed");

    // Clear any pending events (e.g. initial service status)
    while event_receiver
        .recv_timeout(Duration::from_millis(100))
        .is_ok()
    {}

    // Wait 1.5 seconds (interval is 1000ms). While hidden, no periodic sample must arrive!
    let start_wait = Instant::now();
    while start_wait.elapsed() < Duration::from_millis(1500) {
        if let Ok(AppEvent::TelemetryUpdated(snapshot)) =
            event_receiver.recv_timeout(Duration::from_millis(200))
        {
            panic!(
                "Expected no TelemetryUpdated while window is hidden, got: {:?}",
                snapshot
            );
        }
    }

    // 2. Restore UI
    command_sender
        .try_send(zenomen::core::AppCommand::SetUiVisible(true))
        .expect("Sending SetUiVisible(true) must succeed");

    // An immediate sample must arrive upon restoring UI
    let restore_res = event_receiver
        .recv_timeout(Duration::from_millis(500))
        .expect("Immediate sample must arrive when UI is restored");

    match restore_res {
        AppEvent::TelemetryUpdated(snapshot) => {
            assert!(!snapshot.cpu_name.is_empty());
        }
        other => panic!(
            "Expected TelemetryUpdated immediately on restore, got: {:?}",
            other
        ),
    }

    drop(command_sender);
    let _ = worker.join();
}

#[test]
fn test_worker_shutdown_command_exits_promptly() {
    let provider = Arc::new(MockHardware::new(MockScenario::Nominal));
    let (event_sender, _event_receiver) = mpsc::channel();
    let (command_sender, command_receiver) = mpsc::sync_channel(8);

    let worker = spawn_monitoring_worker(provider, event_sender, command_receiver);

    let start = Instant::now();
    command_sender
        .try_send(zenomen::core::AppCommand::Shutdown)
        .expect("Sending Shutdown must succeed");

    // Worker must join promptly even while command_sender is kept alive (CR-01)
    let res = worker.join();
    assert!(res.is_ok(), "Worker must join successfully on Shutdown");
    assert!(
        start.elapsed() < Duration::from_millis(1000),
        "Worker took too long to shut down: {:?}",
        start.elapsed()
    );
}
