---
status: open
opened: 2026-08-20
closed:
severity: medium
owner: marius
related: []
tags:
  - usage-db
  - telemetry
  - friction
kind: bug
---

# BUG: friction_target's key list omits `command` and `file_path`, so 38% of errors — including both largest families — are unattributable

## Summary

`extract_friction_target` coalesces the first non-empty value from
`["name_path", "symbol", "name", "query", "path", "pattern"]`. `run_command`'s input key is
`command`; `artifact`'s is `id`; several tools accept `file_path` as the documented alias
for `path`. None are in the list, so those calls record `friction_target = NULL` even on
failure. The two largest error families in the corpus are both `run_command` families and
are therefore **0% attributable**, which silently defeats any per-target friction grouping.

## Symptom (Effect)

Measured 2026-08-19/20 over `.codescout/usage.db`: `run_command` errors **431/431 NULL**,
`artifact` **88/88 NULL**, `memory` **8/8 NULL** — about **38% of all errors** with no
target. Within that, the two highest-volume families:

```
il3_pipe_to_trimmer   256 errors,   0 with a friction_target
il3_shell_on_source   168 errors,   0 with a friction_target
```

The field is populated and meaningful for `symbols`/`edit_code`/`read_file`, so a query
grouping by `friction_target` returns plausible-looking results that omit the largest
populations entirely — a zero that is evidence about the predicate, not about the world.

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

Not yet implemented. Two parts, separable:

1. **Extend `KEYS`** with `file_path`, `id`, `topic`, and `command`. `command` deserves
   thought: the whole command string is a poor grouping key (it varies by flags), so the
   useful target is probably the first path-shaped token in it, or the executable name.
   Storing the raw string is still strictly better than NULL, since `input_json` is
   available for refinement.
2. **Make the miss observable.** A tool whose input matched no key is currently
   indistinguishable from one with no target. A sentinel (or a counter) would have surfaced
   this without a census.

Backfill is possible and cheap: `input_json` is populated on ~99% of rows, and a
reconstruction calibrated at 100% agreement against all 952 non-null stored values in one
analysis pass — so historical rows can be repaired rather than written off. Note the
`err_family` backfill already owns `PRAGMA user_version` (`src/usage/db.rs:485-525`), so a
second backfill needs its own gate rather than reusing that marker.

Record the fix SHA **and** its patch-id (`git show <sha> | git patch-id --stable`).

## Tests added

None yet. `extract_friction_target_coalesces_input_keys`
(`src/usage/mod.rs:274-290`) already pins the existing precedence and asserts
`{"unrelated": 1} → None`; it should gain cases for `command`, `file_path`, `id` and
`topic`, and the `None` case should be narrowed to an input that genuinely has no address.

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

Extend `KEYS` at `src/usage/mod.rs:181` and add the four test cases to
`extract_friction_target_coalesces_input_keys` at `src/usage/mod.rs:274`. Decide the
`command` representation first — raw string versus first path token — because the backfill
has to use the same rule as the live path or the two populations will not be comparable.

## References

- `src/usage/mod.rs:180-190` — `extract_friction_target`
- `src/usage/mod.rs:81-86` — the `is_friction` gate
- `src/usage/mod.rs:274-290` — the existing precedence test
- `src/usage/db.rs:485-525` — `err_family` fingerprint, which already occupies `user_version`
