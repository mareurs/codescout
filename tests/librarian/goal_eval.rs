//! Tier 3 eval harness: runs the goal augmentation prompt against each
//! fixture × checkpoint and scores the result. Marked #[ignore] — run on demand
//! with `cargo test --test goal_eval -- --ignored`.
//!
//! Requires an Anthropic API key for synthesis (set via ANTHROPIC_API_KEY).
//!
//! TODO(Task 11): wire `synthesize()` to a real Anthropic SDK call. The stub
//! returns input unchanged so the harness compiles cleanly today; the deferred
//! Phase 4 task (Task 11 in the goal-tracker plan) will pick the SDK and
//! implement the request. Use Haiku 4.5 (`claude-haiku-4-5-20251001`) to match
//! the Stop hook's default model.

#[path = "goal_eval/rubric.rs"]
mod rubric;

use rubric::{score, Checkpoint};
use serde_json::Value;

/// (slug, expected T2 status, commits considered part of this goal's real history).
/// Replace placeholder commit hashes (e.g. `abc1234`) with real commits before
/// expecting the `no_fabrication` rubric to grade truthfully.
const GOALS: &[(&str, &str, &[&str])] = &[
    (
        "goal_01_phase6_provider_lifts",
        "done",
        &["f7ca520", "59b3f63d"],
    ),
    ("goal_02_retrieval_p5", "done", &["f651cef5"]),
    (
        "goal_03_tools_mod_refactor",
        "done",
        &["1fc60c4", "ba9fe16"],
    ),
    (
        "goal_04_kotlin_lsp_mux",
        "done",
        &["c2658f1b", "1c152030", "e8855098", "d662a30c", "0926842e"],
    ),
    ("goal_05_augmentation_postfix", "done", &["69d09851"]),
];

fn read_fixture(goal_slug: &str, cp: Checkpoint) -> Value {
    let cp_name = match cp {
        Checkpoint::T0 => "t0",
        Checkpoint::T1 => "t1",
        Checkpoint::T2 => "t2",
    };
    let path = format!("tests/librarian/goal_eval/fixtures/{goal_slug}/{cp_name}.json");
    let content =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("missing fixture {path}: {e}"));
    serde_json::from_str(&content).unwrap_or_else(|e| panic!("invalid JSON in {path}: {e}"))
}

/// Synthesize new params by running the goal augmentation prompt.
///
/// **STUB** — Task 11 (deferred) will:
///   1. Read the goal archetype's `prompt_template` from `archetype_goal()`.
///   2. Send a request to Anthropic (Haiku 4.5: `claude-haiku-4-5-20251001`)
///      with the prompt + the fixture params + any necessary child state.
///   3. Parse the model's response as the new params object.
///
/// Returning input unchanged is the conservative scaffold: every `correct_status`
/// rubric will fail at T2 (status stays at fixture's value, not advancing to "done"),
/// providing a visible signal that the synthesizer is not yet wired up.
async fn synthesize(_prompt: &str, params: &Value) -> Value {
    params.clone()
}

#[tokio::test]
#[ignore = "eval — run manually with --ignored after API key set + synthesize() wired"]
async fn tier3_goal_eval() {
    use codescout::librarian::tools::tracker_design;

    let archetypes = tracker_design::archetypes();
    let goal_arch = archetypes
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["name"] == "goal")
        .expect("goal archetype not registered");
    let prompt = goal_arch["prompt_template"].as_str().unwrap();

    let mut goal_pass_count = 0;
    let total = GOALS.len();

    for (slug, expected_t2_status, commits) in GOALS {
        let mut all_cp_pass = true;
        for cp in [Checkpoint::T0, Checkpoint::T1, Checkpoint::T2] {
            let before = read_fixture(slug, cp);
            let after = synthesize(prompt, &before).await;
            let expected_status = match cp {
                Checkpoint::T0 => "scoping",
                Checkpoint::T1 => "active",
                Checkpoint::T2 => expected_t2_status,
            };
            let s = score(cp, &before, &after, expected_status, commits);
            if !s.passed() {
                eprintln!("FAIL {slug} {cp:?}: {s:?}");
                all_cp_pass = false;
            }
        }
        if all_cp_pass {
            goal_pass_count += 1;
        }
    }

    println!("Tier 3 eval: {goal_pass_count}/{total} goals passed");
    assert!(
        goal_pass_count >= 4,
        "Tier 3 eval gate: need ≥4 of {total} goals to pass; got {goal_pass_count}. \
         Iterate the augmentation prompt and re-run."
    );
}
