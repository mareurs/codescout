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
//! `docs/issues/2026-08-27-experiments-head-fails-the-lean-build-and-the-local-gate-cannot-see-it.md`

use serde_json::Value;

/// Whether a librarian response names a path containing `needle`.
///
/// Generalised from the bug-file/tracker-path check in `librarian::adapter` so a section
/// declaration can carry an arbitrary `path~<substring>` predicate. The scanned shapes
/// are unchanged and still deliberately shallow: top-level `abs_path`/`rel_path`,
/// one level into a `find`-style `items` array, and `path` inside a `doctor`-style
/// `violations` array. Each is enumerated explicitly because a missing shape fails
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
}
