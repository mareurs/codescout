//! Every per-request tool path must resolve the project through the WORKSPACE PIN,
//! never through the ambient active project.
//!
//! # What this guards
//!
//! `ToolContext::workspace_override` carries a per-request `workspace=` pin. `Agent`
//! exposes two families of accessor:
//!
//! | pinned (correct in a tool) | ambient (correct only at startup/dispatch) |
//! |---|---|
//! | `with_project_at(override, …)` | `with_project(…)` |
//! | `with_project_at_mut(override, …)` | — |
//! | `require_project_root_for(override)` | `require_project_root()` |
//! | — | `inner.active_project()` |
//!
//! A tool that reaches for the ambient family silently scopes its work to the
//! session's active project and ignores the caller's pin. That fails **silent-wrong**:
//! the call succeeds and answers about the wrong repository. It has happened four
//! times and all four are archived —
//! `docs/issues/archive/2026-07-17-artifact-find-ignores-workspace-pin.md` is the one
//! that reached the librarian adapter.
//!
//! # Why a population guard rather than the migration
//!
//! The pin itself is fully wired (324 references across 47 files; phases 0–3 of
//! `docs/plans/2026-05-30-per-request-workspace-pinning.md` completed 2026-05-31).
//! The 2026-09-16 architecture scout recommended **against** threading a resolved
//! request object through the 95 resolution call sites, and **for** this guard: the
//! defect class has zero live instances, so what is worth buying is that a NEW site
//! cannot join it silently. Derivation: `docs/trackers/architecture-boundary-measurement.md`
//! § *Slice 3 — scouted 2026-09-16*.
//!
//! # The population, and why it is file-level
//!
//! Scanned: `.rs` files under `src/tools/`, excluding any file named `tests.rs`.
//!
//! Test code uses the ambient accessors legitimately and heavily — 23 of the 24
//! ambient calls under `src/tools/` live in `tests.rs` files. Excluding by FILENAME
//! rather than by `#[cfg(test)]` is deliberate: in this tree `#[cfg(test)]` attaches
//! to individual functions as often as to a `mod tests` block (see
//! `src/tools/config/mod.rs`, markers at 782 and 1328 with production code between
//! them), so truncating a file at its first marker would silently drop real
//! production code from the population — a false NEGATIVE, the direction that passes.
//!
//! **The residual failure runs toward a false POSITIVE.** An inline `#[cfg(test)]`
//! module inside a production file that calls an ambient accessor reds this guard.
//! That is loud and cheap to resolve — annotate the site or move the test to
//! `tests.rs` — and it is the safe direction to be wrong in. There are zero such
//! sites today.
//!
//! # The escape hatch
//!
//! A genuine ambient call carries `// pin-exempt: <reason>` within the three lines
//! above it. There is exactly one, in `src/tools/memory/mod.rs` — the documented
//! "no workspace configured" fallback, whose sibling branch resolves the pin.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Ambient accessors — correct at startup and dispatch, wrong on a per-request path.
const AMBIENT: [&str; 3] = [
    ".with_project(",
    ".require_project_root(",
    ".active_project(",
];

/// Pinned accessors, used only by the non-vacuity companion to prove the scanned
/// population really is tool code that resolves projects.
const PINNED: [&str; 3] = [
    ".with_project_at(",
    ".require_project_root_for(",
    ".with_project_at_mut(",
];

const EXEMPT_MARKER: &str = "pin-exempt:";

struct Site {
    file: String,
    line: usize,
    text: String,
    exempt: bool,
}

/// `.rs` files under `src/tools/`, minus `tests.rs`. Walks one explicit root — never
/// the repo root, which carries `.worktrees/` checkouts of other branches whose
/// findings this working tree cannot fix.
fn scanned_files() -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs")
                && path.file_name().is_some_and(|n| n != "tests.rs")
            {
                out.push(path);
            }
        }
    }
    let mut out = Vec::new();
    walk(&repo_root().join("src/tools"), &mut out);
    out.sort();
    out
}

fn rel(p: &Path) -> String {
    p.strip_prefix(repo_root())
        .unwrap_or(p)
        .to_string_lossy()
        .replace('\\', "/")
}

/// Every ambient-accessor call site in the scanned population, each tagged with
/// whether an exemption annotation sits within the three lines above it.
fn ambient_sites() -> Vec<Site> {
    let mut sites = Vec::new();
    for path in scanned_files() {
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let lines: Vec<&str> = text.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            // A pinned call contains an ambient substring (`.with_project_at(` holds
            // no `.with_project(`, but `.require_project_root_for(` is not matched by
            // `.require_project_root(` either — both differ before the paren, so a
            // plain `contains` is exact here). Checked by the companion test below.
            if !AMBIENT.iter().any(|a| line.contains(a)) {
                continue;
            }
            let exempt = lines[i.saturating_sub(3)..i]
                .iter()
                .any(|l| l.contains(EXEMPT_MARKER));
            sites.push(Site {
                file: rel(&path),
                line: i + 1,
                text: line.trim().to_string(),
                exempt,
            });
        }
    }
    sites
}

/// A per-request tool path that resolves the project ambiently ignores the caller's
/// `workspace=` pin and answers about the wrong repository, successfully.
#[test]
fn every_tool_path_resolves_the_project_through_the_workspace_pin() {
    let sites = ambient_sites();
    let offenders: Vec<&Site> = sites.iter().filter(|s| !s.exempt).collect();

    assert!(
        offenders.is_empty(),
        "these tool paths resolve the project ambiently and will ignore a caller's \
         `workspace=` pin:\n{}\n\nUse the pinned accessor \
         (`with_project_at(ctx.workspace_override.as_deref(), …)`, \
         `require_project_root_for(ctx.workspace_override.as_deref())`). If the ambient \
         call is genuinely correct — a no-workspace fallback, say — put \
         `// {EXEMPT_MARKER} <reason>` within three lines above it.",
        offenders
            .iter()
            .map(|s| format!("  {}:{}  {}", s.file, s.line, s.text))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// Four ways the guard above goes quiet while the tree is broken: the walk stops
/// descending, the root moves, `tests.rs` exclusion swallows everything, or an
/// accessor is renamed so `AMBIENT` stops matching. Every one returns the same green
/// tick as a clean tree.
///
/// Named distinctly from `tests/feature_lanes.rs` § `the_guard_is_not_vacuous` on
/// purpose: `CLAUDE.md`'s gate ritual is to read YOUR OWN test names out of the
/// default lane, and two identical names in one lane's output cannot be resolved
/// except by line adjacency, which is not a disambiguator.
#[test]
fn the_pin_parity_scan_is_not_vacuous() {
    let files = scanned_files();
    assert!(
        files.len() >= 30,
        "expected a populated set of tool source files, found {} — the walk has stopped \
         reaching src/tools/, and an empty population passes the guard above silently",
        files.len()
    );

    let names: BTreeSet<String> = files.iter().map(|p| rel(p)).collect();
    assert!(
        names.contains("src/tools/memory/mod.rs"),
        "expected the scan to reach src/tools/memory/mod.rs, which holds the sole \
         exempted ambient call; scanned {} files",
        names.len()
    );

    // The matcher must actually fire. If `AMBIENT` stopped matching — an accessor
    // renamed, a leading `.` dropped — `ambient_sites()` empties and the guard above
    // passes on a tree where every tool resolves ambiently. The one known exemption
    // is the fixture that proves the pattern is live.
    let sites = ambient_sites();
    assert!(
        sites
            .iter()
            .any(|s| s.file == "src/tools/memory/mod.rs" && s.exempt),
        "expected the known exempt ambient call in src/tools/memory/mod.rs to be \
         FOUND and recognised as exempt; without it the AMBIENT patterns are not \
         proven to match anything, and the guard above is vacuous. Found {} ambient \
         site(s): {:?}",
        sites.len(),
        sites
            .iter()
            .map(|s| format!("{}:{}", s.file, s.line))
            .collect::<Vec<_>>()
    );

    // And the population must be tool code that resolves projects at all — otherwise
    // "no ambient calls" is true of a directory that never resolves a project.
    let pinned_hits: usize = files
        .iter()
        .filter_map(|p| std::fs::read_to_string(p).ok())
        .map(|t| PINNED.iter().filter(|p| t.contains(**p)).count())
        .sum();
    assert!(
        pinned_hits >= 10,
        "expected the scanned population to contain pinned-accessor calls, found {pinned_hits} — \
         a population that never resolves a project satisfies the guard above for free"
    );
}

/// `AMBIENT`'s substrings must not match the PINNED accessors, or the guard reds on
/// correct code and the fix is to make every tool ambient.
#[test]
fn the_ambient_patterns_do_not_match_the_pinned_accessors() {
    for pinned in PINNED {
        for ambient in AMBIENT {
            assert!(
                !pinned.contains(ambient),
                "pinned accessor `{pinned}` contains ambient pattern `{ambient}` — the \
                 guard would report correct pinned calls as offenders"
            );
        }
    }
}
