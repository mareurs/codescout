//! Build-time tasks for codescout.
//!
//! Two responsibilities, kept in one file because they share a `main`:
//!
//! 1. Bake the git SHA into the binary as `CODESCOUT_GIT_SHA`.
//! 2. Slice `src/prompts/source.md` (the single source of truth for the .md
//!    prompt surfaces) into one file per surface in `OUT_DIR`. `mod.rs` then
//!    `include_str!`s those files into `pub const &str` constants — keeping
//!    call-site semantics unchanged while moving the editable surface into a
//!    single document. The slicing logic mirrors
//!    `src/prompts/source.rs::extract_surface`; build scripts can't depend on
//!    the crate they build, so the parser is duplicated. A Phase 1a unit test
//!    pins byte-equality between the original .md files and source.md slices,
//!    catching divergence between the two parser copies.

use std::path::PathBuf;

const SURFACES: &[&str] = &["server_instructions", "onboarding_prompt"];

fn main() {
    bake_git_sha();
    emit_prompt_surfaces();
}

fn bake_git_sha() {
    let sha = std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    println!("cargo:rustc-env=CODESCOUT_GIT_SHA={sha}");
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs/heads/");
}

fn emit_prompt_surfaces() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let source_path = manifest.join("src/prompts/source.md");
    let source = std::fs::read_to_string(&source_path)
        .unwrap_or_else(|e| panic!("read {}: {e}", source_path.display()));

    let out_dir: PathBuf = std::env::var_os("OUT_DIR")
        .expect("OUT_DIR set by cargo")
        .into();

    for surface in SURFACES {
        let content = extract_surface(&source, surface).unwrap_or_else(|| {
            panic!(
                "surface `{surface}` not found in {} — every entry in build.rs SURFACES \
                 must have a `<!-- @surface NAME -->` block in source.md",
                source_path.display()
            )
        });
        let dest = out_dir.join(format!("{surface}.md"));
        std::fs::write(&dest, content).unwrap_or_else(|e| panic!("write {}: {e}", dest.display()));
    }

    println!("cargo:rerun-if-changed=src/prompts/source.md");
    println!("cargo:rerun-if-changed=build.rs");
}

fn extract_surface<'a>(source: &'a str, surface: &str) -> Option<&'a str> {
    let open = format!("<!-- @surface {surface} -->\n");
    let start = source.find(&open)? + open.len();
    let rest = &source[start..];
    let end_offset = rest.find("<!-- @end -->")?;
    Some(&rest[..end_offset])
}
