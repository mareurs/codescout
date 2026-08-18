---
status: open
opened: 2026-08-18
closed:
severity: medium
owner: marius
related: []
tags: [guides, guide-ledger, gate]
kind: bug
---

# BUG: `get_guide` on a large topic returns two blocks instead of one, since `d9be8835`

## Summary

`tools::guide::tests::get_guide_large_topic_returns_full_body_inline_not_buffered` fails
on `experiments`. A large guide is expected to come back as **one** inline block and now
comes back as **two**. The gate is red for everyone on the branch.

**Not my work stream** — filed under capture-on-notice while running an unrelated
prompt-surface audit (A-28). Whoever owns the guide-ledger Phase B/C work should take it;
the fix is almost certainly a one-liner in the commit named below.

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

Not attempted — owner's call. Start at `d9be8835`.

## Tests added

None. The failing test is pre-existing and is the correct regression guard; it caught this.

## Workarounds

None needed for correctness of shipped behaviour — the effect is on guide framing, not on
guide content. But the branch gate is red, so any *other* work using
`cargo test` as its completion check will see a failure it did not cause.

## Resume

Read `src/tools/guide.rs:225` and the assertion's surroundings, then diff `d9be8835` for
the predicate change that decides whether a second block is emitted. Confirm against a
fresh (bootstrap-missing) ledger context, which is what the test constructs.

## References

- `src/tools/guide.rs:225` — the assertion
- `d9be8835` — the commit the bisect lands on
- `docs/trackers/prompt-surface-compaction-session-log.md` W-2 — the guide corpus is
  91,869 bytes across 10 topics with **no size gate** of any kind (the only guide-related
  constant is a TTL). This bug is about framing rather than size, but it lands on the same
  ungated surface.
