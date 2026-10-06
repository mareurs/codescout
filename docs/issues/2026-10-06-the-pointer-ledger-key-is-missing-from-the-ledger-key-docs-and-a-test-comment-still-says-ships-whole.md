---
id: '18eb26aefcd6ca4c'
kind: bug
status: open
title: 'BUG: the topic-pointer ledger key is missing from the ledger-key docs, and a test comment still says tracker-conventions ships whole'
tags:
- guides
- doc-drift
- guide-ledger
- cluster/doc-contradicted-by-code
opened: 2026-10-06
owner: marius
related:
- docs/issues/archive/2026-09-24-a-guide-in-a-result-the-harness-saves-to-disk-is-marked-delivered-unread.md
severity: low
---

# BUG: the `<topic>#<pointer>` ledger key is missing from the ledger-key documentation, and a test comment still says `tracker-conventions` "ships WHOLE"

## Summary

`MAX_AUTO_INJECT_GUIDE_BYTES` made `tracker-conventions` ship a one-line pointer, stamped under a new ledger key `<topic>#<pointer>`. Three comments that enumerate the key shapes or the delivery shape were not updated. They still describe the old behaviour.

## Symptom (Effect)

1. `src/engines/mod.rs:184-187`, the doc of `owns_guide_key`, lists the keys as "whole (`<topic>`) or sliced (`<topic>#<heading>`, `<topic>#<preamble>`)". It omits `<topic>#<pointer>`, which `owns_guide_key` also claims, because it splits on `#` and checks only the topic.
2. `src/prompts/guide_index.rs:459-463`, the doc of `ledger_keys()`, says it lists "topic keys and section keys" and names only `<topic>#<preamble>` as the key it leaves out. It does not mention the pointer key.
3. `src/server.rs:14058`, in a test doc comment, says `tracker-conventions` "declares nothing, so it ships WHOLE". It does not ship whole: the guide is about 60 KB and the bound is 16 KiB.

The runtime shows the real behaviour. Measured 2026-10-06 at `10e935e3`: a `doc(action="find")` result in a fresh fork carried `_guide_hint: "Guide 'tracker-conventions' is too large to auto-inject and was NOT delivered. Call `get_guide("tracker-conventions")` to read it."` plus the pointer block, and no guide body.

## Reproduction

```
git rev-parse --short HEAD    # 10e935e3, branch experiments
```

1. Run `grep -n "#<pointer>" -r src/`. The key appears only in `src/tools/core/guide_emit.rs`.
2. Read the three locations above.
3. Run any `doc` call whose result names a `docs/trackers/` or `docs/issues/` path, in a session that has not read `tracker-conventions`. The pointer shows once.

## Environment

Linux, `experiments` at `10e935e3`.

## Root cause

The pointer change (`MAX_AUTO_INJECT_GUIDE_BYTES` in `src/tools/core/guide_emit.rs`) added the key and documented it only in `guide_emit.rs:133-141`. The other places that enumerate key shapes were not found by a search for the new name, because they do not contain it.

Measured 2026-10-06: the `grep` above and the live pointer.

## Evidence

The archived record `docs/issues/archive/2026-09-24-a-guide-in-a-result-the-harness-saves-to-disk-is-marked-delivered-unread.md` says in its "Doc drift" bullet and again in its Resume that the key "is missing from the ledger-key tables in `src/engines/mod.rs` and from the `ledger_keys()` docs in `src/tools/guide_index.rs`, and the test doc comment ... still says `tracker-conventions` "ships WHOLE"". It listed this as a small follow-up and did not file it.

## Hypotheses tried

None needed.

## Fix

Not started. Add the pointer shape to the first two comments. Reword the third to say the topic ships a pointer while over the bound.

## Tests added

N/A — comments only.

## Workarounds

None needed. Read `guide_emit.rs:133-141` for the real key shapes.

## Resume

Three comment edits. No behaviour changes.

## References

- `docs/issues/archive/2026-09-24-a-guide-in-a-result-the-harness-saves-to-disk-is-marked-delivered-unread.md`.
- Cluster `IC-11`: the prose was true when written, and the code gained a delivery shape later.
