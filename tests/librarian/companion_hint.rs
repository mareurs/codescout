//! Guard tests for the companion-hint prompt.
//!
//! Mirrors the `prompt_surfaces_reference_only_real_tools` pattern in
//! `src/server.rs`: every token in the hint that has the *shape* of a codescout
//! tool reference must resolve to a really-registered tool. Catches stale
//! references when a tool is renamed or folded away.
//!
//! **Two defects this file previously had, both of which returned green.**
//!
//! 1. It was in no cargo target. `tests/librarian/` is a directory, and cargo
//!    auto-discovers `tests/*.rs` only — see `main.rs` beside this file.
//! 2. Its extractor filtered candidates to `REAL_TOOLS` and the assertion then
//!    checked membership in `REAL_TOOLS`, so the refuting observation was
//!    removed before it could be asserted on. Wiring that into the build would
//!    have bought nothing, which is why `extractor_can_surface_a_non_tool_token`
//!    below exists: it is the only test here that fails if the extractor is
//!    ever "simplified" back into a filter.
//!
//! The real tool set is read from the live registry rather than stored, so a
//! rename cannot leave a stale copy here to drift.

use std::collections::HashSet;

const COMPANION_HINT: &str = include_str!("../../src/librarian/prompts/companion_hint.md");

/// Names that have the shape of a codescout tool reference. Any identifier
/// built from one of these stems is either a real tool or drift — there are no
/// English words of this form, which is what lets the compound scan run over
/// the whole document instead of only inside backticks.
const TOOL_STEMS: &[&str] = &["artifact", "librarian", "doc"];

fn real_tools() -> HashSet<&'static str> {
    codescout::librarian::tools::all_tools()
        .iter()
        .map(|t| t.name())
        .collect()
}

/// Compound tool-shaped identifiers (`artifact_event`, `librarian_context`),
/// scanned over the WHOLE document — deliberately **not** backtick-scoped.
///
/// The surface this file mirrors is backtick-scoped and has a filed bug for it
/// (`docs/issues/2026-09-02-the-prompt-surface-gate-is-backtick-scoped-so-the-iron-laws-are-invisible-to-it.md`).
/// Inheriting that would have mattered here: `artifact_link` occurs in this very
/// prompt **unbackticked**, so a backtick-scoped extractor reports it clean.
fn compound_tool_tokens(s: &str) -> Vec<&str> {
    let re = regex::Regex::new(&format!(r"\b(?:{})_[a-z0-9_]+\b", TOOL_STEMS.join("|"))).unwrap();
    re.find_iter(s).map(|m| m.as_str()).collect()
}

/// Bare tool references, backtick-scoped by necessity: `artifact` and `doc` are
/// ordinary English words in prose ("an artifact is a markdown file"), so only
/// the backticked form is unambiguously a tool reference.
fn bare_backticked_tool_tokens(s: &str) -> Vec<&str> {
    let re = regex::Regex::new(&format!("`({})`", TOOL_STEMS.join("|"))).unwrap();
    re.captures_iter(s)
        .map(|c| c.get(1).unwrap().as_str())
        .collect()
}

/// Call/brace forms: `` `artifact(update, …)` ``, `` `artifact {action: …}` ``.
///
/// A third shape, because the first two do not reach it: the stem is followed
/// by `(` or `{` rather than by `_` or a closing backtick, so neither the
/// compound scan nor the bare-backtick scan matches. Both forms were live in
/// this prompt while the other two extractors reported it clean, which is the
/// same "one law, N implementation sites" trap as the guard itself.
fn call_form_tool_tokens(s: &str) -> Vec<&str> {
    let re = regex::Regex::new(&format!(r"`({})\s*[({{]", TOOL_STEMS.join("|"))).unwrap();
    re.captures_iter(s)
        .map(|c| c.get(1).unwrap().as_str())
        .collect()
}

/// The anti-tautology guard, and the reason the other tests here mean anything.
///
/// The previous extractor filtered its input to `REAL_TOOLS` and the assertion
/// then checked membership in `REAL_TOOLS` — true by construction, for any
/// input, forever. This test fails the moment an extractor stops being able to
/// return a token that the membership assertion would reject, so the refuting
/// observation is required to survive into the assertion.
#[test]
fn extractor_can_surface_a_non_tool_token() {
    let real = real_tools();
    let synthetic = "prose mentioning `artifact_bogus` and artifact_alsobogus and `doc`";

    let compound = compound_tool_tokens(synthetic);
    assert!(
        compound.contains(&"artifact_bogus") && compound.contains(&"artifact_alsobogus"),
        "compound extractor must surface non-tool tokens, backticked or not; got {compound:?}"
    );
    assert!(
        compound.iter().any(|t| !real.contains(t)),
        "extractor is filtering to the real-tool set — the assertions below are \
         then tautologies. Got {compound:?}"
    );
    assert!(
        bare_backticked_tool_tokens(synthetic).contains(&"doc"),
        "bare extractor must still find backticked stems"
    );

    let call_forms = "see `artifact(update, patch={})` and `artifact {action: \"find\"}`";
    let found = call_form_tool_tokens(call_forms);
    assert_eq!(
        found,
        vec!["artifact", "artifact"],
        "call/brace forms must be surfaced -- the other two extractors cannot \
         reach them, and both shapes were live in the prompt while this file \
         reported it clean"
    );
    assert!(
        found.iter().any(|t| !real.contains(t)),
        "call-form extractor must be able to return a non-tool token"
    );
}

#[test]
fn hint_names_only_real_tools() {
    let real = real_tools();
    let mut drift: Vec<String> = Vec::new();

    for tok in compound_tool_tokens(COMPANION_HINT) {
        if !real.contains(tok) {
            drift.push(format!("`{tok}` (compound)"));
        }
    }
    for tok in bare_backticked_tool_tokens(COMPANION_HINT) {
        if !real.contains(tok) {
            drift.push(format!("`{tok}` (bare)"));
        }
    }
    for tok in call_form_tool_tokens(COMPANION_HINT) {
        if !real.contains(tok) {
            drift.push(format!("`{tok}` (call form)"));
        }
    }
    drift.sort();
    drift.dedup();

    let mut registered: Vec<&str> = real.into_iter().collect();
    registered.sort_unstable();
    assert!(
        drift.is_empty(),
        "companion_hint.md names tools that are not registered: {}\n\
         registered tools are: {registered:?}",
        drift.join(", ")
    );
}

#[test]
fn hint_mentions_every_real_tool() {
    let mentioned: HashSet<&str> = bare_backticked_tool_tokens(COMPANION_HINT)
        .into_iter()
        .collect();
    for tool in real_tools() {
        assert!(
            mentioned.contains(tool),
            "companion_hint.md never mentions the registered tool `{tool}`"
        );
    }
}

#[test]
fn hint_is_not_empty() {
    assert!(!COMPANION_HINT.trim().is_empty());
    assert!(COMPANION_HINT.ends_with('\n'));
}
