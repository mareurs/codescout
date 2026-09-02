---
id: '2fbf7181f0621794'
kind: bug
status: open
title: A test fixture interpolates a tempdir path into JSON, so its validity depends on the path's alphabet
tags:
- cluster/assertion-satisfiable-by-accident
topic: cross-platform test fixtures
closed: null
opened: 2026-09-02
owner: marius
related: []
severity: medium
---

## Summary

Two test fixtures built an audit payload by interpolating a filesystem path into a JSON
string literal:

```rust
let gone_path = tmp.path().join("gone.md").to_string_lossy().to_string();
format!("… VALUES(…,'{{\"abs_path\":\"{gone_path}\"}}')")
```

On Unix that path is `/tmp/.tmpAbC/gone.md` and the JSON is valid. On Windows it is
`C:\Users\RUNNER~1\AppData\Local\Temp\.tmpAbC\gone.md`, where `\U`, `\A` and `\T` are
**not legal JSON escapes** — so `serde_json::from_str` inside `attribute` fails, the row
resolves to `None`, and `export` reports it `unattributed`.

**The fixture's validity depended on the alphabet of a tempdir name.** It passed on every
developer machine and on every ubuntu and macos lane, and failed on all four Windows
lanes. That is `IC-9` in its own words: *an assertion whose haystack embeds
environment-controlled text — a path, a tempdir name — can be satisfied by coincidence.*

## Symptom (Effect)

The last 2 of the 21 Windows failures, surviving after
`docs/issues/archive/2026-09-02-lockfileex-refuses-an-append-only-handle-on-windows.md` cleared
the other 19 (run `33574961971`, `windows-latest / default`):

```
delete_row_is_attributed_from_its_payload_not_a_live_join
  assertion failed: a delete row with a usable payload must not fall through to
  unattributed: ExportReport { exported: 0, …, unattributed: 1 }   left: 1  right: 0

rows_from_different_months_land_in_different_files
  read_dir(audit_dir(tmp)) -> Os { code: 3, kind: NotFound, "The system cannot find
                                   the path specified." }
```

**One cause, two shapes.** The first is the direct symptom. The second is downstream:
with nothing attributable, `export` writes nothing, and it creates the audit directory
only when it has something to write — so the test's `read_dir` finds no directory at all.

## Reproduction

Measured 2026-09-02, and it needs no Windows machine — only a Windows-*shaped* string:

```python
json.loads('{"abs_path":"/tmp/.tmpAbC/gone.md"}')            # parses
json.loads('{"abs_path":"C:\\Users\\RUNNER~1\\gone.md"}')    # Invalid \escape: column 16
```

That is what makes this bug different in kind from its sibling: the `LockFileEx` defect
is unobservable from Linux because `flock(2)` ignores access mode, so nothing local can
express it. **Here the trigger is the CONTENT of the path, not the platform**, so a
backslash-bearing string exercises it fully on any OS.

## Environment

`experiments` @ `6d89a69b`. Failing: all four Windows lanes. Green: every ubuntu and
macos lane, and the full local gate on Linux.

## Root cause

`src/librarian/catalog/audit/shard.rs`, two fixtures (`rows_from_different_months_…` and
`delete_row_is_attributed_from_its_payload_…`) constructing JSON with `format!` instead
of a serializer.

**Production was never affected, and that is worth stating explicitly rather than
assumed:** the catalog's audit triggers build payloads from SQLite expressions, and
`prune_before` uses `json_object()` over bound parameters (`audit/mod.rs`). No
string-concatenated JSON exists on any non-test path. Only hand-written fixtures could
reach this.

## Fix

**Fixed at `52cb0930`** (the audit fixtures) and **`0dd22d9f`** (a third instance, below).

**VERIFIED on run `33577964380`: all four Windows lanes green.**

| lane | before | after |
|---|---|---|
| `windows-latest / default` | 21 failed | **success** |
| `windows-latest / no-features` | 1 failed | **success** |
| `windows-latest / local-embed` | 1 failed | **success** |
| `Windows-gnu cross (MinGW + wine)` | 2 failed | **success** |

17 of 18 jobs green. The one failure, `Audit Doc Refs`, was already failing on runs
`33487717391` and `33455664305`, both of which pre-date every commit in this thread — so
it is a pre-existing red and not a residual of any of these fixes.

## A THIRD instance, in another file, found only because two lanes were never opened

`tests/config_propagation.rs` compared discovered worktree paths against forward-slash
literals **without normalising separators** — while its sibling test four lines away did:

```
windows-latest: [".worktrees\\wt-a", "checkout\\inner-wt", "deep\\a\\b\\c\\wt-b"]
compared against "checkout/inner-wt"
```

Same class, different mechanism: not a JSON escape but a platform separator, and again the
haystack was environment-controlled text that happens to be benign on Unix. Fixed by a
shared `rel_slash()` helper, so there is one place the `.replace` can be written rather
than two places to remember it — the same remedy shape as `delete_payload`.

**Why it went unseen for three runs is the reusable part.** Four Windows lanes were red
and only `windows-latest / default` was ever examined — the lane where this one failure
sat under twenty-one others. `no-features` and `local-embed` each had **exactly one**
failure, this test, and had reported it since run `33574961971`. That is the same error as
the 2-of-21 panic sample that produced this thread's retracted first diagnosis: examine a
subset, generalise, be wrong about the rest — twice in one investigation, the second time
with *lanes* as the subset rather than *panics*.

**Deliberately not filed separately.** A new tagged bug file moves `IC-9`'s count, which
reds the shared ledger gate while several sessions are mid-repair; per `codescout-5e`, it
would have to land coupled with its own bump. Recorded here instead, where it costs no
count — the class is the same and the file already carries the class.

**The regression test runs on Linux, which is the point.** `a_backslash_path_survives_the_delete_payload_round_trip`
feeds a Windows-shaped path through the helper and through `attribute`, asserting a
byte-for-byte round-trip. Verified by mutation 2026-09-02: reverting the helper to
`format!` reds it on Linux. The two original fixtures could not do this, because they
took their path from the environment instead of stating one.

## Hypotheses tried

1. **Hypothesis: the remaining 2 share a cause with the other 19 (`LockFileEx`).**
   **Verdict: rejected.** The lock fix cleared 19 and left these 2 with entirely
   different signatures — `unattributed: 1` and `NotFound`, neither mentioning access
   rights. Same module, same lane, two unrelated defects.

2. **Hypothesis: a path-comparison defect in attribution — separator or case sensitivity
   when matching `abs_path` against `repo_root`.**
   **Verdict: rejected on reading the code.** `attribute` never compares paths; it
   *parses* the payload and returns whatever `abs_path` it finds. The failure is upstream
   of any comparison, in JSON parsing.

## Provenance

These two were also the pair the **wine** lane failed in the very first run
(`33570342471`) — the 2-of-21 sample that produced a since-retracted diagnosis of the
other 19. So that sample was not merely small: it was drawn from the minority failure
mode and described a defect that was never causing the majority. Both are now identified
and separately fixed.
