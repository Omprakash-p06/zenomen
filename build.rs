// Compiles the Slint UI definition into Rust at build time.
//
// The style is pinned here rather than left to the ambient SLINT_STYLE
// environment variable so that a developer machine and CI render the same
// widgets. Phase 1 draws no std-widgets yet; later phases depend on this being
// deterministic.
fn main() {
    // `slint-build` is a build-dependency, so it is present even when the `ui`
    // feature is off. Skip the compiler in that case: the daemon must not fail
    // to build because of a `.slint` typo it never renders.
    if std::env::var_os("CARGO_FEATURE_UI").is_some() {
        println!("cargo:rerun-if-changed=ui/");
        let config = slint_build::CompilerConfiguration::new().with_style("fluent".into());
        slint_build::compile_with_config("ui/app.slint", config)
            .expect("failed to compile ui/app.slint");
    }

    #[cfg(windows)]
    {
        let mut res = winres::WindowsResource::new();
        res.set_icon("data/icons/zenomen.ico");
        if let Err(e) = res.compile() {
            println!("cargo:warning=Failed to compile Windows resources: {}", e);
        }
    }
}
