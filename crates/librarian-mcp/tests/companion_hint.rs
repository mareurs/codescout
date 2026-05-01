//! Guard tests for the companion-hint prompt.
//!
//! Mirrors the codescout `prompt_surfaces_reference_only_real_tools` pattern:
//! every `artifact_*` / `librarian_*` token in the hint must correspond to a
//! real registered tool. Catches stale references at build time.

const COMPANION_HINT: &str = include_str!("../src/prompts/companion_hint.md");

const REAL_TOOLS: &[&str] = &[
    "artifact_list_by_kind",
    "artifact_find",
    "artifact_get",
    "artifact_links",
    "artifact_graph",
    "artifact_create",
    "artifact_update",
    "artifact_link",
    "artifact_observe",
    "librarian_context",
    "librarian_reindex",
    "artifact_augment",
    "artifact_update_params",
    "artifact_refresh",
    "artifact_refresh_commit",
    "tracker_create",
];

fn extract_tool_tokens(s: &str) -> Vec<&str> {
    s.split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
        .filter(|t| {
            t.starts_with("artifact_") || t.starts_with("librarian_") || t.starts_with("tracker_")
        })
        .collect()
}

#[test]
fn hint_mentions_only_real_tools() {
    for tok in extract_tool_tokens(COMPANION_HINT) {
        assert!(
            REAL_TOOLS.contains(&tok),
            "companion_hint.md mentions unknown tool: `{tok}`"
        );
    }
}

#[test]
fn hint_mentions_every_real_tool() {
    let tokens = extract_tool_tokens(COMPANION_HINT);
    for tool in REAL_TOOLS {
        assert!(
            tokens.contains(tool),
            "companion_hint.md does not mention real tool: `{tool}`"
        );
    }
}

#[test]
fn hint_is_not_empty() {
    assert!(!COMPANION_HINT.trim().is_empty());
    assert!(COMPANION_HINT.ends_with('\n'));
}
