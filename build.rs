use std::process::Command;

fn main() {
    println!("cargo:rerun-if-env-changed=REFORGE_BUILD_REVISION");
    println!("cargo:rerun-if-changed=.git/HEAD");
    if let Ok(head) = std::fs::read_to_string(".git/HEAD")
        && let Some(reference) = head.trim().strip_prefix("ref: ")
    {
        println!("cargo:rerun-if-changed=.git/{reference}");
        println!("cargo:rerun-if-changed=.git/packed-refs");
    }

    let revision = std::env::var("REFORGE_BUILD_REVISION")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            Command::new("git")
                .args(["rev-parse", "HEAD"])
                .output()
                .ok()
                .filter(|output| output.status.success())
                .and_then(|output| String::from_utf8(output.stdout).ok())
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        });

    let version = std::env::var("CARGO_PKG_VERSION").unwrap_or_default();
    if let Some(revision) = revision {
        println!("cargo:rustc-env=REFORGE_BUILD_REVISION={revision}");
        println!("cargo:rustc-env=REFORGE_LONG_VERSION={version} ({revision})");
    } else {
        println!("cargo:rustc-env=REFORGE_LONG_VERSION={version}");
    }
}
