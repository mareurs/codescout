# Layer 2a, Plan 3 — Wiring and One Budget Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the coordinator the *actual* fan-out — `call_content` calls it instead of inlining the logic — with output byte-identical to today, and then widen the one byte ceiling so it counts every managed emitter rather than only the guide corpus.

**Architecture:** Task 1 deletes ~120 lines from `Tool::call_content` and replaces them with one `run_post` call, changing no bytes. Task 2 changes what the p50 ceiling test *measures*: today `shape_total` filters for the `<!-- auto-injected get_guide(` marker, so operator-rule bytes land in the same context window counted by nothing. Dropping the filter makes the ceiling a budget over the whole managed emission, and the new ceiling is **derived from a measurement taken in Task 1**, not chosen.

**Tech Stack:** Rust (edition per `Cargo.toml`), `rmcp::model::Content`, `serde_json::Value`, `tokio` tests.

**Spec:** [`../specs/2026-09-02-retrieval-engine-coordination-design.md`](../specs/2026-09-02-retrieval-engine-coordination-design.md) — Rollout step 2a, § *Layer 2*, Gate 3.

**Plan sequence:** [1 — coordinator](2026-09-02-layer-2a-1-coordinator.md) → [2 — emitters](2026-09-02-layer-2a-2-emitters.md) → **3 — wiring and one budget (this)**. **Plans 1 and 2 must both be complete and committed before Task 1 here.**

## Global Constraints

- **The gate, all four, in this order** — two test lanes chained with `;`, **never** `&&`:
  ```
  cargo fmt
  cargo clippy --workspace --all-targets --features local-embed -- -D warnings
  cargo test --workspace --no-default-features ; cargo test --workspace
  ```
- **Task 1 must be byte-identical.** Any pre-existing test whose expectation needs editing is evidence the refactor was not faithful — stop rather than adjust the test.
- **A worktree exists at `.worktrees/tool-collapse`.** Use `git -C /home/marius/work/claude/codescout` for every git mutation; bare `git commit` is hook-blocked.
- **Shared checkout.** Never `git add -A`. Stage by pathspec → `git diff --cached --name-only` → `git diff --cached` → commit by pathspec.
- **A derived number ships with its derivation and its population.** Task 2 sets a new ceiling; the value alone is not the deliverable. See that task's Step 7.

---

## File Structure

| file | responsibility |
|---|---|
| **Modify** `src/tools/core/types.rs:920-1160` | Replace the inlined fan-out with one `run_post` call. Keeps: primary-block assembly, `inject_hint`, `inject_notice`, buffering. |
| **Modify** `src/server.rs` (`a_p50_session_stays_under_the_committed_guide_byte_ceiling`, ~`:7981`) | Widen `shape_total` from the guide marker to every appended block; re-derive `CEILING`. |
| **Modify** `docs/superpowers/specs/2026-09-02-retrieval-engine-coordination-design.md` | Record the derived ceiling and, in the same place, what it does **not** cover. |

---

## Task 1: `call_content` calls the coordinator

**Files:**
- Modify: `src/tools/core/types.rs` — the `let (guide_hint, guide_content) = { … }` block (~`:932-1100`), the `let op_content = … ;` block (~`:1119-1157`), and the block assembly (~`:1230-1233`)
- Test: no new tests. The regression gate is the existing suite — `mod guide_hint_tests` in `src/server.rs`, `src/tools/core/tests.rs`, `src/tools/memory/tests.rs::a_real_memory_write_call_delivers_op_3`, `src/tools/config/tests.rs::workspace_activate_injects_bootstrap_guide_body_v2`.

**Interfaces:**
- Consumes: `crate::engines::coordinator::{PostCtx, run_post}` and `Emission { hint, blocks }` from Plans 1 and 2.
- Produces: nothing new. `call_content`'s signature and output are unchanged.

- [ ] **Step 1: Verify `relevant_guide_topic` is pure — this is a real hazard, not a formality**

Today `self.relevant_guide_topic(&val)` runs **only** when the session opener does not fire. After this refactor it runs on every call, because `PostCtx` resolves it eagerly. If any implementation has a side effect, that side effect becomes newly unconditional.

Run: `grep -rn "fn relevant_guide_topic" src/`

Read every override. Each must be a pure function of `result` — a `match` on fields, returning `Option<&'static str>`. If any one writes, logs at warn/error, or touches `ctx`, **stop and report**: `PostCtx` needs a lazy variant and that is a design change, not a transcription.

- [ ] **Step 2: Record the p50 byte total BEFORE the change**

Add temporarily, immediately before the final `assert!` in `a_p50_session_stays_under_the_committed_guide_byte_ceiling` (`src/server.rs`):

```rust
        eprintln!("P50_TOTAL_BEFORE={total}");
```

Run: `cargo test --lib a_p50_session_stays_under -- --nocapture 2>&1 | grep P50_TOTAL`

Write the number down. It is the byte-identical check for Step 6 and the input to Task 2's derivation. **Leave the `eprintln!` in place** — Step 6 needs it and Task 2 Step 3 removes it.

- [ ] **Step 3: Replace the guide/rule fan-out**

In `src/tools/core/types.rs`, delete the whole `let (guide_hint, guide_content): (Option<(String, GuideDeliveryShape)>, Vec<Content>) = { … };` block **and** the whole `let op_content: Vec<Content> = if selector.is_some() { … } else { … };` block, and put this in their place:

```rust
        // The post-phase fan-out. Ordering, corpus exclusivity and the
        // anonymous-tier idle re-arm all live in the coordinator now; what
        // stays here is assembling the primary block, which the coordinator
        // must not know about because `inject_hint` mutates it in place.
        //
        // One lock for the whole fan-out, where this used to take two — the
        // guide path and the rule path each locked `guide_hints_emitted`
        // separately. Fewer acquisitions, identical bytes.
        let (guide_hint, injected) = {
            let mut emitted = ctx.guide_hints_emitted.lock();
            let post = crate::engines::coordinator::PostCtx {
                selector: selector.as_deref(),
                value: &val,
                content_topic: self.relevant_guide_topic(&val),
                // Either the default-path buffering will kick in (large JSON),
                // or the tool itself pre-buffered (e.g. `run_command` storing a
                // `@cmd_*` ref and returning a small envelope with
                // `output_id`). Both signal the agent should learn the
                // progressive-disclosure pattern.
                overflowing: exceeds_inline_limit(&json)
                    || val
                        .as_object()
                        .and_then(|o| o.get("output_id"))
                        .and_then(|v| v.as_str())
                        .is_some(),
            };
            let e = crate::engines::coordinator::run_post(&post, &mut emitted);
            (e.hint, e.blocks)
        };
```

Then change the block assembly at the end of the method from:

```rust
        let mut blocks = vec![primary];
        blocks.extend(guide_content);
        blocks.extend(op_content);
        Ok(blocks)
```

to:

```rust
        let mut blocks = vec![primary];
        blocks.extend(injected);
        Ok(blocks)
```

`guide_hint` keeps its name and type (`Option<(String, GuideDeliveryShape)>`), so both `inject_hint` call sites in the primary-block assembly are untouched.

- [ ] **Step 4: Fix the imports**

`src/tools/core/types.rs:15` imports `guide_block, guide_blocks_for, inject_hint, GuideDeliveryShape` from `guide_emit`. After Step 3, `guide_block` and `guide_blocks_for` are no longer called here — they moved to `src/engines/emitters.rs`. Remove exactly those two from the import list; keep `inject_hint` and `GuideDeliveryShape`.

Run: `cargo build 2>&1 | grep -E "^(error|warning: unused)" | head -20`
Expected: no unused-import warnings, no errors.

- [ ] **Step 5: Run the injection test suites**

```
cargo test --lib guide_hint_tests
cargo test --lib tools::core::tests
cargo test --lib a_real_memory_write_call_delivers_op_3
cargo test --lib workspace_activate_injects_bootstrap_guide_body_v2
```

Expected: all pass, **with no test edited.** If one fails, the move was not verbatim — diff your emitters against the deleted code rather than adjusting the assertion.

- [ ] **Step 6: Prove byte-identical**

Run: `cargo test --lib a_p50_session_stays_under -- --nocapture 2>&1 | grep P50_TOTAL`

Expected: **exactly the number from Step 2.** Not "close", not "under the ceiling" — equal. A different number means ordering, exclusivity, or a ledger stamp changed, and the whole premise of 2a is that none of them did.

If it differs, the two most likely causes in order: (a) `ENGINES` order does not put `session-opener` first, so guide-sections now claims the corpus; (b) an emitter returns `Declined` where the original branch fell through to `(None, Vec::new())`. Both are Plan 1 or Plan 2 defects — fix them there.

- [ ] **Step 7: Run the gate**

```
cargo fmt
cargo clippy --workspace --all-targets --features local-embed -- -D warnings
cargo test --workspace --no-default-features ; cargo test --workspace
```

`peer::server::tests::run_exits_after_idle_timeout_with_no_connections` is a filed load-sensitive flake (`docs/issues/2026-09-01-peer-idle-timeout-test-is-the-third-load-sensitive-step.md`). If it is the only failure, re-run it alone and record both results. Also run `cargo test --test cli_artifact` and confirm 11/11 — that is what tells you `target/debug/codescout` is librarian-bearing and the lean-lane trap is not armed for other sessions.

- [ ] **Step 8: Commit**

```bash
git -C /home/marius/work/claude/codescout add src/tools/core/types.rs
git -C /home/marius/work/claude/codescout diff --cached --name-only
git -C /home/marius/work/claude/codescout diff --cached
git -C /home/marius/work/claude/codescout commit -m "refactor(engines): call_content fans out through the coordinator, byte-identical" -- src/tools/core/types.rs
```

The body must carry the two P50 totals from Steps 2 and 6 side by side, and state the line-count delta on `call_content`. Note that the temporary `eprintln!` is still in the tree and is removed by the next task.

---

## Task 2: One budget — the ceiling counts every managed emitter

**Files:**
- Modify: `src/server.rs` — `guide_blocks` is untouched; `shape_total` and `CEILING` inside `a_p50_session_stays_under_the_committed_guide_byte_ceiling` (~`:7981-8019`)
- Modify: `docs/superpowers/specs/2026-09-02-retrieval-engine-coordination-design.md` — § *Measurements this spec rests on*
- Test: the modified ceiling test is itself the deliverable

**Interfaces:**
- Consumes: the P50 total measured in Task 1 Step 6.
- Produces: a `CEILING` covering three of the four registered engines, and a written statement of the fourth.

**Why this is not a one-line change.** The spec's § *Problem 4* records the current state: there is **one** byte budget, it covers **part** of the window, and two emitters are bounded by nothing. `shape_total` sums only blocks containing `<!-- auto-injected get_guide(`; operator rules emit `<!-- operator-rule OP-N …`. The filter is why the guide ceiling is *written not to see* the other engine's bytes.

- [ ] **Step 1: Confirm the marker filter appears in exactly one place**

Run: `grep -rn "auto-injected get_guide(" src/ tests/ scripts/`

Expected: the emitter in `src/tools/core/guide_emit.rs`, the filter in `shape_total`, and assertions in guide tests. If a **second** budget-style filter exists, it needs the same treatment — say so and widen this task rather than leaving a second partial accounting.

- [ ] **Step 2: Measure what the widened count actually is**

Change `shape_total`'s body in `src/server.rs` from:

```rust
            let bytes: usize = guide_blocks(out)
                .iter()
                .filter(|b| b.contains("<!-- auto-injected get_guide("))
                .map(|b| b.len())
                .sum();
```

to:

```rust
            // Every block after the primary, not only the ones carrying the
            // guide marker. `guide_blocks` is already `skip(1)`, so this is
            // the whole managed emission for the call: guide sections, the
            // session opener, and triggered operator rules. Filtering by
            // marker is what made this a guide budget rather than a budget.
            let bytes: usize = guide_blocks(out).iter().map(|b| b.len()).sum();
```

Change the `eprintln!` Task 1 left behind to `eprintln!("P50_TOTAL_WIDENED={total}");` and run:

`cargo test --lib a_p50_session_stays_under -- --nocapture 2>&1 | grep P50_TOTAL`

Expected: a number **greater than or equal to** Task 1's. Equal means the p50 fixture session triggers no operator rule — check which shapes route (`grep -n "Serves:" docs/trackers/operator-rules.md`) before accepting it, because an unchanged number is also what a broken filter edit produces.

- [ ] **Step 3: Set the ceiling from the measurement, and remove the `eprintln!`**

Delete the `eprintln!` line. Replace the `CEILING` constant and its comment with:

```rust
        // One budget over every MANAGED emitter, not a guide budget.
        //
        // DERIVED, not chosen: measured at <MEASURED> B on <DATE> by summing
        // every block after the primary across the p50 session's six shapes
        // (see `shape_total` below). Set to that figure plus ~10% headroom so
        // ordinary corpus edits do not red the gate while a new emitter or a
        // doubled section does. Re-derive rather than raise if it fires — the
        // number is a fact about the corpus, and raising it to fit is how a
        // ceiling stops being one.
        //
        // POPULATION — what this does NOT cover, stated here because a bound
        // whose scope lives elsewhere gets read as covering everything:
        //   * `craft-skills` (engine 6) is `Mode::Unmanaged`. Skill bodies
        //     reach the same context window through the harness, never through
        //     an MCP response, so no assertion on `Content` blocks can see a
        //     byte of them.
        //   * The PRE phase does not exist yet (Rollout 2b). When it does,
        //     its blocks arrive through this same return value and are counted
        //     automatically — no change needed here, which is the point of
        //     counting blocks rather than markers.
        const CEILING: usize = <MEASURED_PLUS_HEADROOM>;
```

Substitute `<MEASURED>`, `<DATE>` and `<MEASURED_PLUS_HEADROOM>` with real values. Round the ceiling to a clean hundred.

- [ ] **Step 4: Rename the test to match what it now measures**

`a_p50_session_stays_under_the_committed_guide_byte_ceiling` no longer measures a *guide* ceiling. Rename to:

```rust
    async fn a_p50_session_stays_under_the_committed_emission_byte_ceiling() {
```

Run: `grep -rn "a_p50_session_stays_under_the_committed_guide_byte_ceiling" src/ docs/`
Update every hit. The spec's § *Problem 4* table names this test in its "bound" column — that row must move with it.

- [ ] **Step 5: Run the test**

Run: `cargo test --lib a_p50_session_stays_under`
Expected: PASS, with the assertion message reporting real headroom.

- [ ] **Step 6: Verify the ceiling can fail**

Temporarily set `const CEILING: usize = 1;` and run the test.
Expected: FAIL, and the message names the overage. **Revert.**

This is not ceremony: a ceiling whose assertion never fires is decoration, and the filter edit in Step 2 is exactly the kind of change that can silently zero a sum. A green test proves nothing about a sum that is now always 0.

- [ ] **Step 7: Publish the derivation where a reader will meet it**

The number now lives in a test-module comment — an audience that reads it only when it breaks. Add to the spec's § *Measurements this spec rests on*, under the Layer 2 scout block:

```markdown
- **One budget, derived <DATE>.** The p50 session's total managed emission is
  <MEASURED> B across six shapes, counting every block after the primary
  rather than only those carrying the `<!-- auto-injected get_guide(` marker.
  Ceiling set to <CEILING> B. Covers `guide-sections`, `session-opener` and
  `operator-rules`; **does not** cover `craft-skills`, whose bodies never
  travel through an MCP response. Re-derive with
  `cargo test --lib a_p50_session_stays_under -- --nocapture`.
```

And update Gate 3's text in the same file: it currently says the work is *"extending an accounting to emitters that have none"*, which is now done for two of the three unbudgeted rows in § *Problem 4*'s table. Mark those two rows as covered and leave `craft-skills` uncovered, so the table keeps naming the remaining hole.

- [ ] **Step 8: Run the gate**

```
cargo fmt
cargo clippy --workspace --all-targets --features local-embed -- -D warnings
cargo test --workspace --no-default-features ; cargo test --workspace
```

- [ ] **Step 9: Commit**

```bash
git -C /home/marius/work/claude/codescout add src/server.rs docs/superpowers/specs/2026-09-02-retrieval-engine-coordination-design.md
git -C /home/marius/work/claude/codescout diff --cached --name-only
git -C /home/marius/work/claude/codescout diff --cached
git -C /home/marius/work/claude/codescout commit -m "test(engines): the byte ceiling counts every managed emitter, not just the guide corpus" -- src/server.rs docs/superpowers/specs/2026-09-02-retrieval-engine-coordination-design.md
```

The body must carry: the before figure (guide-only), the after figure (widened), the chosen ceiling with its headroom rationale, and the `craft-skills` exclusion. The code change and the spec change **must go in one commit** — a ceiling whose published scope lands separately is a partial state every other session reads as complete.

---

## Self-review notes for the executor

- **Task 1 and Task 2 must not be merged.** Task 1's claim is *"nothing changed"* and Task 2's is *"the measurement changed"*. Combined, neither is checkable: you could no longer tell a refactor slip from a wider count.
- **Two numbers are recorded and both matter.** Task 1 Step 6 asserts equality with Step 2 — that is the byte-identical proof. Task 2 Step 2 expects an increase — that is the widening proof. Confusing them turns a regression into a feature.
- **If `relevant_guide_topic` turns out impure (Task 1 Step 1), stop.** That is a design change and belongs back in the spec, not in a transcription step.
- **After both tasks, Rollout 2b (the pre phase) is unblocked.** Nothing here should have to move for it: the coordinator already takes a `Phase`-shaped role, and Task 2's block-counting ceiling absorbs pre-phase bytes with no further edit.
