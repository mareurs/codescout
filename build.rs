fn main() {
    // Bake the codescout git SHA into the binary at compile time.
    // Falls back to "unknown" for non-git builds (e.g. crates.io install).
    let sha_short = std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let sha_full = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let dirty = std::process::Command::new("git")
        .args(["status", "--porcelain"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| if o.stdout.is_empty() { "0" } else { "1" })
        .unwrap_or("unknown");

    println!("cargo:rustc-env=CODESCOUT_GIT_SHA={sha_short}");
    println!("cargo:rustc-env=CODESCOUT_GIT_SHA_FULL={sha_full}");
    println!("cargo:rustc-env=CODESCOUT_GIT_DIRTY={dirty}");

    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/index");
    println!("cargo:rerun-if-changed=.git/refs/heads/");
}
