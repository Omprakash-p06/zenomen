// Placeholder entry point so `cargo check` can prove the build pipeline in
// isolation. Task 3 of this plan replaces this with the real entry point that
// initialises structured logging.
slint::include_modules!();

fn main() -> Result<(), slint::PlatformError> {
    AppWindow::new()?.run()
}
