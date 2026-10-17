//! Path-shape queries over librarian tool responses, on the **ungated** side of
//! the `librarian` feature.
//!
//! The scan is pure `serde_json` and needs no librarian type, but its two callers
//! sit on opposite sides of the feature gate: `librarian::adapter` (gated) and
//! `prompts::guide_index` (unconditional). Defined in `librarian::adapter` it broke
//! `cargo check --no-default-features` with `E0433` — a break the documented local
//! gate cannot see, because `fmt`/`clippy`/`test` all run with default features.
//!
//! Same resolution as [`super::librarian_guard`], which reached this shape first: the
//! half with no librarian dependency lives in `util`, reachable from either side of
//! the gate. Gating the *call* instead was rejected — a `path~` shape would then match
//! on tool+action alone in the lean build, which is precisely the silent mismatch the
//! predicate exists to prevent.
//!
//! `docs/issues/archive/2026-08-27-experiments-head-fails-the-lean-build-and-the-local-gate-cannot-see-it.md`

use serde_json::Value;

/// Whether a librarian response names a path containing `needle`.
///
/// Generalised from the bug-file/tracker-path check in `librarian::adapter` so a section
/// declaration can carry an arbitrary `path~<substring>` predicate. The scanned shapes
/// are still deliberately shallow: top-level `abs_path`/`rel_path`, top-level
/// `old_abs_path`/`new_abs_path` (a `move` result, added 2026-10-17), one level into a
/// `find`-style `items` array, and `path` inside a `doctor`-style `violations` array.
/// Each is enumerated explicitly because a missing shape fails
/// as a *wrong guide* rather than an error — see
/// `docs/issues/archive/2026-08-20-doctor-entry-validity-rows-never-route-to-tracker-conventions.md`
/// for the case where `doctor` was the missing shape.
///
/// Separators are normalized before matching. A backslash-spelled Windows path
/// matched nothing against the hardcoded forward-slash needle and silently
/// delivered the *wrong* guide instead of erroring — measured under wine,
/// 2026-08-26, once Git Bash was available there to separate this from the
/// no-POSIX-shell failures it had been hiding behind.
pub fn names_path_containing(result: &Value, needle: &str) -> bool {
    fn hit(v: Option<&Value>, needle: &str) -> bool {
        v.and_then(Value::as_str)
            .is_some_and(|p| p.replace('\\', "/").contains(needle))
    }
    fn any_path_field(obj: &Value, needle: &str) -> bool {
        hit(obj.get("abs_path"), needle) || hit(obj.get("rel_path"), needle)
    }

    if any_path_field(result, needle) {
        return true;
    }
    // `move` -- the archive flow -- names the artifact only as the pair it moved between,
    // never as `abs_path` (`src/librarian/tools/mv.rs`). Either half counts: an archive
    // goes FROM the tracked directory TO its `archive/` child, and a move out of one still
    // routes by where it started. Top-level only, like `abs_path`: no other response
    // carries these keys, so the blast radius is exactly `move`.
    if hit(result.get("old_abs_path"), needle) || hit(result.get("new_abs_path"), needle) {
        return true;
    }
    if result
        .get("items")
        .and_then(Value::as_array)
        .is_some_and(|items| items.iter().any(|i| any_path_field(i, needle)))
    {
        return true;
    }
    // `doctor` is the one action whose rows carry neither `abs_path`/`rel_path` nor an
    // `items` array: it returns `violations: [{check, artifact_id, path, detail}]`
    // (`src/librarian/tools/doctor.rs:466`, field at `:135`). Scanned as its own key
    // rather than folded into `any_path_field`, which would widen the TOP-LEVEL check
    // across every response shape to serve one action; `violations` is unique to
    // `doctor`, so the blast radius is exactly that response.
    result
        .get("violations")
        .and_then(Value::as_array)
        .is_some_and(|rows| rows.iter().any(|row| hit(row.get("path"), needle)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn names_path_containing_generalises_and_normalises_separators() {
        let v = json!({"abs_path": "docs\\issues\\x.md"});
        assert!(names_path_containing(&v, "docs/issues/"));
        assert!(!names_path_containing(&v, "docs/trackers/"));
        // find-style items and doctor-style violations keep working.
        let items = json!({"items": [{"rel_path": "docs/trackers/t.md"}]});
        assert!(names_path_containing(&items, "docs/trackers/"));
        let viol = json!({"violations": [{"path": "docs/issues/b.md"}]});
        assert!(names_path_containing(&viol, "docs/issues/"));
    }

    /// The move must not have carried a `librarian`-feature dependency with it: this
    /// module is compiled unconditionally, so this test runs in the lean build too and
    /// is the thing that fails if someone reaches back across the gate from here.
    #[test]
    fn scans_every_documented_shape_without_a_librarian_dependency() {
        // Absent needle in each shape — the negative half the separators test omits
        // for `items` and `violations`.
        let items = json!({"items": [{"rel_path": "src/main.rs"}]});
        assert!(!names_path_containing(&items, "docs/trackers/"));
        let viol = json!({"violations": [{"path": "src/main.rs"}]});
        assert!(!names_path_containing(&viol, "docs/issues/"));
        // A shape carrying no path field at all matches nothing.
        assert!(!names_path_containing(&json!({"ok": true}), "docs/"));
    }
    /// `doc(action="move")` is the archive flow, and its result names the artifact only as
    /// `old_abs_path`/`new_abs_path` -- no `abs_path`, no `items`. Both halves are scanned
    /// (an archive moves FROM `docs/issues/` TO `docs/issues/archive/`, so either would do;
    /// a move out of the tracked directories still routes by its source), and a move that
    /// touches neither names nothing.
    /// docs/issues/2026-10-16-tracker-conventions-write-sections-are-declared-but-unreachable.md
    #[test]
    fn a_move_result_names_the_paths_it_moved_between() {
        let archive = json!({
            "id": "0123456789abcdef",
            "previous_id": "fedcba9876543210",
            "old_abs_path": "/repo/docs/issues/2026-10-16-x.md",
            "new_abs_path": "/repo/docs/issues/archive/2026-10-16-x.md",
        });
        assert!(names_path_containing(&archive, "docs/issues/"));
        assert!(!names_path_containing(&archive, "docs/trackers/"));

        // Either half alone is enough, and separators are normalised on both.
        let out_of_trackers = json!({
            "old_abs_path": "C:\\repo\\docs\\trackers\\t.md",
            "new_abs_path": "/repo/docs/plans/t.md",
        });
        assert!(names_path_containing(&out_of_trackers, "docs/trackers/"));
        let into_trackers = json!({
            "old_abs_path": "/repo/docs/plans/t.md",
            "new_abs_path": "/repo/docs/trackers/t.md",
        });
        assert!(names_path_containing(&into_trackers, "docs/trackers/"));

        let unrelated = json!({
            "old_abs_path": "/repo/docs/plans/a.md",
            "new_abs_path": "/repo/docs/plans/archive/a.md",
        });
        assert!(!names_path_containing(&unrelated, "docs/issues/"));
        assert!(!names_path_containing(&unrelated, "docs/trackers/"));
    }
}
