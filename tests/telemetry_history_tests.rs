//! Tests for bounded telemetry history and SVG path generation.

use std::time::Instant;
use zenomen::core::telemetry::{MAX_SAMPLES, TelemetryHistory, TelemetryPoint, generate_svg_path};

#[test]
fn history_strictly_bounds_samples_to_120_on_overflow() {
    let mut history = TelemetryHistory::new();
    assert_eq!(history.sample_count(), 0);

    for i in 0..150 {
        history.push(TelemetryPoint {
            timestamp: Instant::now(),
            cpu: Some(i as f32),
            gpu: Some((i * 2) as f32),
            ram: Some(50.0),
            fan: None,
        });
    }

    assert_eq!(history.sample_count(), MAX_SAMPLES);
    assert_eq!(history.sample_count(), 120);

    // Oldest points (0..30) must have been evicted via pop_front
    let first = history.points().front().expect("history must not be empty");
    assert_eq!(first.cpu, Some(30.0));

    let last = history.points().back().expect("history must not be empty");
    assert_eq!(last.cpu, Some(149.0));
}

#[test]
fn svg_path_empty_for_fewer_than_two_reported_samples() {
    // Zero samples
    assert_eq!(generate_svg_path(&[], 800.0, 140.0), "");

    // One sample
    assert_eq!(generate_svg_path(&[Some(42.0)], 800.0, 140.0), "");

    // One reported sample with None padding
    assert_eq!(
        generate_svg_path(&[None, Some(42.0), None], 800.0, 140.0),
        ""
    );
}

#[test]
fn svg_path_inserts_m_break_commands_across_none_gaps() {
    // 5 samples with a gap in the middle: index 0, 1, (gap at 2), 3, 4
    let samples = vec![Some(10.0), Some(20.0), None, Some(40.0), Some(50.0)];
    let path = generate_svg_path(&samples, 100.0, 100.0);

    assert!(!path.is_empty());
    assert!(path.starts_with('M'), "Path must start with M move command");

    // Count occurrences of 'M' in the path. A contiguous path has 1 'M'; with 1 gap, it must have 2 'M' commands.
    let m_count = path.matches('M').count();
    assert_eq!(
        m_count, 2,
        "Expected exactly 2 M commands due to gap, got: {path}"
    );

    // Should also contain L commands
    assert!(path.contains('L'), "Path must contain L line commands");
}

#[test]
fn generate_chart_paths_produces_all_three_series() {
    let mut history = TelemetryHistory::new();
    for i in 0..10 {
        history.push(TelemetryPoint {
            timestamp: Instant::now(),
            cpu: Some(20.0 + i as f32),
            gpu: Some(30.0 + i as f32),
            ram: Some(40.0 + i as f32),
            fan: None,
        });
    }

    let (cpu_path, gpu_path, ram_path) = history.generate_chart_paths(800.0, 140.0);
    assert!(!cpu_path.is_empty(), "CPU path must be non-empty");
    assert!(!gpu_path.is_empty(), "GPU path must be non-empty");
    assert!(!ram_path.is_empty(), "RAM path must be non-empty");

    let (cpu_sp, gpu_sp, ram_sp) = history.generate_sparkline_paths();
    assert!(!cpu_sp.is_empty(), "CPU sparkline must be non-empty");
    assert!(!gpu_sp.is_empty(), "GPU sparkline must be non-empty");
    assert!(!ram_sp.is_empty(), "RAM sparkline must be non-empty");
}

#[test]
fn test_history_capacity_adjusts_dynamically_to_preserve_time_window() {
    let mut history = TelemetryHistory::new_with_duration(120, 1000);
    assert_eq!(history.capacity(), 120);
    assert_eq!(history.duration_seconds(), 120);

    // Polling rate increases to 500ms -> capacity doubles to 240 samples to maintain 120s
    history.update_interval(500);
    assert_eq!(history.capacity(), 240);
    assert_eq!(history.duration_seconds(), 120);

    // Polling rate drops to 2000ms -> capacity halves to 60 samples to maintain 120s
    history.update_interval(2000);
    assert_eq!(history.capacity(), 60);
    assert_eq!(history.duration_seconds(), 120);

    // Change duration to 300s at 2000ms -> 150 samples
    history.set_duration(300, 2000);
    assert_eq!(history.capacity(), 150);
    assert_eq!(history.duration_seconds(), 300);
}
