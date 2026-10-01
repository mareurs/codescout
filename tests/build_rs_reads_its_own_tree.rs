//! `build.rs` must slice the `source.md` of the tree it is BUILDING, not the tree its build
//! script was first compiled in (`e76df396`).
//!
//! `env!("CARGO_MANIFEST_DIR")` is expanded when the BUILD SCRIPT is compiled. Cargo does not
//! put the checkout path in a path package's metadata hash, so two checkouts with a
//! byte-identical `build.rs` share one compiled script when they share a `CARGO_TARGET_DIR` --
//! which `scripts/gate.sh`'s slot pool does on purpose. The second checkout then ran a script
//! with the FIRST checkout's path inside it: silently generating its prompt surfaces from the
//! wrong `source.md` while that tree existed, and panicking once it was removed. The panic is
//! the loud half; it reds three gate lanes in ~25 seconds before any code is compiled, which
//! reads exactly like a regression in the commits under test.
//!
//! **What this builds.** Two throwaway crates whose `build.rs` is this repository's own, byte
//! for byte, whose `Cargo.toml` and `src/main.rs` are identical, and which differ ONLY in the
//! sentinel inside `src/prompts/source.md`. Both build into one target directory. Each must
//! then print its own sentinel. Reading the repository's real `build.rs` is the point: a test
//! over a copy of the idea would agree with itself and say nothing about the shipped file.
//!
//! **The control, and what it does not cover.** Tree A is built first and must print A's
//! sentinel; that is what separates "the probe works" from "the probe is broken", since a
//! probe that printed nothing would otherwise fail the second assertion for the wrong reason.
//! It does NOT prove cargo still shares the compiled script across the two trees. If a future
//! cargo hashes the path in, this test stays green over a defect that can no longer occur, which
//! is harmless but is not coverage: the mutation that put `env!` back is what shows it
//! discriminates today.

use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// A crate whose `build.rs` is `build_rs` and whose generated surface carries `sentinel`.
fn write_probe_crate(dir: &Path, sentinel: &str, build_rs: &str) {
    std::fs::create_dir_all(dir.join("src/prompts")).unwrap();
    std::fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname = \"probe\"\nversion = \"0.0.0\"\nedition = \"2021\"\nbuild = \"build.rs\"\n\n\
         [workspace]\n",
    )
    .unwrap();
    std::fs::write(dir.join("build.rs"), build_rs).unwrap();
    std::fs::write(
        dir.join("src/main.rs"),
        "fn main() {\n    print!(\n        \"{}\",\n        include_str!(concat!(env!(\"OUT_DIR\"), \"/server_instructions.md\"))\n    );\n}\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("src/prompts/source.md"),
        format!(
            "<!-- @surface server_instructions -->\n{sentinel}\n<!-- @end -->\n\
             <!-- @surface onboarding_prompt -->\nunused\n<!-- @end -->\n"
        ),
    )
    .unwrap();
}

/// What the probe built in `dir` prints: the surface its build script generated. `Err(cause)`
/// carries cargo's own stderr, so a cargo that could not answer never reads as a pass.
fn run_probe(dir: &Path, target: &Path) -> Result<String, String> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let out = Command::new(&cargo)
        .args(["run", "--offline", "--quiet"])
        .current_dir(dir)
        .env("CARGO_TARGET_DIR", target)
        .env_remove("CARGO_MANIFEST_DIR")
        .output()
        .map_err(|e| format!("failed to spawn `{cargo}`: {e}"))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            format!("exited {} with no stderr", out.status)
        } else {
            stderr
        });
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

#[test]
fn two_trees_sharing_a_target_each_slice_their_own_source_md() {
    let build_rs = std::fs::read_to_string(repo_root().join("build.rs")).unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let tree_a = tmp.path().join("tree-a");
    let tree_b = tmp.path().join("tree-b");
    let target = tmp.path().join("shared-target");
    write_probe_crate(&tree_a, "SENTINEL-A", &build_rs);
    write_probe_crate(&tree_b, "SENTINEL-B", &build_rs);

    let from_a = run_probe(&tree_a, &target)
        .unwrap_or_else(|cause| panic!("the probe did not build in tree A: {cause}"));
    assert!(
        from_a.contains("SENTINEL-A"),
        "control: tree A's own build must print A's sentinel, got {from_a:?}"
    );

    let from_b = run_probe(&tree_b, &target)
        .unwrap_or_else(|cause| panic!("the probe did not build in tree B: {cause}"));
    assert!(
        from_b.contains("SENTINEL-B"),
        "tree B's build script read another tree's source.md instead of its own: got {from_b:?}"
    );
}
