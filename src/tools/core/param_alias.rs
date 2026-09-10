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

use std::collections::{HashMap, HashSet};

use serde_json::Value;

/// A non-canonical parameter name a caller sent, and what it was rewritten to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Correction {
    /// The key as received.
    pub received: String,
    /// The advertised key it was rewritten to.
    pub canonical: &'static str,
    /// True when the CALLER supplied the canonical key directly (present in
    /// `input` before normalization ran), so this alias's value was
    /// discarded in its favor. Reported separately from a plain rename
    /// because a silently dropped value is a different event.
    ///
    /// Mutually exclusive with `superseded_by`: this is about what the
    /// caller sent, snapshotted before the loop runs, never about what an
    /// earlier alias in this same call did.
    pub conflicted: bool,
    /// `Some(other_received)` when the caller never supplied the canonical
    /// key, but an earlier alias in this same call already claimed it — so
    /// this alias's value was discarded too, for a different reason than
    /// `conflicted`. `other_received` names the alias key that won.
    ///
    /// This state matters on its own: "two aliases naming the same
    /// canonical, no canonical supplied" is genuinely ambiguous input (which
    /// value did the caller mean?), not a rename — a distinction Task 3
    /// branches on to escalate it to `RecoverableError` for write tools per
    /// the ADR's write-asymmetry clause. Reporting it as `conflicted` would
    /// falsely tell the caller they supplied a key they never sent.
    pub superseded_by: Option<String>,
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
    // Snapshot which canonical keys the CALLER supplied, before this loop's
    // own insertions can change what `obj.contains_key` would report.
    // Checking presence live, inside the loop, conflates two different
    // states: "the caller supplied the canonical key" and "an earlier alias
    // in this same call already claimed it" — the latter would otherwise be
    // misreported as the former merely because of declaration order in
    // `aliases`. See `Correction::conflicted` / `Correction::superseded_by`.
    let supplied_canonical: HashSet<&'static str> = aliases
        .iter()
        .map(|(_, canonical)| *canonical)
        .filter(|canonical| obj.contains_key(*canonical))
        .collect();
    // Which alias (by received key) has already claimed a given canonical
    // key so far in this call — populated only for canonicals the caller
    // did NOT supply, since `supplied_canonical` already covers those.
    let mut claimed_by: HashMap<&'static str, String> = HashMap::new();
    let mut corrections = Vec::new();
    for (received, canonical) in aliases {
        let Some(value) = obj.remove(*received) else {
            continue;
        };
        let conflicted = supplied_canonical.contains(canonical);
        let superseded_by = if conflicted {
            None
        } else {
            claimed_by.get(canonical).cloned()
        };
        if !conflicted && superseded_by.is_none() {
            obj.insert((*canonical).to_string(), value);
            claimed_by.insert(canonical, (*received).to_string());
        }
        corrections.push(Correction {
            received: (*received).to_string(),
            canonical,
            conflicted,
            superseded_by,
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
            // LOAD-BEARING: the SINGLE quotes around every key and value below.
            // A test in `src/tools/core/tests.rs` asserts that Site B (the
            // compact-text render) carries this text verbatim, AND separately
            // that it carries no `{`. Those two assertions only cover opposite
            // directions because single quotes survive `serde_json`
            // string-encoding byte-identically: a regression that dumps the
            // serialized `corrections` object into the text renderer still
            // contains this notice verbatim, so the exact-match assertion stays
            // green and only the `{` check reds. Switch these to double quotes
            // and serde escapes them, the dump no longer contains the notice,
            // both assertions red together, and the comment explaining why both
            // exist becomes false while the suite stays green.
            if c.conflicted {
                format!(
                    "'{}' is not a parameter of {tool} — '{}' was also supplied and won; \
                     the '{}' value was ignored.",
                    c.received, c.canonical, c.received
                )
            } else if let Some(winner) = &c.superseded_by {
                format!(
                    "'{}' is not a parameter of {tool} — '{}' already set '{}' earlier in \
                     this call, which won; the '{}' value was ignored.",
                    c.received, winner, c.canonical, c.received
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
        assert!(got[0].superseded_by.is_none());
    }

    #[test]
    fn canonical_wins_and_the_conflict_is_recorded() {
        let mut input = json!({ "path": "canon.rs", "file_path": "alias.rs" });
        let got = normalize_params(&mut input, MAP);
        assert_eq!(input["path"], json!("canon.rs"), "canonical value survives");
        assert!(input.get("file_path").is_none());
        assert_eq!(got.len(), 1);
        assert!(got[0].conflicted, "must record that a value was discarded");
        assert!(
            got[0].superseded_by.is_none(),
            "conflicted and superseded_by must never both be set"
        );
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

    /// The doc-comment claim for `conflicted` is "the CALLER supplied the
    /// canonical key" — a state decided at entry, not by declaration order
    /// in `MAP`. Two aliases racing for one canonical, with the caller
    /// never sending it at all, must NOT be reported as a conflict: the
    /// second alias is superseded by the first, a different state.
    #[test]
    fn two_aliases_no_canonical_supplied_the_later_one_is_superseded_not_conflicted() {
        let mut input = json!({ "file_path": "a.rs", "relative_path": "b.rs" });
        let got = normalize_params(&mut input, MAP);
        assert_eq!(input["path"], json!("a.rs"), "the first alias's value wins");
        assert_eq!(got.len(), 2);

        assert!(
            !got[0].conflicted && got[0].superseded_by.is_none(),
            "the first alias to claim an unsupplied canonical is a plain rename"
        );

        assert!(
            !got[1].conflicted,
            "the caller never sent 'path' directly — this must not be reported \
             as though they had"
        );
        assert_eq!(
            got[1].superseded_by.as_deref(),
            Some("file_path"),
            "must name which earlier alias claimed the canonical first"
        );

        let n = correction_notice("read_file", &got).unwrap();
        assert!(
            !n.contains("also supplied"),
            "the caller never supplied 'path'; the notice must not claim they did: {n}"
        );
        assert!(
            n.contains("already set"),
            "must positively name which alias won, not merely avoid the false \
             claim above — an absence check alone can't see the winner's name \
             going missing too: {n}"
        );
    }

    /// The companion state to the test above: here the caller DID supply the
    /// canonical directly, alongside one alias. This must be reported as
    /// `conflicted`, never as `superseded_by` (which is reserved for the
    /// no-canonical-supplied case), and the notice must name the real
    /// conflict.
    #[test]
    fn canonical_plus_one_alias_is_conflicted_and_never_superseded() {
        let mut input = json!({ "path": "canon.rs", "relative_path": "alias.rs" });
        let got = normalize_params(&mut input, MAP);
        assert_eq!(got.len(), 1);
        assert!(
            got[0].conflicted,
            "the caller supplied the canonical key directly"
        );
        assert!(
            got[0].superseded_by.is_none(),
            "conflicted and superseded_by must never both be set"
        );

        let n = correction_notice("read_file", &got).unwrap();
        assert!(
            n.contains("was also supplied"),
            "a real conflict with the caller's own canonical key must say so: {n}"
        );
    }

    #[test]
    fn notice_names_the_tool_the_key_and_the_canonical_name() {
        let c = vec![Correction {
            received: "file_path".into(),
            canonical: "path",
            conflicted: false,
            superseded_by: None,
        }];
        let n = correction_notice("read_file", &c).expect("a correction must yield a notice");
        assert!(n.contains("file_path"), "must name what was received: {n}");
        assert!(n.contains("read_file"), "must name the tool: {n}");
        assert!(
            n.contains("'path'"),
            "must name the canonical key, not merely a substring the received \
             key itself already contains (e.g. 'file_path' contains \"path\"): {n}"
        );
        assert!(
            !n.contains("ignored"),
            "a plain rename must not tell the agent a value was ignored — that \
             is only true of a real conflict: {n}"
        );
    }

    #[test]
    fn notice_for_a_conflict_says_the_value_was_ignored() {
        let c = vec![Correction {
            received: "file_path".into(),
            canonical: "path",
            conflicted: true,
            superseded_by: None,
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
