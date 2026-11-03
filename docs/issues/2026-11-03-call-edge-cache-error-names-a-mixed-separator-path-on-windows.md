---
id: '1ca433a98628cd17'
kind: bug
status: fixed
title: 'BUG: on Windows the call-edge cache error names the store as `<root>\.codescout/call_edges.db`, so `a_cache_error_names_the_store_and_the_step` fails on three Windows CI lanes'
tags:
- cluster/unclassified
- windows
- call_graph
closed: 2026-11-03
opened: 2026-11-03
owner: marius
severity: low
unverified: 'Verified under wine 11.20 (x86_64-pc-windows-gnu) only. Not yet seen green on a windows-latest MSVC runner: the fix is not pushed.'
---

# BUG: on Windows the call-edge cache error names the store as `<root>\.codescout/call_edges.db`, so `a_cache_error_names_the_store_and_the_step` fails on three Windows CI lanes

## Summary

`cache_error` (`src/tools/symbol/call_graph/mod.rs`) builds the store path as `root.join(".codescout/call_edges.db")`. On Windows `Path::join` puts `\` after the root but keeps the `/` inside the joined literal, so the message reads `/proj\.codescout/call_edges.db`. The test `a_cache_error_names_the_store_and_the_step` asserts the substring `/proj/.codescout/call_edges.db`, which holds only on unix. Both came in with `83af8a574` (bug `cd85dbae70c76670`).

## Class fit

No class on the roster was walked. `cluster/unclassified` is the sanctioned escape.

## Symptom (Effect)

- CI run 41450758460 on `origin/experiments` at `71d9995f1` (2026-11-02): `Test (windows-latest / no-features)` and `Test (windows-latest / local-embed)` fail on this test with `call_graph: reading the call-edge cache at /proj\.codescout/call_edges.db failed: no such table: call_edges`.
- The user-facing message on Windows names a path with mixed separators. It still points at the right file, so the cost to a user is cosmetic. The cost to the project is a red Windows lane that hides any other Windows failure.

## Reproduction

```
scripts/build-windows.sh test a_cache_error_names_the_store_and_the_step
```

Run 2026-11-03 under wine 11.20 (profile from `scripts/wine-env.sh`): exit 101, `0 passed; 1 failed`, panic at `src/tools/symbol/call_graph/mod.rs:846:9` with the same message as CI.

## Environment

codescout `experiments`, Windows targets only (`x86_64-pc-windows-msvc` on CI, `x86_64-pc-windows-gnu` under wine locally). Linux and macOS lanes pass.

## Root cause

A multi-segment literal passed to `Path::join`. Windows `Path::join` inserts its own separator only at the join point and does not rewrite separators inside the argument. The test then hard-codes the unix separator.

## Evidence

The CI log of run 41450758460 (jobs above) and the local wine run above. The same run's `Test (windows-latest / default)` and `Windows-gnu cross` lanes fail for a different reason, `cannot find function canon` in `src/librarian/read_door.rs:952`; that one is already repaired locally by `b3889b9f9`, which is not yet on `origin/experiments`.

## Hypotheses tried

None needed: the panic text shows the mixed separator directly.

## Fix

`cache_error` now joins the path one segment at a time: `root.join(".codescout").join("call_edges.db")`. Every separator in the message is then the platform's own.

## Fix provenance

- **SHA:** `6b4bfded88c0459c31c1553b750bb325bba4a301` (`experiments`, not pushed)
- **patch-id:** `4ca59027d31c0dfb03fd45aaae7a8045d9824e75`

## Tests added

`a_cache_error_names_the_store_and_the_step` now asserts the store path built from `std::path::MAIN_SEPARATOR`, so it is right on every platform.

- **Red** under wine with the old `cache_error`: `expected /proj\.codescout\call_edges.db in: call_graph: reading the call-edge cache at /proj\.codescout/call_edges.db failed: no such table: call_edges`.
- **Green** under wine after the fix, and on Linux. `./scripts/gate.sh` green: `FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0`, 5993 + 8844 tests passed.
- On Linux the new assertion passes with both the old and the new code, so only a Windows run can catch a regression.

## Workarounds

None needed for users. For CI, none.

## Resume

Found 2026-11-03 while reading CI for the unpushed range.

## References

- `src/tools/symbol/call_graph/mod.rs` (`cache_error`, `tests/a_cache_error_names_the_store_and_the_step`)
- Bug `cd85dbae70c76670` (the change that introduced both)
