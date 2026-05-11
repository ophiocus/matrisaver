// Modeled on I:\Skeleton\build.rs — derive a 4-part version from the
// latest `git describe --tags --match v*` tag, fall back to
// CARGO_PKG_VERSION when not in a git checkout. The result is exposed
// to the compiled binary as the APP_VERSION env var so update_check.rs
// can compare against GitHub Releases tags.

fn main() {
    let cargo_version = env!("CARGO_PKG_VERSION");
    let full_version = std::process::Command::new("git")
        .args(["describe", "--tags", "--match", "v*", "--abbrev=0"])
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                String::from_utf8(o.stdout).ok()
            } else {
                None
            }
        })
        .map(|s| s.trim().trim_start_matches('v').to_string())
        .unwrap_or_else(|| cargo_version.to_string());

    println!("cargo:rustc-env=APP_VERSION={full_version}");
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs/tags");

    #[cfg(target_os = "windows")]
    {
        let mut res = winres::WindowsResource::new();
        res.set("ProductName", "MatriSaver");
        res.set(
            "FileDescription",
            "Matrix-style digital rain Windows screensaver",
        );
        res.set("FileVersion", &full_version);
        res.set("ProductVersion", &full_version);
        let ico = std::path::Path::new("assets/icon.ico");
        if ico.exists() {
            res.set_icon("assets/icon.ico");
        }
        // Resource compilation is best-effort; failure here should never
        // block a dev build. CI will re-run on a clean Windows host.
        let _ = res.compile();
    }
}
