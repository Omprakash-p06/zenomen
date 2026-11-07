//! Tests for the process-wide logging setup.
//!
//! One test only: installing a subscriber is process-wide state, so a second
//! parallel test would race on `XDG_STATE_HOME` and on the global dispatcher.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

struct TestDir(PathBuf);

impl TestDir {
    fn new(prefix: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("{prefix}-{nanos}"));
        fs::create_dir_all(&path).expect("create test dir");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn log_files(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let logs = root.join("zenomen").join("logs");
    let Ok(entries) = fs::read_dir(&logs) else {
        return found;
    };
    for entry in entries.flatten() {
        if entry.path().extension().is_some_and(|ext| ext == "log") {
            found.push(entry.path());
        }
    }
    found
}

#[test]
fn a_log_line_reaches_the_state_directory_and_double_init_is_safe() {
    let dir = TestDir::new("zenomen-logging");
    // `directories` reads XDG_STATE_HOME on Linux; every other test in this
    // binary is in this function, so the mutation cannot race anything.
    // SAFETY: no other thread in this binary reads the environment concurrently.
    unsafe {
        std::env::set_var("XDG_STATE_HOME", dir.path());
    }

    let guard = zenomen::logging::init();
    tracing::info!(
        marker = "logging-smoke-test",
        "log line from the test suite"
    );

    // A second install must degrade rather than panic on the global dispatcher.
    let second = zenomen::logging::init();

    // Dropping the guard flushes the non-blocking writer.
    drop(guard);
    drop(second);

    let deadline = Instant::now() + Duration::from_secs(5);
    let mut contents = String::new();
    while Instant::now() < deadline {
        let files = log_files(dir.path());
        if let Some(path) = files.first()
            && let Ok(text) = fs::read_to_string(path)
            && text.contains("logging-smoke-test")
        {
            contents = text;
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }

    assert!(
        !contents.is_empty(),
        "expected a log file under {}/zenomen/logs containing the marker; found {:?}",
        dir.path().display(),
        log_files(dir.path())
    );
}
