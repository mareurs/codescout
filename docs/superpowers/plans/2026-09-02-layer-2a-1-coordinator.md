# Layer 2a, Plan 1 — The Coordinator Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the fan-out that turns the Layer 1 registry into the thing deciding what a tool response carries — ordering, corpus exclusivity, and the idle re-arm — proven against synthetic engines, with no production call site touched.

**Architecture:** `src/engines/coordinator.rs` gains `PostCtx`, `Emission`, `Emitted` and `run_post_in`/`run_post`. `EngineDecl` gains an `emit_post` function pointer, `None` on all four engines for now. Nothing calls any of it: Plan 2 supplies the real emitters, Plan 3 wires production.

**Tech Stack:** Rust (edition per `Cargo.toml`), `rmcp::model::Content`, `serde_json::Value`, `parking_lot::Mutex`.

**Spec:** [`../specs/2026-09-02-retrieval-engine-coordination-design.md`](../specs/2026-09-02-retrieval-engine-coordination-design.md) — Rollout step 2a, § *Layer 2 — Coordinator: two phases, one of them total*.

**Plan sequence:** **1 — coordinator (this)** → [2 — emitters](2026-09-02-layer-2a-2-emitters.md) → [3 — wiring and one budget](2026-09-02-layer-2a-3-wiring-and-one-budget.md)
## Global Constraints

- **The gate, all four, in this order** — the order is load-bearing and the two test lanes are chained with `;`, **never** `&&`:
  ```
  cargo fmt
  cargo clippy --workspace --all-targets --features local-embed -- -D warnings
  cargo test --workspace --no-default-features ; cargo test --workspace
  ```
  Read the exit codes; do not let a red lean lane short-circuit the default lane, which is what rebuilds `target/debug/codescout` for the next session.
- **Byte-identical is the whole point of the 2a sequence.** No response's bytes may change. In this plan that is trivially true — no production call site is touched — and the tests you write are what makes it checkable in Plan 3.
- **A worktree exists at `.worktrees/tool-collapse`.** Bare `git commit` is blocked by a hook. Use `git -C /home/marius/work/claude/codescout commit ...`.
- **Shared checkout, several concurrent sessions.** Never `git add -A`. Stage by pathspec, then `git diff --cached --name-only` (the index is shared), then `git diff --cached` and read it, then commit by pathspec. A pre-commit hook refuses a pathspec commit carrying unstaged content.
- **Errors:** `RecoverableError` for anything a caller can fix, `anyhow::bail!` for invariant breaches. Full tree: `get_guide("error-handling")`. Nothing in this plan should need either — the coordinator is infallible by construction.
- Commit messages end with `Co-Authored-By:` and `Session-Id:` trailers, matching the repo's existing log.

---

## File Structure

| file | responsibility |
|---|---|
| **Create** `src/engines/coordinator.rs` | `PostCtx`, `Emission`, `Emitted`, `run_post_in`, `run_post`. Ordering, corpus exclusivity, and the TTL tick. Knows nothing about guides, rules, or the `Tool` trait. |
| **Create** `src/engines/emitters.rs` | The three engine bodies, moved verbatim from `call_content`. Knows nothing about ordering. |
| **Modify** `src/engines/mod.rs` | Add `emit_post` to `EngineDecl`; reorder `ENGINES`; declare the two new modules. |

The split is by responsibility, not by layer: a reviewer can reject the ordering semantics while approving the emitters, or the reverse.

---

## Background the implementer needs

Read these before Task 1. They are the source of every design decision below.

**The current fan-out is `src/tools/core/types.rs:836-1235`.** Three things about it are easy to get wrong:

1. **The session opener and guide-sections are MUTUALLY EXCLUSIVE, deliberately.** They are an `if !emitted.contains(SESSION_OPENING_GUIDE) { … } else if let Some(t) = self.relevant_guide_topic(&val) { … }` chain. The comment at `types.rs:968` argues the trade explicitly: *"The cost is a one-response delay, and a tool called exactly once in a session forfeits its guide — an acceptable trade against 18 KB of librarian guide landing on a one-shot `artifact` call."* A naive "for each engine, emit and concatenate" loop would deliver both and is **not** byte-identical.

2. **The opener CLAIMS even when it emits nothing.** If `guide_block(SESSION_OPENING_GUIDE)` returns `None`, the chain yields `(None, Vec::new())` — it does **not** fall through to guide-sections. That is unreachable today (the topic is always registered) and must survive the move anyway, which is why `Emitted` has three states rather than two.

3. **The guide path produces a side-channel, not just blocks.** `guide_hint: Option<(String, GuideDeliveryShape)>` feeds `inject_hint`, which mutates the **primary** block's JSON (`_guide_hint`). There is one such field, so at most one hint per response. `op_content` has no equivalent — it is append-only.

**Ordering today** is `[primary, ...guide_content, ...op_content]` (`types.rs:1230-1233`), and within the guide path the opener is tried **before** guide-sections. `ENGINES` currently lists `guide-sections` first, so the registry order and the behavioural order disagree. Plan 2 fixes that by reordering the registry — which is the spec's promise that *"ordering becomes registry order, and therefore reviewable"* actually cashing out.

**APIs you will call** (verified 2026-09-02):

```rust
// src/tools/guide_ledger.rs
pub fn contains(&self, topic: &str) -> bool;
pub fn insert(&mut self, topic: String) -> bool;   // return value unused at every call site
pub fn tick(&mut self) -> usize;                   // anonymous-tier idle re-arm; 0 when no TTL

// src/tools/core/guide_emit.rs
pub(crate) fn guide_block(topic: &str) -> Option<Content>;
pub(crate) fn guide_blocks_for(
    topic: &str, selector: Option<&str>, result: &Value,
    emitted: &mut GuideLedger,
) -> (Vec<Content>, Option<GuideDeliveryShape>);

// src/operator_rules/route.rs
pub fn route(sel: Option<&str>, result: &Value) -> Vec<&'static Rule>;
pub fn ledger_key(id: &str) -> String;

// src/prompts/guide_index.rs
GUIDE_INDEX.topic_declaring(sel: Option<&str>, result: &Value) -> Option<&'static str>
```

`guide_emit`'s helpers are `pub(crate)`, so `src/engines/emitters.rs` can call them unchanged.

---

## Task 1: The coordinator — ordering and corpus exclusivity, over synthetic engines

**Files:**
- Create: `src/engines/coordinator.rs`
- Modify: `src/engines/mod.rs` (add `emit_post` field, declare the module)
- Test: inline `#[cfg(test)] mod tests` in `src/engines/coordinator.rs`

**Interfaces:**
- Consumes: `Corpus` and `EngineDecl` from `src/engines/mod.rs`; `GuideLedger`.
- Produces: `PostCtx<'a>`, `Emission`, `Emitted`, `run_post_in(&[EngineDecl], &PostCtx, &mut GuideLedger) -> Emission`, `run_post(&PostCtx, &mut GuideLedger) -> Emission`. Plans 2 and 3 both depend on these exact names and types.

- [ ] **Step 1: Add the `emit_post` field to `EngineDecl`, `None` on all four engines**

In `src/engines/mod.rs`, add to the struct:

```rust
    /// This engine's post-phase emitter, or `None` while it is still inlined
    /// in `call_content`. A function pointer rather than a trait object
    /// because an engine is data, not behaviour with state — and because a
    /// `&'static EngineDecl` must stay `Sync` without a `Box`.
    pub emit_post: Option<fn(&crate::engines::coordinator::PostCtx<'_>,
                             &mut crate::tools::guide_ledger::GuideLedger)
                          -> crate::engines::coordinator::Emitted>,
```

Add `emit_post: None,` to each of the four `EngineDecl` literals in `ENGINES`, and add above the module's `use`:

```rust
pub mod coordinator;
```

- [ ] **Step 2: Write the failing tests**

Create `src/engines/coordinator.rs` containing ONLY the test module below plus `use super::*;`. It will not compile yet — that is the point.

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::engines::{Corpus, EngineDecl, Mode, RetrievalKey};
    use crate::tools::guide_ledger::GuideLedger;
    use rmcp::model::Content;
    use serde_json::json;

    fn owns_nothing(_k: &str) -> bool { false }

    fn decl(id: &'static str, corpus: Corpus,
            emit: fn(&PostCtx<'_>, &mut GuideLedger) -> Emitted) -> EngineDecl {
        EngineDecl {
            id, key: RetrievalKey::CallShape, corpus, mode: Mode::Push,
            writes_at: &[], owns_key: owns_nothing, emit_post: Some(emit),
        }
    }

    fn claims_a_block(_c: &PostCtx<'_>, _l: &mut GuideLedger) -> Emitted {
        Emitted::Claimed(Emission {
            hint: Some(("first".into(), GuideDeliveryShape::Whole)),
            blocks: vec![Content::text("FIRST")],
        })
    }
    fn claims_nothing(_c: &PostCtx<'_>, _l: &mut GuideLedger) -> Emitted {
        Emitted::Claimed(Emission::default())
    }
    fn declines(_c: &PostCtx<'_>, _l: &mut GuideLedger) -> Emitted { Emitted::Declined }
    fn claims_second(_c: &PostCtx<'_>, _l: &mut GuideLedger) -> Emitted {
        Emitted::Claimed(Emission {
            hint: Some(("second".into(), GuideDeliveryShape::Whole)),
            blocks: vec![Content::text("SECOND")],
        })
    }

    fn ctx<'a>(v: &'a serde_json::Value) -> PostCtx<'a> {
        PostCtx { selector: Some("t.a"), value: v, content_topic: None, overflowing: false }
    }

    fn texts(e: &Emission) -> Vec<String> {
        e.blocks.iter().filter_map(|b| b.as_text().map(|t| t.text.clone())).collect()
    }

    /// The load-bearing one. A CLAIMED-but-empty engine still spends its
    /// corpus: the pre-refactor `if/else` returned `(None, Vec::new())` when
    /// the opener's `guide_block` came back `None`, rather than falling
    /// through to guide-sections. Mutating `Emitted::Claimed(e) if
    /// e.is_empty() => continue` reds this test and only this test.
    #[test]
    fn a_claim_spends_its_corpus_even_when_it_emits_nothing() {
        let v = json!({});
        let engines = [
            decl("empty-claimer", Corpus::CompiledGuides, claims_nothing),
            decl("would-emit", Corpus::CompiledGuides, claims_a_block),
        ];
        let out = run_post_in(&engines, &ctx(&v), &mut GuideLedger::default());
        assert!(texts(&out).is_empty(), "got {:?}", texts(&out));
        assert!(out.hint.is_none());
    }

    #[test]
    fn a_decline_passes_the_corpus_to_the_next_engine() {
        let v = json!({});
        let engines = [
            decl("decliner", Corpus::CompiledGuides, declines),
            decl("would-emit", Corpus::CompiledGuides, claims_a_block),
        ];
        let out = run_post_in(&engines, &ctx(&v), &mut GuideLedger::default());
        assert_eq!(texts(&out), vec!["FIRST".to_string()]);
    }

    #[test]
    fn engines_in_different_corpora_both_emit_in_registry_order() {
        let v = json!({});
        let engines = [
            decl("guides", Corpus::CompiledGuides, claims_a_block),
            decl("rules", Corpus::OperatorLedger, claims_second),
        ];
        let out = run_post_in(&engines, &ctx(&v), &mut GuideLedger::default());
        assert_eq!(texts(&out), vec!["FIRST".to_string(), "SECOND".to_string()]);
    }

    /// The primary block carries exactly one `_guide_hint` field, so a second
    /// hint has nowhere to go. First claimant wins; later hints are dropped
    /// rather than overwriting.
    #[test]
    fn only_the_first_hint_survives() {
        let v = json!({});
        let engines = [
            decl("guides", Corpus::CompiledGuides, claims_a_block),
            decl("rules", Corpus::OperatorLedger, claims_second),
        ];
        let out = run_post_in(&engines, &ctx(&v), &mut GuideLedger::default());
        assert_eq!(out.hint.map(|(t, _)| t), Some("first".to_string()));
    }

    /// An engine with no emitter is skipped, not treated as a decline that
    /// claims nothing — `craft-skills` is `Mode::Unmanaged` and must not be
    /// able to spend a corpus it does not draw from.
    #[test]
    fn an_engine_without_an_emitter_is_skipped() {
        let v = json!({});
        let mut unwired = decl("unwired", Corpus::CompiledGuides, claims_a_block);
        unwired.emit_post = None;
        let engines = [unwired, decl("real", Corpus::CompiledGuides, claims_second)];
        let out = run_post_in(&engines, &ctx(&v), &mut GuideLedger::default());
        assert_eq!(texts(&out), vec!["SECOND".to_string()]);
    }
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test --lib engines::coordinator 2>&1 | tail -20`
Expected: compile errors — `PostCtx`, `Emission`, `Emitted`, `run_post_in` not found.

- [ ] **Step 4: Write the implementation**

Prepend to `src/engines/coordinator.rs`, above the test module:

```rust
//! The fan-out that decides what a tool response carries, Layer 2 of
//! `docs/superpowers/specs/2026-09-02-retrieval-engine-coordination-design.md`.
//!
//! Post phase only. This module owns **ordering**, **corpus exclusivity** and
//! the **idle re-arm tick**; it owns no knowledge of guides, rules, or the
//! `Tool` trait. Each engine's body lives in [`super::emitters`].

use super::{Corpus, EngineDecl, ENGINES};
use crate::tools::core::guide_emit::GuideDeliveryShape;
use crate::tools::guide_ledger::GuideLedger;
use rmcp::model::Content;
use serde_json::Value;

/// Everything a post-phase engine may read about the call.
///
/// `content_topic` is resolved by the caller rather than by asking the tool,
/// for two reasons: the coordinator must not depend on the `Tool` trait, and
/// a default trait method cannot coerce `&self` to `&dyn Tool` anyway.
pub struct PostCtx<'a> {
    pub selector: Option<&'a str>,
    pub value: &'a Value,
    /// `Tool::relevant_guide_topic(value)`.
    pub content_topic: Option<&'a str>,
    /// The primary block will overflow into a `@tool_*` buffer, or the tool
    /// pre-buffered and returned an `output_id`. Precomputed because deciding
    /// it requires the serialised JSON, which the coordinator does not hold.
    pub overflowing: bool,
}

/// One engine's contribution to one response.
#[derive(Default)]
pub struct Emission {
    /// Drives the single legacy `_guide_hint` field on the primary block.
    /// There is exactly one such field, so at most one hint survives a
    /// response — see `run_post_in`.
    pub hint: Option<(String, GuideDeliveryShape)>,
    pub blocks: Vec<Content>,
}

impl Emission {
    pub fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }
}

/// Whether an engine's trigger fired — which is **not** the same question as
/// whether it produced bytes.
///
/// The distinction is load-bearing and pre-dates this module. The session
/// opener's pre-refactor branch returned `(None, Vec::new())` when its topic
/// failed to build, rather than falling through to guide-sections: a fired
/// trigger that ships nothing still spends the call. Collapsing these two into
/// `Vec<Content>` would silently restore the fall-through.
pub enum Emitted {
    /// Trigger did not fire. Later engines in the same corpus may still run.
    Declined,
    /// Trigger fired. No later engine in the same corpus runs, even if this
    /// emission is empty.
    Claimed(Emission),
}

/// `run_post`'s logic, generic over the engine slice.
///
/// Split out so the ordering rules can be exercised against synthetic engines
/// — the live registry cannot produce a claimed-but-empty first engine, and
/// that is exactly the case worth pinning. Same idiom, same reason, as
/// `operator_rules::route::route_in`.
pub fn run_post_in(
    engines: &[EngineDecl],
    ctx: &PostCtx<'_>,
    ledger: &mut GuideLedger,
) -> Emission {
    // Anonymous-tier idle re-arm, coordinator-level and FIRST. The session
    // opener's trigger reads the ledger this may re-arm, so running the tick
    // inside any one engine would order it against that engine alone.
    let rearmed = ledger.tick();
    if rearmed > 0 {
        tracing::debug!("anonymous guide ledger idle TTL re-armed {rearmed} topic(s)");
    }

    let mut out = Emission::default();
    let mut claimed: Vec<Corpus> = Vec::new();
    for engine in engines {
        let Some(emit) = engine.emit_post else {
            continue;
        };
        if claimed.contains(&engine.corpus) {
            continue;
        }
        let Emitted::Claimed(e) = emit(ctx, ledger) else {
            continue;
        };
        claimed.push(engine.corpus);
        if out.hint.is_none() {
            out.hint = e.hint;
        }
        out.blocks.extend(e.blocks);
    }
    out
}

/// The live fan-out, bound to [`ENGINES`].
pub fn run_post(ctx: &PostCtx<'_>, ledger: &mut GuideLedger) -> Emission {
    run_post_in(ENGINES, ctx, ledger)
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test --lib engines::coordinator`
Expected: 5 passed.

If `GuideLedger::default()` does not exist, use whatever constructor `src/tools/guide_ledger.rs` exposes for an empty ledger (check `grep -n "fn new\|impl Default" src/tools/guide_ledger.rs`) and use it consistently in all five tests.

- [ ] **Step 6: Verify the load-bearing test actually discriminates**

Temporarily change the claim arm in `run_post_in` to skip empty claims:

```rust
        let Emitted::Claimed(e) = emit(ctx, ledger) else { continue };
        if e.is_empty() { continue; }          // <-- the mutation
```

Run: `cargo test --lib engines::coordinator`
Expected: `a_claim_spends_its_corpus_even_when_it_emits_nothing` FAILS and the other four pass. **Revert the mutation.** If any other test also fails, the mutation is too coarse — say so in the commit rather than proceeding.

- [ ] **Step 7: Run the gate**

```
cargo fmt
cargo clippy --workspace --all-targets --features local-embed -- -D warnings
cargo test --workspace --no-default-features ; cargo test --workspace
```

Expected: clean. `peer::server::tests::run_exits_after_idle_timeout_with_no_connections` is a filed load-sensitive flake (`docs/issues/2026-09-01-peer-idle-timeout-test-is-the-third-load-sensitive-step.md`) — if it is the only failure, re-run it in isolation and record both results rather than dismissing it.

- [ ] **Step 8: Commit**

```bash
git -C /home/marius/work/claude/codescout add src/engines/coordinator.rs src/engines/mod.rs
git -C /home/marius/work/claude/codescout diff --cached --name-only   # index is SHARED — confirm these are yours
git -C /home/marius/work/claude/codescout diff --cached               # read it
git -C /home/marius/work/claude/codescout commit -m "feat(engines): a coordinator that can tell a fired trigger from an emitted block" -- src/engines/coordinator.rs src/engines/mod.rs
```

Body should state the mutation result from Step 6 and name the test it killed.

---

## Self-review notes for the executor

- **Nothing in this plan changes production output.** If any pre-existing test's expectations change, stop — that is evidence the move was not verbatim.
- **The `Emitted` three-state enum is the one thing worth re-reading before you simplify it.** Two independent reviewers have wanted to collapse it into `Vec<Content>`; the reason it cannot be is in `Emitted`'s doc comment and in `a_claim_spends_its_corpus_even_when_it_emits_nothing`.
- **`GuideLedger::default()` is assumed.** Task 1 Step 5 tells you what to do if it does not exist; apply the same substitution everywhere else in this plan.
- **Visibility hazard: `pub struct Emission` holds a `GuideDeliveryShape`.** If that enum is `pub(crate)` in `src/tools/core/guide_emit.rs`, the gate's `-D warnings` will fire `private_interfaces` on `Emission::hint`. Fix by narrowing — make `Emission`, `Emitted`, `PostCtx` and both `run_post*` functions `pub(crate)` — rather than by widening `GuideDeliveryShape` to `pub`. Nothing outside this crate consumes the coordinator, and widening a type to satisfy a lint exports an API nobody asked for.
