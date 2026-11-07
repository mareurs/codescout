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

/// Paths whose changes mark the build dirty: sources compiled into the executable.
/// A conservative input scope, not byte-equivalence of binaries.
const DIRTY_SCOPE: &[&str] = &[
    "src",
    "crates",
    "scripts",
    "build.rs",
    "Cargo.toml",
    "Cargo.lock",
    "docs/PROGRESSIVE_DISCOVERABILITY.md",
    "docs/trackers/operator-rules.md",
];

fn git_output(repo: &std::path::Path, args: &[&str]) -> Option<Vec<u8>> {
    std::process::Command::new("git")
        // A metadata read must not refresh the watched index and retrigger Cargo.
        .arg("--no-optional-locks")
        .arg("-C")
        .arg(repo)
        .args(args)
        // Cargo can inherit these from a caller operating in another repository.
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| o.stdout)
}

fn bake_git_sha() {
    let repo = PathBuf::from(
        std::env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR set by cargo"),
    );
    let sha = git_output(&repo, &["rev-parse", "--short", "HEAD"])
        .and_then(|s| String::from_utf8(s).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let sha_full = git_output(&repo, &["rev-parse", "HEAD"])
        .and_then(|s| String::from_utf8(s).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let mut status_args = vec!["status", "--porcelain", "--untracked-files=all", "--"];
    status_args.extend(DIRTY_SCOPE.iter().copied());
    let dirty = git_output(&repo, &status_args)
        .map(|o| if o.is_empty() { "0" } else { "1" })
        .unwrap_or("unknown");

    println!("cargo:rustc-env=CODESCOUT_GIT_SHA={sha}");
    println!("cargo:rustc-env=CODESCOUT_GIT_SHA_FULL={sha_full}");
    println!("cargo:rustc-env=CODESCOUT_GIT_DIRTY={dirty}");
    for path in DIRTY_SCOPE {
        println!("cargo:rerun-if-changed={path}");
    }
    watch_git_state(&repo);
}

/// Resolve where git keeps `name` for the checkout at `repo`, for ordinary and linked worktrees.
/// `HEAD` and `index` are per worktree; everything under `refs/` and `packed-refs` live in the
/// common git directory shared by every worktree.
fn git_admin_path(repo: &std::path::Path, name: &str) -> Option<PathBuf> {
    let out = git_output(repo, &["rev-parse", "--git-path", name])?;
    let out = String::from_utf8(out).ok()?;
    Some(repo.join(out.trim_end_matches(['\n', '\r'])))
}

/// Ask Cargo to rerun this script when the baked SHA could change, and not otherwise.
///
/// Watching the whole `refs` directory or `packed-refs` reruns the script (and rebuilds the
/// crate) in every checkout that shares the repository whenever ANY branch gets a commit
/// (f8d06297d629928e). Only `HEAD`, `index` and the one ref `HEAD` points to decide the value.
/// A missing path makes Cargo rerun on every invocation, so a path is printed only when it exists.
fn watch_git_state(repo: &std::path::Path) {
    for name in ["HEAD", "index"] {
        if let Some(path) = git_admin_path(repo, name).filter(|p| p.exists()) {
            println!("cargo:rerun-if-changed={}", path.display());
        }
    }
    // Detached HEAD names no ref: `HEAD` itself changes on checkout, `index` on staging.
    let Some(branch) = git_output(repo, &["symbolic-ref", "-q", "HEAD"])
        .and_then(|s| String::from_utf8(s).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
    else {
        return;
    };
    let Some(loose) = git_admin_path(repo, &branch) else {
        return;
    };
    if loose.exists() {
        println!("cargo:rerun-if-changed={}", loose.display());
        return;
    }
    // The branch is packed or still unborn. Its next commit CREATES the loose ref and leaves
    // `packed-refs` alone, so watching only `packed-refs` would miss it: also watch the nearest
    // existing ancestor directory of the missing ref, where that file will appear. This is
    // wider than one file. The main way in is `git gc` or `git pack-refs --all --prune`, which
    // empties `refs/heads`, so every checkout whose branch has had no commit since falls back
    // together; each leaves the fallback when its first commit turns its ref loose. An unborn
    // branch (fresh `git init`) lands here too.
    if let Some(packed) = git_admin_path(repo, "packed-refs").filter(|p| p.exists()) {
        println!("cargo:rerun-if-changed={}", packed.display());
    }
    let mut dir = loose.parent();
    while let Some(d) = dir {
        if d.exists() {
            println!("cargo:rerun-if-changed={}", d.display());
            break;
        }
        dir = d.parent();
    }
}

fn emit_prompt_surfaces() {
    // Read at RUN time, not with `env!`: that macro expands when the build script is COMPILED,
    // and cargo reuses one compiled script across checkouts that share a target directory, so
    // the baked path named whichever tree built first (e76df396,
    // tests/build_rs_reads_its_own_tree.rs). Cargo sets this variable for every run.
    let manifest = PathBuf::from(
        std::env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR set by cargo"),
    );
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

/// **Duplicate of `src/prompts/source.rs::extract_surface` — keep in sync.**
///
/// Build scripts compile as a separate unit and cannot `use` crate code, so
/// this function is intentionally copy-pasted. Any change to surface-marker
/// parsing (e.g. CRLF tolerance — see commits af64c737, c83b5544 of the
/// CI rehab session) must be applied to both copies. F-12 in
/// `docs/trackers/bug-fix-session-log.md` captures the cost of missing this.
fn extract_surface<'a>(source: &'a str, surface: &str) -> Option<&'a str> {
    let open = format!("<!-- @surface {surface} -->");
    let marker_end = source.find(&open)? + open.len();
    let bytes = source.as_bytes();
    let mut start = marker_end;
    if bytes.get(start) == Some(&b'\r') {
        start += 1;
    }
    if bytes.get(start) == Some(&b'\n') {
        start += 1;
    }
    let rest = &source[start..];
    let end_offset = rest.find("<!-- @end -->")?;
    Some(&rest[..end_offset])
}
