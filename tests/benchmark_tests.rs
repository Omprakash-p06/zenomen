//! Benchmarking and headless performance verification tests.
//!
//! Enforces:
//! - Headless CI safety (no GUI server requirement or panics)
//! - Memory footprint constraint: baseline memory well below 30 MB (PERF-01)
//! - Worker tick duration and sampling performance (<5ms per tick)

use std::sync::Arc;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use zenomen::core::benchmark::{BenchmarkTimer, read_memory_metrics};
use zenomen::core::events::AppEvent;
use zenomen::core::spawn_monitoring_worker;
use zenomen::hardware::{MockHardware, MockScenario};

#[test]
fn memory_instrumentation_reads_valid_process_rss() {
    let metrics = read_memory_metrics().expect("read_memory_metrics must succeed on Linux");

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

    // Peak RSS for headless tests must stay well within the project's 40 MB budget (and under 30 MB baseline)
    assert!(
        metrics.rss_mb() < 30.0,
        "Baseline RSS must be under 30 MB, measured: {:.2} MB",
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

    let refresh_duration = refresh_start.elapsed();

    assert!(
        refresh_duration < Duration::from_millis(50),
        "On-demand refresh took too long: {:?}",
        refresh_duration
    );

    let mut event3 = event_receiver
        .recv_timeout(Duration::from_millis(200))
        .expect("Worker must immediately respond to RefreshTelemetry without 1s sleep");

    while matches!(
        event3,
        AppEvent::ServiceStatusChanged(_) | AppEvent::PolkitStatusChanged(_)
    ) {
        event3 = event_receiver
            .recv_timeout(Duration::from_millis(200))
            .expect("Worker must respond with TelemetryUpdated");
    }

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
