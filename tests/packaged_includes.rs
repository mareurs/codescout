//! Every `include_str!` path that escapes a package's `src/` must survive `cargo package`.
//!
//! `Cargo.toml`'s `exclude` list strips `docs/` wholesale from the published tarball and
//! re-includes individual files by gitignore-style negation. A new
//! `include_str!("../docs/…")` therefore compiles and tests perfectly against the WORKING
//! TREE — where `docs/` is fully present — and breaks only inside the tarball that
//! `cargo package` / `cargo publish` build. The four documented gate commands all build
//! from the working tree, so they are blind to this by construction, not by omission.
//!
//! This has already shipped twice: `F-14` (`src/server.rs`), then `src/operator_rules/
//! corpus.rs:16` reintroduced the identical defect during Operator Rules Phase 2 — caught
//! only by a reviewer running `cargo package --list` by hand. F-14's write-up predicted
//! the recurrence in writing and the prediction held on the very next escaping site.
//!
//! **The oracle is cargo itself.** This test shells out to `cargo package --list` rather
//! than reimplementing `exclude`'s gitignore matching. A second implementation of one
//! operation is how the original defect class survives — it would agree with itself and
//! disagree with the tool that actually builds the tarball.
//!
//! Cost: `cargo package --list` does not build. Measured 0.21s per package, which is why
//! this is a test rather than a CI job — a CI-only gate runs on push, and this repo has
//! sat 119 commits ahead of origin for two days.
//!
//! Mutation-verified 2026-08-30, and the matrix is the result rather than the green tick:
//!
//! | mutation | gate | control |
//! |---|---|---|
//! | drop `"!docs/trackers/operator-rules.md"` from `exclude` (the defect that shipped) | **FAILS**, naming site, target and fix | passes |
//! | break `MARKER` so the scan finds nothing | **passes — vacuously** | **FAILS** |
//!
//! Read the second row before deleting `the_scan_actually_finds_the_known_escaping_sites`
//! as ceremony. With an empty population the gate is green over nothing, and the control is
//! the only thing standing between that and a false all-clear.

use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// (package name, package root) for every publishable package in the workspace.
fn packages() -> Vec<(String, PathBuf)> {
    let root = repo_root();
    let mut out = vec![("codescout".to_string(), root.clone())];
    if let Ok(entries) = std::fs::read_dir(root.join("crates")) {
        for e in entries.flatten() {
            let p = e.path();
            if p.join("Cargo.toml").is_file() {
                if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
                    out.push((name.to_string(), p.clone()));
                }
            }
        }
    }
    out.sort();
    out
}

fn rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            rs_files(&p, out);
        } else if p.extension().and_then(|x| x.to_str()) == Some("rs") {
            out.push(p);
        }
    }
}

/// Literal `include_str!("…")` arguments, with 1-based line numbers.
///
/// Deliberately a whole-file scan, not line-based, so a macro split across lines is still
/// seen. **Known blind spot:** an argument built with `concat!` or a `const` is invisible
/// here — this finds literals only, and reports what it found so a shrinking population is
/// noticeable (see `the_scan_actually_finds_the_known_escaping_sites`).
fn include_str_literals(src: &str) -> Vec<(usize, String)> {
    const MARKER: &str = "include_str!";
    let mut out = Vec::new();
    let mut idx = 0usize;
    while let Some(hit) = src[idx..].find(MARKER) {
        let at = idx + hit;
        let after = &src[at + MARKER.len()..];
        // The literal must be the next thing after the paren; anything else is a form
        // this scan does not claim to read.
        let Some(q1) = after.find('"') else { break };
        if after[..q1].contains(')') {
            idx = at + MARKER.len();
            continue;
        }
        let tail = &after[q1 + 1..];
        let Some(q2) = tail.find('"') else { break };
        let line = src[..at].bytes().filter(|b| *b == b'\n').count() + 1;
        out.push((line, tail[..q2].to_string()));
        idx = at + MARKER.len() + q1 + 1 + q2 + 1;
    }
    out
}

/// Resolve `rel` against `base`, collapsing `..` lexically (the file need not exist).
fn resolve(base: &Path, rel: &str) -> PathBuf {
    let mut out = base.to_path_buf();
    for c in Path::new(rel).components() {
        match c {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Sites whose `include_str!` target resolves outside the package's `src/`.
///
/// Returns `(site "file:line", package, package-relative target)`.
fn escaping_sites() -> Vec<(String, String, String)> {
    let root = repo_root();
    let mut found = Vec::new();
    for (pkg, pkg_root) in packages() {
        let src_dir = pkg_root.join("src");
        let mut files = Vec::new();
        rs_files(&src_dir, &mut files);
        for file in files {
            let Ok(text) = std::fs::read_to_string(&file) else {
                continue;
            };
            for (line, rel) in include_str_literals(&text) {
                let target = resolve(file.parent().unwrap(), &rel);
                if target.starts_with(&src_dir) {
                    continue; // stays inside src/ — src/ is never excluded
                }
                let site = format!(
                    "{}:{line}",
                    file.strip_prefix(&root).unwrap_or(&file).display()
                );
                let pkg_rel = target
                    .strip_prefix(&pkg_root)
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|_| format!("OUTSIDE-PACKAGE:{}", target.display()));
                found.push((site, pkg.clone(), pkg_rel));
            }
        }
    }
    found.sort();
    found
}

/// What `cargo package` would actually ship for `pkg`. `None` if cargo could not answer.
fn packaged_files(pkg: &str) -> Option<BTreeSet<String>> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let out = Command::new(cargo)
        .args(["package", "--list", "--allow-dirty", "--offline", "-p", pkg])
        .current_dir(repo_root())
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .map(|l| l.trim().replace('\\', "/"))
            .filter(|l| !l.is_empty())
            .collect(),
    )
}

#[test]
fn every_escaping_include_str_survives_cargo_package() {
    let sites = escaping_sites();
    let mut missing = Vec::new();
    let mut unanswered = Vec::new();

    let mut pkgs: Vec<String> = sites.iter().map(|(_, p, _)| p.clone()).collect();
    pkgs.sort();
    pkgs.dedup();

    for pkg in pkgs {
        let Some(shipped) = packaged_files(&pkg) else {
            unanswered.push(pkg);
            continue;
        };
        for (site, p, target) in sites.iter().filter(|(_, p, _)| *p == pkg) {
            if !shipped.contains(target.as_str()) {
                missing.push(format!("{site}  ->  {target}  (package `{p}`)"));
            }
        }
    }

    // A cargo that cannot answer must not read as a pass — that is the same false-green
    // the gate exists to remove.
    assert!(
        unanswered.is_empty(),
        "`cargo package --list` failed for: {}. The gate could not be evaluated, which is \
         not the same as passing — re-run once cargo can list these packages.",
        unanswered.join(", ")
    );

    assert!(
        missing.is_empty(),
        "these `include_str!` targets escape `src/` and are NOT in the published package:\n  \
         {}\n\nThey compile here because the working tree has them; the tarball will not, so \
         `cargo publish` fails to build. Fix by re-including each file in `Cargo.toml`'s \
         `exclude` list with a leading `!` (the `\"!docs/PROGRESSIVE_DISCOVERABILITY.md\"` \
         pattern), immediately after the entry that strips its directory. Then confirm with \
         `cargo package --list --allow-dirty | grep '^docs/'`.",
        missing.join("\n  ")
    );
}

/// The scan must actually find the sites we know exist.
///
/// Without this, a broken marker search, a wrong `CARGO_MANIFEST_DIR`, or a `src/` that
/// moved would yield "0 escaping sites, none missing" — a green tick over an empty
/// population, which is exactly the shape of the bug this file guards.
#[test]
fn the_scan_actually_finds_the_known_escaping_sites() {
    let sites = escaping_sites();
    assert!(
        !sites.is_empty(),
        "found no escaping `include_str!` at all — the scan is looking in the wrong place, \
         or `include_str_literals` no longer parses this codebase. Two sites existed when \
         this test was written (`src/server.rs`, `src/operator_rules/corpus.rs`). If the \
         last one was genuinely removed, delete this test with a note rather than letting \
         the gate above pass over nothing."
    );
    let targets: Vec<&str> = sites.iter().map(|(_, _, t)| t.as_str()).collect();
    assert!(
        targets.iter().any(|t| t.starts_with("docs/")),
        "expected at least one escaping target under docs/ — found {targets:?}"
    );
}

/// The escape test must discriminate, not just say yes.
///
/// `src/`-internal includes vastly outnumber escaping ones (50-odd across 12 files), so a
/// predicate that flagged everything would still make the gate above pass while turning
/// it into a slow no-op the first time an internal include was legitimately excluded.
#[test]
fn the_escape_detector_discriminates() {
    let src = repo_root().join("src");

    // Real shapes from this codebase: guides and schema live under src/ and never escape.
    assert!(resolve(&src.join("prompts"), "guides/librarian.md").starts_with(&src));
    assert!(resolve(&src.join("librarian/catalog"), "schema.sql").starts_with(&src));

    // The two real escaping shapes.
    assert!(!resolve(&src, "../docs/PROGRESSIVE_DISCOVERABILITY.md").starts_with(&src));
    assert!(!resolve(
        &src.join("operator_rules"),
        "../../docs/trackers/operator-rules.md"
    )
    .starts_with(&src));

    // And the parser reads both the single-line and the split-across-lines form. The line
    // reported is the MACRO's, not the literal's — that is the line the repo's own
    // citations use (`src/server.rs:1509`), and it is where a reader must edit.
    let one = include_str_literals(r#"const A: &str = include_str!("../docs/x.md");"#);
    assert_eq!(one, vec![(1, "../docs/x.md".to_string())]);
    let split = include_str_literals("const A: &str = include_str!(\n    \"../docs/y.md\"\n);");
    assert_eq!(split, vec![(1, "../docs/y.md".to_string())]);
}
