---
kind: bug
status: fixed
title: 'BUG: friction_target dropped the documented path aliases (file_path, rel_path), losing 57 file targets'
tags:
- usage-db
- telemetry
- friction
closed: 2026-08-20
opened: 2026-08-20
owner: marius
related: []
severity: medium
unverified: Historical rows keep their NULL friction_target — no backfill was run, so every figure computed over rows written before db76f69a still understates attribution. The `command`-addressed population (438 rows) remains target-less BY DECISION, not by defect.
---

# BUG: friction_target's key list omits `command` and `file_path`, so 38% of errors — including both largest families — are unattributable

## Summary

`extract_friction_target` coalesced six input keys, and `path` was the only spelling of "a
file" among them. `file_path` and `rel_path` are documented **aliases** of it on the tools
that accept them, so a call that used the alias recorded no target at all — 57 error rows
on this project, every one a file `legibility::recorder_lane` should have been able to join
to a candidate and could not.

**The original framing of this bug was wrong in two ways, both corrected below.** Its
headline figure (38% of errors unattributable) was relayed from an analysis pass without
being recomputed; the real share is **52.5% (596/1135)**. And it asked for `command` to be
added, which running the reproduction first showed to be wrong — see *Fix*, item 2.

The filename still says `omits-command-and-file-path`. That is deliberate: the slug is
cited from `capability-proposals:CAP-9`, the `infra/friction-measurement` memory, and three
commit messages, and re-pointing all of them to sharpen a slug is churn against a stable
path. The title and body carry the corrected finding.
## Symptom (Effect)

Measured 2026-08-20 over `.codescout/usage.db` (1,135 errors in the 30-day retained corpus):

```
total_errors  no_target  pct
        1135        596  52.5
```

Of those 596, the keys that *were* present but unread:

```
has_command  has_id  has_topic  has_file_path  has_rel_path  has_cwd  rows_
        438      83          5             51             6        1    596
```

The alias half, by tool — and **all 51 `file_path` rows carried no `path` at all**, so
these are not duplicates of an already-attributed call:

```
  tool_name    n   also_no_path
-------------  --  ------------
read_file      31            31
edit_file      10            10
read_markdown   4             4
edit_markdown   4             4
edit_code       2             2
artifact (rel_path)          6
```

The field is populated and meaningful for `symbols`/`edit_code`/`read_file` when the
canonical spelling is used, so a query grouping by `friction_target` returns
plausible-looking results that silently omit the alias callers — a zero that is evidence
about the predicate, not about the world.
## Reproduction

```
# HEAD at filing: b4ea12fd989dfc2cbf1604be36090ddd3c99a6a3 (experiments)
sqlite3 -column -header .codescout/usage.db "
  SELECT tool_name, COUNT(*) errs, SUM(friction_target IS NOT NULL) with_target
  FROM tool_calls WHERE outcome='error'
  GROUP BY tool_name ORDER BY errs DESC;"
```

`run_command`, `artifact` and `memory` show `with_target = 0` against non-zero `errs`.

## Environment

Linux; codescout `experiments` at `b4ea12fd`.

## Root cause

`src/usage/mod.rs:180-190`:

```rust
fn extract_friction_target(input: &Value) -> Option<String> {
    const KEYS: [&str; 6] = ["name_path", "symbol", "name", "query", "path", "pattern"];
    ...
}
```

The list was written for the symbol-navigation tools and never extended as
`run_command`, `artifact` and `memory` grew their own address-shaped inputs. There is no
fallback and no diagnostic: a tool whose input matches no key yields `None`, which is
indistinguishable from a tool that legitimately has no target.

A second, separate gate compounds it — `src/usage/mod.rs:81` sets
`is_friction = overflowed || outcome != "success"` and extraction is conditioned on it, so
successful calls never get a target at all (measured: 0 of 25,696 non-overflowed
successes). That gate is deliberate and is *not* what this bug is about, but the two
together mean per-target analysis is confined to errors on a subset of tools.

measured 2026-08-20: the query above, plus
`symbols(path="src/usage/mod.rs", name="extract_friction_target", include_body=true)` read
directly.

## Evidence

### The key list versus the real input keys

| Tool | Address-shaped input key | In `KEYS`? |
|---|---|---|
| `symbols` | `name_path`, `name`, `path` | yes |
| `grep` | `pattern`, `path` | yes |
| `run_command` | `command` | **no** |
| `artifact` | `id`, `rel_path`, `new_rel_path` | **no** |
| `memory` | `topic` | **no** |
| several | `file_path` (documented alias of `path`) | **no** |

### Downstream effect measured

A detector-validation pass over 840 matched errors found that any signal keyed on
`friction_target` is structurally blind to this 38%, and that two candidate detectors
(`target_thrash`, `route_around`) could not exceed the base friction rate partly for this
reason. Reported by that pass; the NULL counts above are directly measured.

## Hypotheses tried

1. **Hypothesis:** the NULLs are calls whose inputs genuinely have no target.
   **Test:** sampled `input_json` for `run_command` error rows. **Verdict:** rejected —
   every one carries a `command` string, and many name a concrete path inside it.
   **Evidence:** *The key list versus the real input keys*.

## Fix

**FIXED 2026-08-20 on `experiments` in `db76f69a`**
(patch-id `bac32905dd0eb7ebb2fc156cb651723cbd2be00a`), at `src/usage/mod.rs:178-210`.

**1. The aliases are in.** `KEYS` goes from 6 to 8, with `file_path` and `rel_path`
immediately after `path` — the same concept, not a new one — so the canonical spelling
still wins when a caller sends both, and neither outranks `name_path`/`symbol`.

**2. `command` is deliberately OUT, and that is the part worth reading.** This file
originally prescribed adding it. Running the reproduction before re-reading the plan is
what stopped it: `command` is the largest target-less population by volume (438 of 596),
so adding it would have closed most of the gap by count while being wrong three ways.

- The field is documented as *the symbol/path a call addressed*. A shell command is neither.
- The sole consumer, `legibility::score_and_rank` (`src/legibility/mod.rs:290-334`),
  iterates **structural** defects and looks friction up by `name_path` then `rel_file`. A
  command string is therefore **inert** there — stored, never matched, never surfaced.
  (Checked before deciding: the join direction is what makes extra keys harmless rather
  than corrupting, so this is also why adding the aliases is safe.)
- `input_json` is populated on ~99% of rows, so the command is already recoverable at query
  time. Storing a derived form buys nothing and makes one column mean two things.

A whole command also varies by flags (groups badly) while the executable name discards what
was addressed — neither is *the target*. If a consumer ever needs per-command grouping, give
it its own column rather than widening this one's contract.
`extract_friction_target_ignores_shell_commands` records that as a decision.

**3. NOT done — historical backfill.** Existing rows keep their NULL targets.
`input_json` makes repair possible, but `PRAGMA user_version` is already the `err_family`
taxonomy fingerprint (`src/usage/db.rs:485-525`), so a second backfill needs its own gate.
Deliberately a separate change; see `unverified:` in the frontmatter.
## Tests added

Both in `src/usage/mod.rs`, written **before** the fix (the alias test failed red on
`file_path` → `None`, the exclusion test passed green as a behaviour pin):

- `extract_friction_target_reads_the_documented_path_aliases` (`src/usage/mod.rs:313-350`)
  — `file_path` and `rel_path` yield targets; `name_path` still outranks an alias; `path`
  wins over `file_path` when both are sent, so the precedence is a decision rather than an
  accident.
- `extract_friction_target_ignores_shell_commands` (`src/usage/mod.rs:352-384`) — `command`
  and `cwd` yield `None`. This is a **negative** test whose doc comment carries the
  reasoning, so a later reader finds a decision instead of a gap and does not "fix" it.

The pre-existing `extract_friction_target_coalesces_input_keys` still passes unchanged.
Gate: `cargo fmt` clean, `cargo clippy --all-targets -- -D warnings` clean, `cargo test`
**4,278 passed / 45 ignored / 0 failed**.
## Workarounds

Recover the target from `input_json` at query time:

```sql
SELECT COALESCE(friction_target,
                json_extract(input_json,'$.command'),
                json_extract(input_json,'$.file_path'),
                json_extract(input_json,'$.id'),
                json_extract(input_json,'$.topic')) AS target
FROM tool_calls WHERE outcome='error';
```

## Resume

N/A for the alias defect — fixed and archived.

Two follow-ups deliberately left, neither owed by this bug:

1. **Historical backfill.** Reconstruct targets for existing rows from `input_json` using
   the *same* key list and precedence as the live path, or the two populations are not
   comparable. Needs its own gate — `PRAGMA user_version` is taken by the `err_family`
   fingerprint (`src/usage/db.rs:485-525`).
2. **The (tool × target-kind) unit.** `friction_target` is the missing half of the unit that
   would fix the fractal mix confound recorded in `capability-proposals:CAP-9` — a per-tool
   error rate is still confounded by *which kind of target* the tool was pointed at
   (`read_markdown` on ordinary docs vs on managed artifacts have different base rates).
   This fix narrows the gap; it does not close it, because `command`-addressed calls
   deliberately still have no target.
## References

- `src/usage/mod.rs:180-190` — `extract_friction_target`
- `src/usage/mod.rs:81-86` — the `is_friction` gate
- `src/usage/mod.rs:274-290` — the existing precedence test
- `src/usage/db.rs:485-525` — `err_family` fingerprint, which already occupies `user_version`
