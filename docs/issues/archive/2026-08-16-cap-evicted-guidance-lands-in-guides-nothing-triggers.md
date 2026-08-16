---
kind: bug
status: fixed
tags:
- prompt-surfaces
- get_guide
- discoverability
- byte-cap
- guide-delivery
closed: null
opened: 2026-08-16
owner: marius
related: []
severity: high
---

# BUG: the 2200-byte cap evicts rules into `get_guide` topics that nothing ever triggers

## Summary

`server_instructions` is capped at 2200 bytes, and the documented remedy when it
overflows is "author a `get_guide(topic)` entry and reference it from the slice"
(`src/prompts/README.md` § Rules, rule 8). That remedy assumes the guide will be
read. Seven of ten guides have **no trigger of any kind** — they are reachable
only if the agent spontaneously calls `get_guide` for them. So the standard
response to cap pressure moves a rule from *always visible* to *effectively
unreachable*, and nothing reports that it happened.

This is a delivery defect, not an adherence one. Audit-log A-10 measured that a
directive fetched once is obeyed as reliably as one kept always-visible — the
failure mode of on-demand guidance is "never fetched", not "fetched then
forgotten". Eviction into an untriggered guide maximises exactly that failure.

## Symptom (Effect)

Measured 2026-08-16. Trigger coverage across the ten guides, from
`relevant_guide_topic()` implementations:

| Guide | Bytes | Fires on |
|---|---|---|
| `progressive-disclosure` | 5669 | 9 nav tools — **but only when output actually overflows** |
| `librarian` | 18174 | first `artifact` / `librarian` call |
| `project-activation-bootstrap` | 2507 | any tool, first call of a session (since `26ce904b`) |
| `workspace-state` | 7821 | **nothing** |
| `tracker-conventions` | 10377 | **nothing** |
| `iron-laws-detail` | 9392 | **nothing** |
| `librarian-runtime` | 9043 | **nothing** |
| `untrusted-content` | 5317 | **nothing** |
| `symbol-navigation` | 3145 | **nothing** |
| `error-handling` | 1857 | **nothing** |

Seven guides, ~46 KB, delivered only on request.

The concrete instance that surfaced this: commit `a926fdf5`
("workspace-gate para 2 moves to its guide") removed this paragraph from
`src/prompts/source.md` to make room for the Iron-Law-1 overlap condition —

```
Parallel subagents on DIFFERENT workspaces: pin each call with
workspace=<abs path>, don't activate. Full rules: get_guide("workspace-state").
```

— relocating it into `workspace-state.md`, which has no trigger. A rule that was
in every session's context is now in none of them unless asked for by name.

## Reproduction

1. `grep(pattern="fn relevant_guide_topic", glob="src/**/*.rs")` — 11 impls; the
   only topics returned are `librarian`, `progressive-disclosure`, and
   `project-activation-bootstrap`.
2. `get_guide()` with no argument lists ten topics.
3. The seven not in step 1 have no auto-delivery path.

## Environment

codescout `experiments` at `148aabe6`. Guide bodies are `include_str!`'d
(`src/prompts/mod.rs`), delivered by the V2 hard-injection path in
`Tool::call_content` (`src/tools/core/types.rs`).

## Root cause

Two mechanisms compose, and neither is wrong alone:

1. **The cap has an escape valve that is not a delivery channel.**
   `src/prompts/README.md` rule 8 instructs the author, on cap failure, to move
   content into a `get_guide` topic and leave a pointer. Nothing in that
   instruction requires the topic to have a trigger, and no test asserts one.
2. **`relevant_guide_topic()` defaults to `None`**
   (`src/tools/core/types.rs`), so a topic is un-triggered unless some tool opts
   in by name. Adding a guide is one edit; wiring its trigger is a second,
   separate edit that nothing prompts for.

The result is that "move it to a guide" reads as *filed* when it is closer to
*deleted from the agent's view*. `a926fdf5` is the measured instance: a rule that
had been in the always-loaded surface is now in an untriggered file.

measured 2026-08-16: `grep(pattern="fn relevant_guide_topic", glob="src/**/*.rs")`
→ 11 impls, 3 distinct topics; `wc -c src/prompts/guides/*.md` → the byte column
above; `git log -S "pin each call" -- src/prompts/source.md` → `a926fdf5`,
`66bfd45c`.

## Evidence

### The eviction is visible as a live-vs-source divergence

This session's `server_instructions` (injected before `a926fdf5` was built)
still carries the pinning paragraph; `src/prompts/source.md` at HEAD does not.
The paragraph exists in `workspace-state.md`, which nothing fires. So the same
rule is simultaneously in an old session's context and out of every new one's,
which is what makes the loss silent — whoever moved it was still seeing it.

### `progressive-disclosure` is a partial exception, not a counter-example

It has 9 triggering tools but fires only when `exceeds_inline_limit(&json)` or
the tool pre-buffered (`src/tools/core/types.rs`, the `"progressive-disclosure"`
match arm). A session whose calls all return small results never receives it.

## Hypotheses tried

1. **Hypothesis** — the untriggered guides are all reference material an agent
   only needs on demand, so no trigger is correct. **Test** — check whether any
   holds a rule that used to be always-visible. **Verdict** — rejected;
   `workspace-state` holds exactly such a rule, moved there by `a926fdf5`.

## Fix

Fixed on `experiments` — all three parts, `72f39849` (1 and 3) and `73ccb495` (2).

**1. Gate the eviction. — DONE.** `server::tests::every_guide_topic_is_triggered_or_declared_pull_only`
fails the build for any registered topic that is neither triggered nor listed in
`prompts::PULL_ONLY_GUIDE_TOPICS` with a reason. Both directions are checked — a stale entry
fails too, whether it names a topic that has since gained a trigger or one that no longer
exists — and a placeholder reason is rejected, since a one-word excuse restores the silent
default it replaced.

**2. Wire the obvious triggers. — DONE**, all three the operator chose, and the mechanism
changed to allow it. `relevant_guide_topic()` now receives the call's **result**. `input`
was the obvious parameter and is the wrong one: `call_content` moves it into `call()`
before the hint is computed, so passing it means cloning every tool's input on every call.
The result is already in scope, already borrowed, and already carries `abs_path`.

- `workspace-state` ← the `workspace` tool. The arm previously returned
  `SESSION_OPENING_GUIDE`, which was already redundant: `activate` clears the ledger inside
  `call()`, and the empty-ledger branch fires the opener regardless.
- `tracker-conventions` ← `artifact`/`librarian` when the result names a `docs/issues/` or
  `docs/trackers/` path.
- `symbol-navigation` ← `symbols`/`references`/`call_graph` on a result that *fits*. Those
  tools returned `progressive-disclosure` unconditionally, but `call_content` gates that
  topic on overflow having actually happened — so on a small result the slot delivered
  **nothing at all**. It now costs nothing to spend.

**3. Amend README rule 8. — DONE.** Both places that gave the incomplete remedy — rule 8 and
the byte-cap section under *Verify the slice* — now name the second step and say why.

**Result: 47,343 → 26,488 undelivered bytes; 63% of the corpus firing for nobody → 35%.**
Four topics remain pull-only *by decision*, each with its reason recorded.

The byte tension this section flagged is unresolved and deliberately so: a session doing
tracker work can now receive `tracker-conventions` (10.4 KB) *and* `librarian` (19.9 KB)
over its lifetime. Cutting the corpus is the answer to that, not withholding the guide —
still sequenced behind the dedup stream.
## Tests added

- `every_guide_topic_is_triggered_or_declared_pull_only` (`src/server.rs`) — the gate.
  Mutation-verified by deleting an allowlist entry.
- `tracker_paths_route_to_the_tracker_guide_and_nothing_else_does`
  (`src/librarian/adapter.rs`) — the discriminator, including the `find`-shaped `items[]`
  case a top-level-only check would miss, and a near-miss (`title` mentioning
  `docs/trackers/`) that must **not** route.
- `an_artifact_call_naming_a_tracker_path_delivers_the_tracker_guide` (`src/server.rs`) —
  end-to-end through `call_content`.

**The gate caught a blind spot in itself, which is the part worth keeping.** Moving
`workspace` off `SESSION_OPENING_GUIDE` left `project-activation-bootstrap` with no *tool*
trigger, and the test failed it — but that guide is delivered by `call_content`'s
empty-ledger branch, a second path a scan of tool impls cannot see. The gate would have had
me re-add a redundant trigger to fix a non-problem. It now knows about both paths.

Also surfaced rather than left as a trap: `first_artifact_call_emits_librarian_hint` runs
`find kind=tracker` and still expects `librarian`, which holds **only** because its catalog
is empty and `items: []` names no path. The new test states that dependency.

**Verified live** on the rebuilt server: all three fired on their first eligible call. The
`workspace-state` payload delivered is the closing argument — it contains the per-call
workspace-pinning rule that `a926fdf5` evicted from the always-loaded slice, a
concurrency-correctness rule that has been unreachable ever since.

Gate: `cargo fmt` + `cargo clippy --all-targets -D warnings` clean, `cargo test --lib`
3777 passed / 0 failed / 7 ignored.
## Workarounds

When a prompt-surface edit moves content out of `source.md` under cap pressure,
check by hand whether the destination guide has a trigger, and add one in the
same commit. There is currently nothing that will tell you.

## Resume

Write the invariant test described in Fix part 1 next to
`prompts::tests::guide_topics_have_bodies` in `src/prompts/mod.rs` — it already
enumerates the topic list, so the trigger check belongs beside it. Decide the
pull-only allowlist with the user before wiring triggers (part 2), since each
trigger adds bytes to a session that may not need them.

## References

- `src/prompts/README.md` § Rules, rule 8 — the cap remedy that ends a step early
- `src/tools/core/types.rs` — `relevant_guide_topic()` default `None`; the V2 injection path
- `src/prompts/mod.rs` — `topic_body()`, `SESSION_OPENING_GUIDE`
- commit `a926fdf5` — the measured eviction
- commit `26ce904b` — the session-opener trigger widening, same delivery class
- `docs/trackers/prompt-hamsa-audit-log.md` A-10 (fetched-once is enough; discoverability is the lever), A-22 (this work stream)
