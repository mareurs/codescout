---
id: '5dad4bea44b8759b'
kind: bug
status: fixed
title: 'BUG: artifact(create/update, extra={...}) quotes hyphenated-key scalar values in frontmatter — breaks downstream regex consumers (e.g. SessionStart next-sweep-due nudge)'
tags:
- librarian
- artifact
- frontmatter
---

## Summary

When creating/updating an artifact with custom frontmatter via the `extra` param, hyphenated-key scalar values are serialized **quoted**. Observed 2026-07-17 during Stage-1 execution of the tracker-management redesign (Task 2, hygiene-ledger bootstrap):

`artifact(action="create", kind="tracker", extra={"next-sweep-due":"2026-07-17","sweep-interval-days":30})`

produced frontmatter `next-sweep-due: '2026-07-17'` (quoted). The consuming hook — `claude-plugins:codescout-companion/hooks/session-start.mjs:119` — captures the value with `/^next-sweep-due:[ \t]*(.+?)[ \t]*$/m` and then tests `/^\d{4}-\d{2}-\d{2}$/`. The captured `'2026-07-17'` (with quotes) fails the ISO test, so the nudge silently does not fire.

## Impact

Silent: no error, the artifact is created fine, but any *raw-text* frontmatter consumer that expects a bare scalar sees a quoted one and mis-parses. The hook is one such consumer; there may be others. The workaround that works is `edit_markdown(path=..., frontmatter={set:{"next-sweep-due":"2026-07-17","sweep-interval-days":30}})`, which writes the bare scalar — so the two frontmatter-write paths (`artifact extra` vs `edit_markdown frontmatter.set`) disagree on quoting.

## Open questions / fix directions

- Is the quoting unconditional for `extra` values, or triggered by the hyphen in the key, or by the value looking date-like? (Task 2 only exercised the hyphenated-date case.)
- Preferred fix: make `extra` scalar serialization match `edit_markdown frontmatter.set` (emit bare scalars where YAML allows). At minimum, document that `extra` values may be quoted and that raw-text consumers should tolerate quotes.
- Note: `next-sweep-due: 2026-07-17` unquoted is parsed by YAML 1.1 as a timestamp, by 1.2 as a string — either way the hook reads raw text, so bare is what it needs.

## Discovered

2026-07-17, Task 2 of `docs/plans/2026-07-17-tracker-lifecycle-stage1-plan.md`; flagged by the implementer subagent, worked around via the documented Step-2 fallback. Related session-log entry: `docs/trackers/tracker-redesign-session-log.md` F-1 (dormant-trigger activation).

## Status log

- 2026-07-17 — opened; worked around in Task 2. Not root-caused (which serialization path quotes, and its trigger condition, unconfirmed).
- 2026-07-17 — **root-caused + fixed** (branch `experiments`, not yet on master). Trigger confirmed by a byte-level diagnostic: it is the **value**, not the hyphenated key — `serde_yml::to_string` (in `src/librarian/frontmatter.rs::write`) conservatively quotes any string YAML 1.1 could reinterpret, so `next-sweep-due: '2026-07-17'` was quoted while `origin_session_id: abc123` and the real number `sweep-interval-days: 30` stayed bare. Fix: `write` now emits the `extra` map itself — a string scalar is emitted **bare** iff it is structurally single-line-safe AND round-trips through the YAML parser as the identical string (new `scalar_can_be_bare` helper); everything else (numbers, bools, null, arrays, nested maps, and type-ambiguous strings like `"30"`/`"true"`) is delegated to serde_yml unchanged, preserving round-trip type-safety and the idempotency the null-churn fix relies on. Test: `extra_scalar_date_is_emitted_bare_not_quoted` (frontmatter.rs) asserts bare dates, quoted type-ambiguous strings, and an identical parse→write round-trip. `cargo fmt` + `clippy -D warnings` + full `cargo test` (3296 passed) green.
