//! Field-aware project-root stripping.
//!
//! Renders absolute project paths relative **by JSON key**, on the typed
//! `Value`, before it is rendered to text. This replaces the text-level
//! transform that lived in `server::post_process`: that one ran after the
//! `Value` had been flattened to a string, so it could only guess from a
//! one-character lookbehind — which stripped path literals inside file
//! content and collapsed root-valued fields to the empty string.
//!
//! See `docs/issues/archive/2026-08-09-path-strip-corrupts-file-content-and-root-fields.md`
//! and `docs/superpowers/specs/2026-08-09-field-aware-path-strip-design.md`.

use serde_json::Value;

/// Keys whose values are paths to be rendered relative to the project root.
///
/// **This is an ALLOWLIST and must stay one.** A key that is absent keeps its
/// absolute path — that costs tokens but never corrupts. Inverting this into a
/// denylist of content keys would strip unknown keys by default, which is
/// precisely the defect this module exists to remove. When a new tool's paths
/// come back absolute, add its key here.
///
/// **Scope of the corpus gate.** `src/server.rs`'s
/// `no_absolute_project_paths_in_rendered_output` only covers the file-tool
/// surface (`tree`, `grep`, `read_file`, `symbols`) — no
/// librarian tool is in its fixture set, so the seven keys librarian emits
/// (`deleted_abs_path`, `main_path`, `new_abs_path`, `new_path`,
/// `old_abs_path`, `stage_together`, `targets`) are exercised only by
/// synthetic-`Value` unit
/// tests below, not by that gate. Of the keys the gate *can* see, only a
/// forgotten key on `grep` currently fails the negative assertion — the rest
/// are masked by tool-specific rendering (see the liveness-guard note on that
/// test). A key omitted from either surface costs tokens on every response;
/// it does not corrupt output.
pub(crate) const PATH_KEYS: &[&str] = &[
    "abs_path",
    "deleted_abs_path",
    "directory",
    "entries",
    "file",
    "file_path",
    "main_path",
    "new_abs_path",
    "new_path",
    "old_abs_path",
    "path",
    "prompt_path",
    "rel_path",
    "stage_together",
    "synthesis_prompt_path",
    "targets",
];

/// Keys whose value IS a root rather than a path under one. These stay
/// ABSOLUTE: a root is the anchor every other path is relative to, and
/// relativizing one yields the empty string — measured 136 times across 12
/// sessions before this change.
pub(crate) const ROOT_KEYS: &[&str] = &[
    "cwd",
    "git_root",
    "new_root",
    "old_root",
    "project_root",
    "repo_root",
    "root",
];

/// Relativize every allowlisted path value in `val`, in place.
///
/// `root_prefix` must end with `/`; pass `""` when no project is active and
/// the call becomes a no-op. The trailing slash is load-bearing: it is what
/// stops a value that *equals* the root (stored bare) from matching, which is
/// how the same key name can mean "file path" on one node and "root" on
/// another without the walker needing path context.
pub(crate) fn strip_paths_in_value(val: &mut Value, root_prefix: &str) {
    if root_prefix.is_empty() {
        return;
    }
    debug_assert!(
        root_prefix.ends_with('/'),
        "root_prefix must end with '/' — a slash-less prefix yields leading-slash \
         values that the empty-string guard does not catch"
    );
    match val {
        Value::Object(map) => {
            for (key, child) in map.iter_mut() {
                if ROOT_KEYS.contains(&key.as_str()) {
                    continue;
                }
                if PATH_KEYS.contains(&key.as_str()) {
                    relativize(child, root_prefix);
                }
                strip_paths_in_value(child, root_prefix);
            }
        }
        Value::Array(items) => {
            for item in items {
                strip_paths_in_value(item, root_prefix);
            }
        }
        _ => {}
    }
}

/// Relativize a value sitting under a path key: a string, or an array of
/// strings (`tree`'s `entries`, `reindex`'s `targets`).
fn relativize(val: &mut Value, root_prefix: &str) {
    match val {
        Value::String(s) => {
            if let Some(rest) = s.strip_prefix(root_prefix) {
                // Never emit "" for a path — an empty string reads as a valid
                // value and is worse than a long one.
                if !rest.is_empty() {
                    *s = rest.to_string();
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                relativize(item, root_prefix);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const ROOT: &str = "/home/u/proj/";

    #[test]
    fn relativizes_a_path_key() {
        let mut v = json!({ "file": "/home/u/proj/src/lib.rs" });
        strip_paths_in_value(&mut v, ROOT);
        assert_eq!(v["file"], "src/lib.rs");
    }

    #[test]
    fn leaves_file_content_untouched() {
        // The reported bug: a quoted path literal inside content must survive.
        let src = "REPO = \"/home/u/proj/.worktrees/single-stage\"";
        // The realistic corruption shape: a value that *begins* with the root.
        // `relativize` only ever runs under PATH_KEYS, so `strip_prefix` never
        // even sees this — but if the gate were gone, offset-0 matching would
        // make this the actual failure mode (unlike the mid-string case above).
        let leading = "/home/u/proj/src/lib.rs\n";
        let mut v = json!({ "content": src, "stdout": leading, "body": src, "text": src });
        strip_paths_in_value(&mut v, ROOT);
        assert_eq!(v["content"], src);
        assert_eq!(v["stdout"], leading);
        assert_eq!(v["body"], src);
        assert_eq!(v["text"], src);
    }

    #[test]
    fn never_produces_an_empty_string() {
        // A bare root under a path key must NOT collapse to "" — it does not
        // match a slash-terminated prefix, and the guard is belt-and-braces.
        let mut v = json!({ "path": "/home/u/proj", "abs_path": "/home/u/proj/" });
        strip_paths_in_value(&mut v, ROOT);
        assert_eq!(v["path"], "/home/u/proj");
        assert_eq!(v["abs_path"], "/home/u/proj/");
    }
    #[test]
    fn a_sibling_directory_sharing_the_root_as_a_prefix_is_untouched() {
        // With the trailing slash, "/home/u/projX/..." cannot match "/home/u/proj/".
        // Without it, strip_prefix("/home/u/proj") matches and yields "X/file.rs" —
        // non-empty, so the empty-string guard would NOT catch the corruption.
        let mut v = json!({ "abs_path": "/home/u/projX/file.rs" });
        strip_paths_in_value(&mut v, ROOT);
        assert_eq!(v["abs_path"], "/home/u/projX/file.rs");
    }

    #[test]
    fn a_path_key_whose_value_contains_the_root_mid_string_is_untouched() {
        // `relativize` must be ANCHORED. An unanchored substring removal would
        // rewrite this to "/other/src/lib.rs". Deleting the old
        // strip_prefix_not_inside_longer_path left this contract homeless.
        let mut v = json!({ "file": "/other/home/u/proj/src/lib.rs" });
        strip_paths_in_value(&mut v, ROOT);
        assert_eq!(v["file"], "/other/home/u/proj/src/lib.rs");
    }

    #[test]
    fn root_keys_stay_absolute() {
        let mut v = json!({
            "project_root": "/home/u/proj",
            "git_root": "/home/u/proj",
            "root": "/home/u/proj",
            "cwd": "/home/u/proj"
        });
        strip_paths_in_value(&mut v, ROOT);
        assert_eq!(v["project_root"], "/home/u/proj");
        assert_eq!(v["git_root"], "/home/u/proj");
        assert_eq!(v["root"], "/home/u/proj");
        assert_eq!(v["cwd"], "/home/u/proj");
    }
    #[test]
    fn a_root_key_prunes_recursion_beneath_it() {
        // The ROOT_KEYS `continue` skips the whole subtree, not just the scalar.
        let mut v = json!({ "root": { "file": "/home/u/proj/a.rs" } });
        strip_paths_in_value(&mut v, ROOT);
        assert_eq!(v["root"]["file"], "/home/u/proj/a.rs");
    }

    #[test]
    fn scope_block_abs_path_survives_while_item_abs_path_relativizes() {
        // Same key, two meanings. The trailing slash is what discriminates:
        // the scope root is stored bare and cannot match ROOT.
        let mut v = json!({
            "items": [{ "abs_path": "/home/u/proj/docs/t.md" }],
            "scope":  { "abs_path": "/home/u/proj", "git_root": "/home/u/proj" }
        });
        strip_paths_in_value(&mut v, ROOT);
        assert_eq!(v["items"][0]["abs_path"], "docs/t.md");
        assert_eq!(v["scope"]["abs_path"], "/home/u/proj");
    }

    #[test]
    fn relativizes_an_array_of_paths() {
        // tree's `entries` is an array of bare strings, not objects.
        let mut v = json!({ "entries": ["/home/u/proj/src/", "/home/u/proj/docs/"] });
        strip_paths_in_value(&mut v, ROOT);
        assert_eq!(v["entries"], json!(["src/", "docs/"]));
    }

    #[test]
    fn recurses_into_nested_objects_and_arrays() {
        let mut v = json!({
            "file_groups": [{ "file": "/home/u/proj/a.rs", "matches": [
                { "file": "/home/u/proj/b.rs", "line_text": "x = \"/home/u/proj/c\"" }
            ]}]
        });
        strip_paths_in_value(&mut v, ROOT);
        assert_eq!(v["file_groups"][0]["file"], "a.rs");
        assert_eq!(v["file_groups"][0]["matches"][0]["file"], "b.rs");
        assert_eq!(
            v["file_groups"][0]["matches"][0]["line_text"],
            "x = \"/home/u/proj/c\""
        );
    }

    #[test]
    fn unknown_key_keeps_its_absolute_path() {
        // Fail-verbose, never fail-corrupt.
        let mut v = json!({ "some_new_key": "/home/u/proj/src/lib.rs" });
        strip_paths_in_value(&mut v, ROOT);
        assert_eq!(v["some_new_key"], "/home/u/proj/src/lib.rs");
    }

    #[test]
    fn empty_prefix_is_a_no_op() {
        let mut v = json!({ "file": "/home/u/proj/src/lib.rs" });
        strip_paths_in_value(&mut v, "");
        assert_eq!(v["file"], "/home/u/proj/src/lib.rs");
    }

    #[test]
    fn a_path_outside_the_root_is_untouched() {
        let mut v = json!({ "file": "/etc/hosts" });
        strip_paths_in_value(&mut v, ROOT);
        assert_eq!(v["file"], "/etc/hosts");
    }
}
