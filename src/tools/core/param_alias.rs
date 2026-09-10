//! Parameter alias normalization.
//!
//! The tool surface advertises exactly ONE name per concept. Callers habitually
//! send others — `file_path` above all, because Claude Code's native `Read` uses
//! it. Those are accepted, rewritten to the canonical key before any consumer
//! reads the input, and announced on the response.
//!
//! Why the rewrite happens at the dispatch boundary rather than in each tool:
//! `call_content` reads `input` three times before `call()` runs (`selector_key`,
//! `is_write`, `write_path`), and a tool that normalized internally would leave
//! those three looking at the un-normalized shape. See the spec,
//! `docs/superpowers/specs/2026-09-10-parameter-alias-collapse-design.md`.

use serde_json::Value;

/// A non-canonical parameter name a caller sent, and what it was rewritten to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Correction {
    /// The key as received.
    pub received: String,
    /// The advertised key it was rewritten to.
    pub canonical: &'static str,
    /// True when the canonical key was ALSO supplied, so the aliased value was
    /// discarded rather than used. Reported separately because a silently
    /// dropped value is a different event from a rename.
    pub conflicted: bool,
}

/// `(received, canonical)` pairs. Declared per tool via `Tool::param_aliases`.
pub type AliasMap = &'static [(&'static str, &'static str)];

/// Rewrite every alias key in `input` to its canonical name, in place.
///
/// MUTATES rather than returning a new value: `create_file`/`edit_file` carry
/// whole file bodies in `input`, so a clone would be paid on every call.
///
/// The canonical key wins a conflict, matching `require_path_param`'s
/// long-standing preference — the alias entry is removed either way, so a
/// downstream reader can never see two spellings of one parameter.
pub fn normalize_params(input: &mut Value, aliases: AliasMap) -> Vec<Correction> {
    let Some(obj) = input.as_object_mut() else {
        return Vec::new();
    };
    let mut corrections = Vec::new();
    for (received, canonical) in aliases {
        let Some(value) = obj.remove(*received) else {
            continue;
        };
        let conflicted = obj.contains_key(*canonical);
        if !conflicted {
            obj.insert((*canonical).to_string(), value);
        }
        corrections.push(Correction {
            received: (*received).to_string(),
            canonical,
            conflicted,
        });
    }
    corrections
}

/// One-line notice for the response, or `None` when nothing was corrected.
///
/// Names the tool as well as the keys: the notice is read out of context, in a
/// response the caller may be scanning among several.
pub fn correction_notice(tool: &str, corrections: &[Correction]) -> Option<String> {
    if corrections.is_empty() {
        return None;
    }
    let parts: Vec<String> = corrections
        .iter()
        .map(|c| {
            if c.conflicted {
                format!(
                    "'{}' is not a parameter of {tool} — '{}' was also supplied and won; \
                     the '{}' value was ignored.",
                    c.received, c.canonical, c.received
                )
            } else {
                format!(
                    "'{}' is not a parameter of {tool} — corrected to '{}'. \
                     Use '{}' next time.",
                    c.received, c.canonical, c.canonical
                )
            }
        })
        .collect();
    Some(parts.join(" "))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const MAP: AliasMap = &[("file_path", "path"), ("relative_path", "path")];

    #[test]
    fn renames_an_alias_to_its_canonical_key() {
        let mut input = json!({ "file_path": "src/x.rs", "limit": 5 });
        let got = normalize_params(&mut input, MAP);
        assert_eq!(input["path"], json!("src/x.rs"));
        assert!(
            input.get("file_path").is_none(),
            "alias key must be removed"
        );
        assert_eq!(input["limit"], json!(5), "unrelated keys untouched");
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].received, "file_path");
        assert_eq!(got[0].canonical, "path");
        assert!(!got[0].conflicted);
    }

    #[test]
    fn canonical_wins_and_the_conflict_is_recorded() {
        let mut input = json!({ "path": "canon.rs", "file_path": "alias.rs" });
        let got = normalize_params(&mut input, MAP);
        assert_eq!(input["path"], json!("canon.rs"), "canonical value survives");
        assert!(input.get("file_path").is_none());
        assert_eq!(got.len(), 1);
        assert!(got[0].conflicted, "must record that a value was discarded");
    }

    #[test]
    fn no_aliases_present_is_a_no_op_and_allocates_no_corrections() {
        let mut input = json!({ "path": "src/x.rs" });
        let got = normalize_params(&mut input, MAP);
        assert_eq!(input, json!({ "path": "src/x.rs" }));
        assert!(got.is_empty());
    }

    #[test]
    fn a_non_object_input_is_left_alone() {
        let mut input = json!("not an object");
        let got = normalize_params(&mut input, MAP);
        assert_eq!(input, json!("not an object"));
        assert!(got.is_empty());
    }

    #[test]
    fn notice_names_the_tool_the_key_and_the_canonical_name() {
        let c = vec![Correction {
            received: "file_path".into(),
            canonical: "path",
            conflicted: false,
        }];
        let n = correction_notice("read_file", &c).expect("a correction must yield a notice");
        assert!(n.contains("file_path"), "must name what was received: {n}");
        assert!(n.contains("read_file"), "must name the tool: {n}");
        assert!(n.contains("path"), "must name the canonical key: {n}");
    }

    #[test]
    fn notice_for_a_conflict_says_the_value_was_ignored() {
        let c = vec![Correction {
            received: "file_path".into(),
            canonical: "path",
            conflicted: true,
        }];
        let n = correction_notice("read_file", &c).unwrap();
        assert!(
            n.contains("ignored"),
            "a discarded value must be stated, not implied: {n}"
        );
    }

    #[test]
    fn no_corrections_yields_no_notice() {
        assert!(correction_notice("read_file", &[]).is_none());
    }
}
