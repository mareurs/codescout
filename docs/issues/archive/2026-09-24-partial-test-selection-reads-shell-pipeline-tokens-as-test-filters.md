---
id: a19ffc66415317a2
kind: bug
status: fixed
title: 'BUG: run_command''s partial-test-selection diagnostic reads shell pipeline tokens as cargo test filters'
tags:
- cluster/addressing-without-an-escape-hatch
closed: 2026-09-24
opened: 2026-09-24
---

# BUG: `run_command`'s partial-test-selection diagnostic reads shell pipeline tokens as cargo test filters

## Summary

`multi_filter_test_command` (`src/tools/run_command/output.rs`) takes every whitespace token
after the last `--` in a `cargo test` command that does not start with `-` as a test filter. So
`2>&1`, `|`, `grep` and grep's own pattern become "filters", and the diagnostic reports them as
having **matched NOTHING** on a run whose real filters all matched. A false alarm, attached to a
green run, on the command shape this repo's own tooling encourages for narrowing output.

## Symptom (Effect)

Observed 2026-09-24 on
`cargo test --lib -- entry_prefix_declared_twice row_checks_scoped_by_project 2>&1 | grep -E "\.\.\. |test result|^error|panicked at|^\s+-->"`:

```
"partial_test_selection": "This run named 9 test filters but 6 matched NOTHING in `cargo test … -- --list`:
 `2>&1`, `|`, `\"\\.\\.\\.`, `|test`, `result|^error|panicked`, `at|^\\s+-->\"`. The filters that DID
 resolve passed, and the exit code reports success over that narrowed set — it says nothing about the
 filter(s) that selected empty. Check for a rename, deletion or typo."
```

Both real filters matched (3 tests ran, 3 passed). Every one of the six "filters" is shell syntax.

## Reproduction

Any `cargo test … -- <f1> <f2> 2>&1 | grep <pattern>` through `run_command`, where `<f1>`/`<f2>`
both match.

**Second shape, MEASURED by a peer (sessionId `3b4fae98-500a-4fa9-8127-b16642a8c23d`, who hit this
independently on 2026-09-24 and withdrew a duplicate filing):** the scan does not stop at `;` either,
and counts redirect targets. On
`cargo test --lib zz_replay_recorded_corpus -- --ignored --nocapture > $S/replay_rs.log 2>&1; grep -E "REPLAY|test result|panicked" $S/replay_rs.log`
the diagnostic reported 7 filters, all 7 unmatched: `>`, `$S/replay_rs.log`, `2>&1;`, `grep`,
`"REPLAY|test`, `result|panicked"`, `$S/replay_rs.log` -- the ENTIRE following command. Note `2>&1;`:
the `;` is glued to the redirection, so a fix that only matches a bare `;` token still misses it.

Still predicted, not run: an `&&` chain (`… -- <f1> <f2> && echo done`).

## Root cause

`src/tools/run_command/output.rs` `multi_filter_test_command`:
`tokens[dash_idx + 1..].iter().filter(|t| !t.starts_with('-'))` — nothing ends the filter list at a
shell control operator or redirection. Its doc comment says "Heuristic, not a shell parser" and
budgets only for a quoted filter containing a space; shell operators are the unbudgeted case.
Read at the bytes 2026-09-24; the symptom above is the measurement.

The `--list` command it builds is unaffected (`tokens[..=dash_idx]`, the prefix before `--`), so
the listing is correct and only the diff against it is polluted.

## Fix

**FIXED 2026-09-24 at `aa045e32`.** The line is split into shell SEGMENTS first (`|` `||` `&&`
`;` `&`, with a GLUED trailing `;` split off — both `2>&1;` and `bb;` carry it, and `bb` is a real
filter), only the segment running `cargo test` is read, and a redirection (`2>&1`, `>file`, and a
bare `>`'s target) is not a filter.

**Found while fixing, not in the original filing:** `rposition("--")` over the whole line could
pick a LATER command's `--` (`; git log -- src`), so the `--list` command was built from the wrong
segment. Now scoped to the cargo segment, keeping any `cd sub &&` prefix so the listing runs in
the right directory.

Staged by blob: `output.rs` carried a live peer's unrelated uncommitted hunks, so the commit is
HEAD's file plus only this change, written with `git update-index --cacheinfo <mode> <blob> <path>`
(the separated form, which the stage-log attributes). No `--no-verify`.

## Tests added

`multi_filter_test_command_stops_at_shell_syntax` (piped grep, `&&`, `> file 2>&1`, and the
peer's exact `; grep` chain) and `multi_filter_test_command_lists_from_the_cargo_segment`, in
`src/tools/run_command/output.rs`. Both red first. Mutation via `scripts/mutation-probe.sh`, 6/6
KILLED.

## Fix provenance

- **SHA:** `aa045e32` (`experiments`)
- **patch-id:** `788a5253ab9bac67b55156df22c674d09f30187f`

## Resume

N/A — fixed.
