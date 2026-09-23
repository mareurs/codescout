---
kind: bug
status: fixed
tags:
- cluster/accepted-parameter-silently-dropped
closed: null
opened: 2026-09-13
owner: marius
related: []
severity: low
---

# The ledger hook silently ignores an unrecognised flag

## Summary

`scripts/pre-commit-ledger-counts.py` parses its arguments with a flat `for arg in sys.argv[1:]`
loop of `if`/`elif` arms and **no `else`**. An unrecognised flag is neither consumed nor reported:
the loop skips it and the script falls through to the normal check path.

The realistic cost is a mistyped `--fixture-*` flag in a Rust test. Instead of failing with
"unknown flag", the script runs the full check suite against the live index and returns whatever
those checks decide — so the test compares against the wrong thing, or reads a refusal as a
fixture answer.

## Symptom (Effect)

Measured 2026-09-13:

```
$ echo '| IC-1 | a | `b` | c | d |' | python3 scripts/pre-commit-ledger-counts.py --fixture-index-mechanisms
exit=1                       # typo: trailing 's'. No error. Ran the real checks instead.

$ echo '| IC-1 | a | `b` | c | d |' | python3 scripts/pre-commit-ledger-counts.py --fixture-index-mechanism
{"extra": [["IC-1", "d"]], "header": null}
exit=0
```

The two differ by one character. The first produced no diagnostic naming the flag.

**A caveat on that exit code, because it misled me first.** The `exit=1` above is not the flag
being rejected — it is a real check firing on the index. So the failure mode is worse than a bare
"exits 0": the exit code is *whatever the corpus happens to make it*, which on a clean tree is 0
and on a dirty one is 1, and neither is about the flag. A test asserting `status.success()` after
a typo'd flag can pass, fail, or flip between runs depending on unrelated ledger state.

## Root cause

`main`'s argument loop has terminal arms for each `--fixture-*` flag (each reads stdin, prints,
`return 0`) and assignment arms for `--source=` and `--json`, with nothing matching the default
case. The `--source` value *is* validated immediately afterwards —

```python
if source not in ("index", "worktree", "head"):
    raise SystemExit(f"--source must be index|worktree|head, got {source!r}")
```

— so the file already establishes that a bad *value* is worth refusing. A bad *flag* is not
checked by the same standard, which is the inconsistency rather than an oversight in principle.

## Suggested fix

**FIXED 2026-09-19** — `562f24d069916182a680a15b3aa6eefa26c40cf0`, patch-id `3698108fc08c31f6fe496d862843866400736541`. A trailing `else` in `main()`'s argv chain exits naming the unrecognised flag. All eight caller surfaces were enumerated before adding the refusal — three peer sessions share this checkout and a new refusal that reds someone's commit would be worse than the bug. The regression test uses a plausible typo (`--fixture-ledgre`) rather than an obviously bogus token, because that is the bug's actual shape, and asserts stderr names the flag rather than merely exiting non-zero.

One line, and it should be cheap to get right:

```python
else:
    raise SystemExit(f"unknown flag {arg!r}")
```

**Check the callers before shipping it.** `scripts/pre-commit-run.sh` and any other invoker must
not be passing a flag this script currently ignores — if one is, the fix turns a silent no-op into
a refused commit for everyone, which is a worse failure than the one being fixed. Grep the
invokers first; that check is the actual work here, not the `else`.

## Fix provenance

- **SHA:** `562f24d069916182a680a15b3aa6eefa26c40cf0` (`experiments`)
- **patch-id:** `3698108fc08c31f6fe496d862843866400736541`

Formalised 2026-09-23 from a pair this record already stated in § *Fix* prose. `doctor`'s
`terminal_status_without_fix_anchor` cannot parse a hash in running text, so the record read as
anchored while nothing resolved it -- which is the failure mode that check exists to name. The
pair was not guessed: the SHA resolves and is an ancestor of `experiments`, and its patch-id
recomputed from the diff equals the one this file already carried. The SHA is positional and
dies on the next rebase of `experiments`; the patch-id is a content hash of the diff and
survives rebase and cherry-pick, which is why both are recorded rather than either.

## Workarounds

None needed — the failure is confined to authoring new fixture-driven tests, and shows up as a
confusing test result rather than corrupt data.

## Tests added

None yet. A regression test is one line of `assert!(!status.success())` on a deliberately
misspelled flag, and it belongs beside the four existing `--fixture-*` drivers.

## References

- `scripts/pre-commit-ledger-counts.py` — `main`'s argument loop and the `--source` validation
  immediately below it.
- Found while adding a fifth `--fixture-*` arm for `no_index_row_stores_a_mechanism`. First
  surfaced by a subagent survey of the script's CLI surface; the survey reported "exits 0", which
  was right about the mechanism and incidental about the code — re-measuring is what turned up the
  sharper claim that the exit code tracks unrelated corpus state.
