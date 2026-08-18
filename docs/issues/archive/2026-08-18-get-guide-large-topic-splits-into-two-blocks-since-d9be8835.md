---
kind: bug
status: fixed
tags:
- guides
- guide-ledger
- gate
closed: 2026-08-19
opened: 2026-08-18
owner: marius
related: []
severity: medium
---

# BUG: `get_guide` on a large topic returns two blocks instead of one, since `d9be8835`

## Summary

`tools::guide::tests::get_guide_large_topic_returns_full_body_inline_not_buffered` failed
on `experiments`. A large guide was expected to come back as **one** inline block and came
back as **two**. The gate was red for everyone on the branch.

**Resolved 2026-08-19 in `98306e53`.** The red gate was real; the *defect* was in the test, not in
shipped behaviour — `get_guide` never stopped returning the body inline. See § Fix.

**Filed cross-stream and picked up as intended.** This was captured on notice by a session running an
unrelated prompt-surface audit (A-28), which correctly declined to fix it and handed it to whoever
owned the guide-ledger Phase B/C work. That stream found it independently in its own whole-branch
review and fixed it. The capture-on-notice handoff worked exactly as the convention intends — worth
noting, because the file sat `open` for a day while the owning stream's *filtered* test runs
(`--lib guide_hint_tests`, `--lib guide_ledger`) stayed green and never reached it.
## Symptom (Effect)

```
---- tools::guide::tests::get_guide_large_topic_returns_full_body_inline_not_buffered ----
thread '...' panicked at src/tools/guide.rs:225:9:
assertion `left == right` failed: guide must be a single inline block
  left: 2
 right: 1
```

Whole-suite result at HEAD (`f5bdc86f`): `4067 passed; 1 failed; 7 ignored`.

## Reproduction

```
cargo test --lib get_guide_large_topic_returns_full_body_inline_not_buffered
```

## Environment

codescout 0.15.0, branch `experiments`, Linux, debug build.

## Root cause

Unknown — not investigated, because it belongs to an active concurrent work stream and
the owner will have the context. **Bisected to a single commit**, which is the useful
part:

| commit | subject | result |
|---|---|---|
| `5002966d` | docs(trackers): W-2 … | **GREEN** |
| `cb6e884e` | feat(guide-ledger): carry the rendezvous gate on the ledger it gates | **GREEN** |
| `cf25c056` | fix(guide-ledger): close the hole where a hardcoded rendezvous_active(true) stayed green | **GREEN** |
| **`d9be8835`** | **feat(guide-ledger): open the session on a missing bootstrap, not an empty set** | **RED** |
| `f5bdc86f` | (HEAD) | RED |

*measured 2026-08-18: `cargo test --lib get_guide_large_topic_returns_full_body_inline_not_buffered`
run in a detached worktree at each commit above.* Each result is observed, not inferred.

The likely mechanism, stated as a hypothesis and **not** measured: `d9be8835` changes
when a session is considered "opened", and the guide emitter appends a second block
(a hint / auto-inject preamble) depending on ledger state. Opening on a *missing*
bootstrap rather than an *empty set* would flip that condition for a fresh test context,
producing the extra block. Read `src/tools/guide.rs` around the assertion at line 225 and
the ledger predicate `d9be8835` introduced before trusting this paragraph.

## Evidence

The bisect was run in an isolated detached worktree so that the working tree of the
session that owns this code was never touched:

```
git worktree add --detach <scratch> 5002966d   -> GREEN
git checkout cb6e884e / cf25c056               -> GREEN
git checkout d9be8835                          -> RED
```

## Hypotheses tried

1. **Hypothesis:** introduced by the concurrent prompt-surface work (A-27's
   `artifact_augment` cut, the `link_scan` regex bound, the tool-surface budget constant).
   **Test:** run the failing test at `5002966d`, the last commit carrying all of those.
   **Verdict:** rejected — GREEN there, and none of those changes touch `src/tools/guide.rs`
   or any guide body.
2. **Hypothesis:** a guide markdown file grew past an inlining threshold.
   **Test:** `git log -- src/prompts/guides/` — last change is `7ee6a9c2`, which predates
   the green run at `5002966d`. **Verdict:** rejected as the *trigger*, though the
   threshold itself remains ungated (see References).

## Fix

Fixed on `experiments` in **`98306e53`** — *fix(guide): stop measuring the opener in the inline-body
regression test*. (Fast-forward promotion path: `git rev-list --left-right --count master...experiments`
returns `0` on the left, so this SHA already is the master SHA once `master` moves. No second SHA to
record.)

**The diagnosis in this file's Root cause was right, and the prescription was one step off.** The
predicate change in `d9be8835` is indeed what made a second block legitimate — but the defect is in
the *test*, not in shipped behaviour. `get_guide` still returns the full guide body inline; the test
was asserting `content.len() == 1` as a **proxy** for "inline, not buffered", and Phase C made
`len() == 2` a correct, unrelated outcome (session opener + guide body) for a fixture starting from
an empty ledger. Nothing user-facing was ever broken.

The fix has two parts, both in `src/tools/guide.rs`:

1. The shared `ctx()` helper now builds `GuideLedger::mid_session()` instead of a bare default, so
   the fixture no longer trips the opener. `mid_session()` exists for exactly this and says so in
   its own doc comment.
2. **The assertion no longer counts blocks.** It asserts on the primary block's actual shape — no
   `@tool_` handle, guide text present, `text.len() > 10_000`. Part 2 is the load-bearing half:
   `call_content` builds `blocks = vec![primary]` before pushing any guide block
   (`src/tools/core/types.rs:895-901`), so `content.first()` is the tool's own output whatever the
   ledger holds. Part 1 is belt-and-braces.

Causation was **demonstrated, not inferred**: the Phase C whole-branch reviewer reverted the
predicate at `src/tools/core/types.rs:709` back to `emitted.is_empty()` and watched this test turn
green — one probe that killed a mutation and diagnosed the failure at the same time.
## Tests added

None added — and none needed. The pre-existing test *is* the regression guard, and it now guards the
property in its own name rather than a proxy for it.

`tools::guide::tests::get_guide_large_topic_returns_full_body_inline_not_buffered`
(`src/tools/guide.rs`). What now makes it fail: removing `GetGuide::force_inline()`. Without that
override the ~14 KB `librarian` body exceeds `TOOL_OUTPUT_BUFFER_THRESHOLD`, `call_content` takes the
overflow branch, and the primary block becomes a `{output_id, summary, …}` envelope — which trips
**two** assertions independently: it contains `@tool_`, and it cannot reach 10 KB (`summary` is
hard-capped at `COMPACT_SUMMARY_HARD_MAX_BYTES = 3000`). Verified by an independent re-review that
tried and failed to construct a passing-but-buffered case.

Gate on `experiments` at the fix: `cargo test` 4223 passed / 0 failed, `cargo fmt --check` clean,
`cargo clippy --all-targets -- -D warnings` clean.
## Workarounds

None needed for correctness of shipped behaviour — the effect is on guide framing, not on
guide content. But the branch gate is red, so any *other* work using
`cargo test` as its completion check will see a failure it did not cause.

## Resume

N/A — fixed and verified on `experiments`.
## References

- `src/tools/guide.rs:225` — the assertion
- `d9be8835` — the commit the bisect lands on
- `docs/trackers/prompt-surface-compaction-session-log.md` W-2 — the guide corpus is
  91,869 bytes across 10 topics with **no size gate** of any kind (the only guide-related
  constant is a TTL). This bug is about framing rather than size, but it lands on the same
  ungated surface.
