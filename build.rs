// Compiles the Slint UI definition into Rust at build time, and sets Windows resource metadata.
//
// The style is pinned here rather than left to the ambient SLINT_STYLE
// environment variable so that a developer machine and CI render the same
// widgets.
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
        res.set("ProductName", "ZenOmen");
        res.set("FileDescription", "ZenOmen OMEN/Victus Control Center");
        res.set("LegalCopyright", "Copyright (c) 2026 ZenOmen Project");
        res.set("OriginalFilename", "zenomen.exe");
        res.set_manifest(r#"
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <assemblyIdentity version="1.0.0.0" name="ZenOmen.App"/>
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="asInvoker" uiAccess="false"/>
      </requestedPrivileges>
    </security>
  </trustInfo>
  <application xmlns="urn:schemas-microsoft-com:asm.v3">
    <windowsSettings>
      <dpiAware xmlns="http://schemas.microsoft.com/SMI/2005/WindowsSettings">true/pm</dpiAware>
      <dpiAwareness xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">PerMonitorV2, PerMonitor</dpiAwareness>
    </windowsSettings>
  </application>
</assembly>
"#);
        if let Err(e) = res.compile() {
            println!("cargo:warning=Failed to compile Windows resources: {}", e);
        }
    }
}
