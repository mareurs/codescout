---
status: open
opened: 2026-08-16
closed:
severity: high
owner: marius
related: []
tags: [prompt-surfaces, get_guide, discoverability, byte-cap, guide-delivery]
kind: bug
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

Plan — three parts, in increasing cost:

1. **Gate the eviction.** A test asserting every registered `get_guide` topic
   either has a `relevant_guide_topic()` trigger or is explicitly listed as
   pull-only with a reason. That converts a silent omission into a build
   failure, and is the piece that stops recurrence.
2. **Wire the obvious triggers.** `workspace-state` → the `workspace` tool
   (alongside the session opener); `tracker-conventions` → `artifact`/`librarian`
   when the target is under `docs/issues/` or `docs/trackers/`;
   `error-handling` → nothing obvious, likely genuinely pull-only.
3. **Amend `src/prompts/README.md` rule 8** so the documented cap remedy is
   "author the topic AND give it a trigger, or record why it is pull-only" —
   the instruction currently ends one step early, which is what allowed this.

Note the tension with byte cost: `librarian` alone is 18 KB and already fires on
a routine call. Wiring more triggers without first cutting the corpus trades one
problem for another — sequence this behind the dedup stream (audit-log A-22, P3).

## Tests added

None yet. Part 1 of the fix IS the test, and is the load-bearing piece: without
it the next cap overflow evicts the next rule the same way.

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
