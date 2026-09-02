# Layer 2a, Plan 2 — The Emitters Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move the three real engines' post-phase bodies out of `Tool::call_content` into `src/engines/emitters.rs` unchanged, wire them onto the registry, and make registry order carry the delivery precedence that today hides inside an `if/else` chain.

**Architecture:** Each emitter answers one question — *"does my trigger fire on this call, and if so what do I ship?"* — and answers nothing about ordering, which belongs to Plan 1's coordinator. `ENGINES` is reordered so `session-opener` precedes `guide-sections`, matching the behavioural order `call_content` has always had. **Production still runs the old inlined path**; Plan 3 does the swap.

**Tech Stack:** Rust (edition per `Cargo.toml`), `rmcp::model::Content`, `serde_json::Value`.

**Spec:** [`../specs/2026-09-02-retrieval-engine-coordination-design.md`](../specs/2026-09-02-retrieval-engine-coordination-design.md) — Rollout step 2a, § *Layer 2 — Coordinator: two phases, one of them total*.

**Plan sequence:** [1 — coordinator](2026-09-02-layer-2a-1-coordinator.md) → **2 — emitters (this)** → [3 — wiring and one budget](2026-09-02-layer-2a-3-wiring-and-one-budget.md). **Plan 1 must be complete and committed first** — this plan's emitters have `Emitted` in their signature.
## Global Constraints

- **The gate, all four, in this order** — the order is load-bearing and the two test lanes are chained with `;`, **never** `&&`:
  ```
  cargo fmt
  cargo clippy --workspace --all-targets --features local-embed -- -D warnings
  cargo test --workspace --no-default-features ; cargo test --workspace
  ```
  Read the exit codes; do not let a red lean lane short-circuit the default lane, which is what rebuilds `target/debug/codescout` for the next session.

  **On `cargo fmt` and this shared checkout — the obvious scoping does not work.** Six other sessions hold uncommitted edits here, and bare `cargo fmt` is a *write* across every crate root in the workspace, so it will reformat their in-flight files. The natural remedy is wrong: **`cargo fmt -- <your file>` scopes nothing.** Verified 2026-09-02 with `cargo fmt -v`, cargo-fmt emits a single invocation — `rustfmt --edition 2021 <your file> --check <build.rs> <both libs> <main.rs> <24 test roots>` — appending the full crate-root list *after* your path. It is a whole-workspace format plus one redundant argument.

  Two forms are actually safe. `cargo fmt --check` in any arrangement is read-only and writes nothing. `rustfmt --edition 2021 <your files>`, invoked **directly**, bypasses cargo's target enumeration and touches only what you name. Use the direct `rustfmt` call when you need to write, and name the form you ran in your report — a substituted instrument reported under the mandated one's name is invisible exactly when it is benign.
- **Byte-identical is the whole point of the 2a sequence.** No response's bytes may change. In this plan that is trivially true — no production call site is touched — and the tests you write are what makes it checkable in Plan 3.
- **A worktree exists at `.worktrees/tool-collapse`.** Bare `git commit` is blocked by a hook. Use `git -C /home/marius/work/claude/codescout commit ...`.
- **Shared checkout, several concurrent sessions.** Never `git add -A`. Stage by pathspec, then `git diff --cached --name-only` (the index is shared), then `git diff --cached` and read it, then commit by pathspec. A pre-commit hook refuses a pathspec commit carrying unstaged content.

  **If the shared index holds paths you did not stage, leave them exactly as they are.** Commit by pathspec — it ignores the index for paths it does not name, so foreign staged paths cannot reach your commit and need no removal. Do **not** run `git restore --staged`, `git reset`, or anything else that unstages them. Measured 2026-09-02 on this very sequence: an implementer found three of another session's `docs/issues/*.md` staged alongside its own file, correctly refused to commit them, and then unstaged them — which bought nothing (the pathspec commit had already excluded them) and destroyed a peer's staging intent. Working trees were never at risk, but a peer who had staged files and stepped away would have returned to an empty index with no explanation. Report a foreign index; never repair one.
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

## Task 1: The three real emitters, and the registry order that drives them

**Files:**
- Create: `src/engines/emitters.rs`
- Modify: `src/engines/mod.rs` (declare the module; wire `emit_post` on three engines; reorder `ENGINES`)
- Test: inline `#[cfg(test)] mod tests` in `src/engines/emitters.rs`, plus one order test in `src/engines/mod.rs`

**Interfaces:**
- Consumes: `PostCtx`, `Emission`, `Emitted` from Plan 1.
- Produces: `emit_session_opener`, `emit_guide_sections`, `emit_operator_rules`, each `fn(&PostCtx<'_>, &mut GuideLedger) -> Emitted`. Plan 3 relies on `ENGINES` being wired with all three and on the registry order below.

> **Controller ruling — execute Step 4 before Step 3.** The steps below are written test-first, but wiring `emit_post: Some(emitters::…)` and `pub mod emitters;` into `src/engines/mod.rs` *before* `emitters.rs` exists leaves the whole crate non-compiling for every session sharing this checkout. Write `src/engines/emitters.rs` complete first (Step 4): an untracked `.rs` file that no `mod` declaration references is not compiled at all, so it costs peers nothing. Then do Step 3's reorder-and-wire as one edit. Step 1's order test still fails for the right reason — it is an *assertion* failure on `ENGINES`' contents, not a compile error — so nothing about the TDD cycle is lost. Measured cost of getting this wrong on 2026-09-02: two peer sessions independently diagnosed a red tree that was not theirs.

> **Controller ruling — Plan 1's dead-code suppressions are `#[expect]`, not `#[allow]`, and they are self-cleaning.** Plan 1 shipped `#[expect(dead_code, reason=…)]` and `#[cfg_attr(not(test), expect(dead_code, reason=…))]` on seven items in `coordinator.rs` and `mod.rs`. Wiring the emitters makes several of them live — `Emitted::Declined` and `Emitted::Claimed` become constructed, `PostCtx`'s fields become read — at which point the compiler raises `unfulfilled_lint_expectations`, which `-D warnings` promotes to an error. **That is the mechanism working, not a regression.** When clippy names an unfulfilled expectation, **delete that attribute**. Never widen it, never convert it to `#[allow]`, and never re-add it: the whole point is that the suppression cannot outlive the condition that justified it. Expect to remove some here and the rest in Plan 3.

- [ ] **Step 1: Write the failing order test**

Add to `src/engines/mod.rs`'s test module:

```rust
    /// Registry order IS delivery precedence, and the two must not drift.
    ///
    /// The session opener precedes guide-sections because the pre-refactor
    /// `if/else` in `call_content` tried it first — deliberately, so a
    /// one-shot `artifact` call receives the 2.5 KB opener rather than 18 KB
    /// of librarian guide (`types.rs:968`). Both draw `Corpus::CompiledGuides`,
    /// so under `run_post_in` the earlier one claims and the later never runs.
    /// Swapping these two rows silently inverts that trade with no other test
    /// failing, which is why the order is pinned here rather than commented.
    #[test]
    fn registry_order_is_delivery_precedence() {
        let ids: Vec<&str> = ENGINES.iter().map(|e| e.id).collect();
        assert_eq!(
            ids,
            vec!["session-opener", "guide-sections", "operator-rules", "craft-skills"]
        );
    }
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test --lib engines::tests::registry_order_is_delivery_precedence`
Expected: FAIL — today's order is `guide-sections` first.

- [ ] **Step 3: Reorder `ENGINES` and wire the emitters**

In `src/engines/mod.rs`: move the `session-opener` literal above the `guide-sections` literal, leaving `operator-rules` and `craft-skills` where they are. Set `emit_post` on the first three:

```rust
        emit_post: Some(crate::engines::emitters::emit_session_opener),
        // …
        emit_post: Some(crate::engines::emitters::emit_guide_sections),
        // …
        emit_post: Some(crate::engines::emitters::emit_operator_rules),
```

`craft-skills` keeps `emit_post: None` — it is `Mode::Unmanaged`, and Plan 1's `an_engine_without_an_emitter_is_skipped` is what makes that safe.

Add `pub mod emitters;` beside `pub mod coordinator;`.

- [ ] **Step 4: Write the emitters**

Create `src/engines/emitters.rs`:

```rust
//! The post-phase body of each registered engine, moved out of
//! `Tool::call_content` unchanged.
//!
//! Each function answers one question — *"does my trigger fire on this call,
//! and if so what do I ship?"* — and answers nothing about ordering. Ordering
//! and corpus exclusivity belong to [`super::coordinator`].

use super::coordinator::{Emission, Emitted, PostCtx};
use crate::tools::guide_emit::{guide_block, guide_blocks_for, GuideDeliveryShape};
use crate::tools::guide_ledger::GuideLedger;
use rmcp::model::Content;

/// Engine `session-opener`. Fires whenever the bootstrap topic specifically is
/// absent from the ledger — not merely whenever the ledger is empty, which
/// diverges the moment `GuideLedger::re_arm` removes just that topic.
pub fn emit_session_opener(_ctx: &PostCtx<'_>, ledger: &mut GuideLedger) -> Emitted {
    let topic = crate::prompts::SESSION_OPENING_GUIDE;
    if ledger.contains(topic) {
        return Emitted::Declined;
    }
    match guide_block(topic) {
        Some(block) => {
            // The key is burned only once the block actually builds: an
            // unregistered or empty topic must not consume the slot on silence.
            ledger.insert(topic.to_string());
            Emitted::Claimed(Emission {
                hint: Some((topic.to_string(), GuideDeliveryShape::Whole)),
                blocks: vec![block],
            })
        }
        // CLAIMED, not Declined. The pre-refactor `if/else` returned
        // `(None, Vec::new())` here rather than falling through to
        // guide-sections, and that fail-safe survives the move.
        None => Emitted::Claimed(Emission::default()),
    }
}

/// Engine `guide-sections`. Two ways to name a topic: the tool's own
/// `relevant_guide_topic` reads the RESULT and goes first because it encodes
/// what the call TOUCHED; `topic_declaring` asks which section was written for
/// this call's shape and is a fallthrough. Letting declarations win outright
/// would starve `tracker-conventions` as thoroughly as the old order starved
/// the sections — the same defect with the sign flipped.
pub fn emit_guide_sections(ctx: &PostCtx<'_>, ledger: &mut GuideLedger) -> Emitted {
    let Some(content_topic) = ctx.content_topic else {
        return Emitted::Declined;
    };
    let mut candidates: Vec<&str> = vec![content_topic];
    if let Some(t) = crate::prompts::guide_index::GUIDE_INDEX
        .topic_declaring(ctx.selector, ctx.value)
    {
        if t != content_topic {
            candidates.push(t);
        }
    }
    for topic in candidates {
        // `progressive-disclosure` is the one topic conditional on the
        // response actually overflowing — either the default path buffered a
        // large JSON, or the tool pre-buffered and returned an `output_id`.
        let should = match topic {
            "progressive-disclosure" => ctx.overflowing,
            _ => true,
        };
        if !should {
            continue;
        }
        let (blocks, shape) = guide_blocks_for(topic, ctx.selector, ctx.value, ledger);
        // `shape.is_none()` iff `blocks.is_empty()` — every return path in
        // `guide_blocks_for` keeps the two in lockstep.
        if let Some(shape) = shape {
            return Emitted::Claimed(Emission {
                hint: Some((topic.to_string(), shape)),
                blocks,
            });
        }
    }
    Emitted::Claimed(Emission::default())
}

/// Engine `operator-rules`. Append-only: it produces no hint, because the
/// primary block's single `_guide_hint` field belongs to the guide corpus.
///
/// A tool that opted out of `selector_key` declines rather than matching
/// everything. That is deliberate — a wildcard here would deliver every
/// triggered rule on every call from every un-opted-in tool.
pub fn emit_operator_rules(ctx: &PostCtx<'_>, ledger: &mut GuideLedger) -> Emitted {
    if ctx.selector.is_none() {
        return Emitted::Declined;
    }
    let mut blocks = Vec::new();
    for r in crate::operator_rules::route::route(ctx.selector, ctx.value) {
        let key = crate::operator_rules::route::ledger_key(&r.id);
        // `contains` then `insert` rather than `insert`'s return value: a
        // repeat must not refresh the stamp `expire_idle`'s TTL reads, nor pay
        // `persist()`'s unconditional disk write, for a call delivering nothing.
        if ledger.contains(&key) {
            continue;
        }
        ledger.insert(key);
        blocks.push(Content::text(format!(
            "<!-- operator-rule {} — delivered once this session for this call \
             shape; see docs/trackers/operator-rules.md -->\n{}",
            r.id, r.imperative
        )));
    }
    Emitted::Claimed(Emission {
        hint: None,
        blocks,
    })
}
```

- [ ] **Step 5: Write the emitter tests**

Append to `src/engines/emitters.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn ctx<'a>(value: &'a serde_json::Value, sel: Option<&'a str>) -> PostCtx<'a> {
        PostCtx { selector: sel, value, content_topic: None, overflowing: false }
    }

    #[test]
    fn the_opener_declines_once_its_topic_is_stamped() {
        let mut ledger = GuideLedger::default();
        let v = json!({});
        assert!(matches!(
            emit_session_opener(&ctx(&v, Some("t.a")), &mut ledger),
            Emitted::Claimed(_)
        ));
        assert!(matches!(
            emit_session_opener(&ctx(&v, Some("t.a")), &mut ledger),
            Emitted::Declined
        ));
    }

    /// Not a restatement of the opener test: this asserts the ledger key is
    /// the BARE topic name. Keying it finer would desync the trigger from what
    /// `GuideLedger::re_arm` re-arms, which no delivery assertion can see.
    #[test]
    fn the_opener_stamps_the_bare_topic_name() {
        let mut ledger = GuideLedger::default();
        let v = json!({});
        let _ = emit_session_opener(&ctx(&v, Some("t.a")), &mut ledger);
        assert!(ledger.contains(crate::prompts::SESSION_OPENING_GUIDE));
    }

    #[test]
    fn guide_sections_declines_when_the_tool_names_no_topic() {
        let v = json!({});
        assert!(matches!(
            emit_guide_sections(&ctx(&v, Some("t.a")), &mut GuideLedger::default()),
            Emitted::Declined
        ));
    }

    /// `progressive-disclosure` is gated on overflow. With `overflowing:
    /// false` the candidate is skipped, so the emitter claims and ships
    /// nothing — the topic must NOT arrive on a small response.
    #[test]
    fn progressive_disclosure_is_withheld_when_nothing_overflowed() {
        let v = json!({});
        let mut c = ctx(&v, Some("t.a"));
        c.content_topic = Some("progressive-disclosure");
        let Emitted::Claimed(e) = emit_guide_sections(&c, &mut GuideLedger::default()) else {
            panic!("a named content topic must claim");
        };
        assert!(e.is_empty(), "got {} block(s)", e.blocks.len());
    }

    #[test]
    fn progressive_disclosure_ships_when_the_response_overflowed() {
        let v = json!({});
        let mut c = ctx(&v, Some("t.a"));
        c.content_topic = Some("progressive-disclosure");
        c.overflowing = true;
        let Emitted::Claimed(e) = emit_guide_sections(&c, &mut GuideLedger::default()) else {
            panic!("a named content topic must claim");
        };
        assert!(!e.is_empty(), "the overflow path must deliver the topic");
    }

    #[test]
    fn operator_rules_declines_for_a_tool_that_opted_out_of_selector_key() {
        let v = json!({"status": "ok"});
        assert!(matches!(
            emit_operator_rules(&ctx(&v, None), &mut GuideLedger::default()),
            Emitted::Declined
        ));
    }

    /// OP-3 declares `**Serves:** memory.write` in the shipped ledger.
    #[test]
    fn operator_rules_delivers_a_matching_rule_once_then_dedups() {
        let mut ledger = GuideLedger::default();
        let v = json!({"status": "ok"});
        let Emitted::Claimed(first) = emit_operator_rules(&ctx(&v, Some("memory.write")), &mut ledger)
        else { panic!("a selector-bearing call must claim") };
        assert_eq!(first.blocks.len(), 1, "OP-3 must route on memory.write");
        assert!(first.hint.is_none(), "the rule corpus owns no _guide_hint");

        let Emitted::Claimed(second) = emit_operator_rules(&ctx(&v, Some("memory.write")), &mut ledger)
        else { panic!("a selector-bearing call must claim") };
        assert!(second.is_empty(), "a delivered rule must not fire twice in a session");
    }
}
```

- [ ] **Step 6: Run the tests**

Run: `cargo test --lib engines::`
Expected: Plan 1's 5 + these 7 + `registry_order_is_delivery_precedence` + the 6 pre-existing registry tests, all passing.

If `operator_rules_delivers_a_matching_rule_once_then_dedups` reports 0 blocks, OP-3 has been retired or its `Serves:` changed — read `docs/trackers/operator-rules.md`, pick a live `triggered` rule and its selector, and update the test's comment to name the rule you used. Do not delete the assertion.

- [ ] **Step 7: Confirm the emitters are not yet reachable from production**

Run: `grep -rn "run_post\|emitters::" src/tools/ src/server.rs`
Expected: **no matches.** Plans 1 and 2 must leave `call_content` untouched; if this prints anything, a step was done out of order.

- [ ] **Step 8: Run the gate**

```
cargo fmt
cargo clippy --workspace --all-targets --features local-embed -- -D warnings
cargo test --workspace --no-default-features ; cargo test --workspace
```

Clippy will flag the emitters as dead code if nothing references them. They are referenced by `ENGINES`, so this should be clean — if it is not, the wiring in Step 3 was missed.

- [ ] **Step 9: Commit**

```bash
git -C /home/marius/work/claude/codescout add src/engines/emitters.rs src/engines/mod.rs
git -C /home/marius/work/claude/codescout diff --cached --name-only
git -C /home/marius/work/claude/codescout diff --cached
git -C /home/marius/work/claude/codescout commit -m "feat(engines): the three post-phase emitters, and registry order as delivery precedence" -- src/engines/emitters.rs src/engines/mod.rs
```

The body must state that the reorder is behaviour-preserving **only because** nothing calls `run_post` yet, and that Plan 3 is where the order becomes live.

---

## Self-review notes for the executor

- **Nothing in this plan changes production output.** If any pre-existing test's expectations change, stop — that is evidence the move was not verbatim.
- **The `Emitted` three-state enum is the one thing worth re-reading before you simplify it.** Two independent reviewers have wanted to collapse it into `Vec<Content>`; the reason it cannot be is in `Emitted`'s doc comment and in `a_claim_spends_its_corpus_even_when_it_emits_nothing`.
- **`GuideLedger::default()` is assumed.** Task 1 Step 5 tells you what to do if it does not exist; apply the same substitution everywhere else in this plan.
- **Visibility hazard: `pub struct Emission` holds a `GuideDeliveryShape`.** If that enum is `pub(crate)` in `src/tools/core/guide_emit.rs`, the gate's `-D warnings` will fire `private_interfaces` on `Emission::hint`. Fix by narrowing — make `Emission`, `Emitted`, `PostCtx` and both `run_post*` functions `pub(crate)` — rather than by widening `GuideDeliveryShape` to `pub`. Nothing outside this crate consumes the coordinator, and widening a type to satisfy a lint exports an API nobody asked for.
