---
kind: bug
status: fixed
tags:
- cluster/declared-not-wired
closed: 2026-09-01
opened: 2026-09-01
owner: marius
related: []
severity: medium
---

# BUG: `artifact(action="graft")` requires two params the schema never advertises, so the action is unusable as advertised

## Summary

`graft` is listed in `artifact`'s advertised action enum, but its two **required**
parameters — `from_id` and `into_id` — were absent from the tool's 53 advertised properties.
A caller reading `tools/list` had no way to learn what the action needs. The single real-world
attempt in `usage.db` failed with `missing_required_param`.

## Symptom (Effect)

From `.codescout/usage.db`, the one recorded `graft` call:

```
tool_name  action  outcome  err_family              msg
artifact   graft   error    missing_required_param  graft requires 'from_id' and 'into_id': missing field `from_id`
```

The error message is good — it names both fields. The defect is that nothing *before* the
failed call could have told the caller, because the schema is the only discovery surface and it
did not carry them.

## Reproduction

Against `8982f775` (pre-fix), driving the real binary over stdio JSON-RPC:

```
python3 scratchpad/schema_probe.py     # initialize -> notifications/initialized -> tools/list
#   artifact advertises 53 params
#     from_id      advertised=False
#     into_id      advertised=False
```

Then `artifact(action="graft")` with any argument set fails, because no advertised key maps to
either required field.

## Environment

Linux, Rust, codescout `0.15.0`, branch `experiments`, MCP stdio transport. Feature `librarian`
enabled (the action does not exist without it).

## Root cause

Two independent representations of "what `graft` requires", and nothing compared them.

- `src/librarian/tools/graft.rs:9-12` declares `struct Args { from_id: String, into_id: String }`
  — both non-`Option`, no `serde(default)`, so both are required at deserialisation.
- `src/librarian/tools/artifact.rs:35` lists `graft` in the action enum, and
  `Artifact::input_schema` (hand-written `json!()`, `artifact.rs:28-205`) declared neither key.

**Measured 2026-09-01:** `python3 schema_probe.py` against the live wire → `artifact` advertises
53 params, `from_id`/`into_id` absent; `sqlite3 .codescout/usage.db` → 1 `graft` call, 1 error,
`err_family=missing_required_param`. Not inferred from source alone.

**Why the existing guard could not see it.** `every_action_labelled_schema_key_is_honored_by_that_action`
(`artifact.rs`) iterates `schema["properties"]` and asks whether each advertised key is honored
— **schema→action**. A key the schema never advertises is not in that iteration, so the test is
structurally blind to this direction.

Sharper, and the part worth keeping: **that test's own `required()` helper hardcoded
`from_id`/`into_id` for `graft`.** The knowledge that would have caught the bug was written
down, out-of-band, inside the test that missed it — and supplying the params out-of-band is
precisely what let the probe's base call succeed while the schema stayed silent. Two
representations, one truth, drift silent (`system-retrospective-improvements:T-6`).

## Evidence

### The two representations, side by side

`src/librarian/tools/graft.rs:9-12` (what the code requires):

```rust
#[derive(Deserialize)]
struct Args {
    from_id: String,
    into_id: String,
}
```

`src/librarian/tools/artifact.rs` (what the test knew, while the schema did not):

```rust
"graft" => {
    m.insert("from_id".into(), json!(NO_SUCH_ID));
    m.insert("into_id".into(), json!("1111111111111111"));
}
```

### The new guard, red before the fix

```
artifact: these params are REQUIRED by an action's Args but are not advertised in the
schema, so a caller cannot discover them and the action is unusable as advertised — the
inverse of IC-15, and invisible to the forward sweep. Add them to input_schema:
["graft:from_id", "graft:into_id"]
```

That red was **earned, not staged** — the test was written before the fix and failed on the
real defect, which is the deliberate break CLAUDE.md § *Testing Discipline* asks for.

## Hypotheses tried

1. **Hypothesis:** this is one instance of a wider class, and other actions have the same gap.
   **Test:** cross-checked every `Args` struct under `src/librarian/tools/` with a required
   (non-`Option`) field against the advertised schemas — `link` (`src_id`/`dst_id`), `mv`
   (`id`/`new_rel_path`), `append_entry` (`id`/`id_prefix`), `update_entry`
   (`id`/`entry_collection`), `merge_worktree` (`root`), `graph`/`get`/`delete`/`refresh` (`id`).
   **Verdict:** rejected — `graft` was the only instance. All other required params were
   already advertised.

2. **Hypothesis:** this is the third instance of *advertised ≠ accepted* and therefore trips the
   `tool-registration-rule-of-three` in `docs/superpowers/specs/2026-08-18-tool-surface-budget-design.md`,
   which would license replacing hand-written schemas with `schemars` derivation.
   **Test:** checked the spec's two claimed instances. F-1 (`anchor_heading`) is confirmed and
   since fixed. The second, listed as unverified — `query` / `title_contains` / `preview` — was
   checked by grepping every `Args` under `src/librarian/tools/`.
   **Verdict:** rejected, in the *opposite* direction from expected. No `Args` accepts any of
   those three (sole hit is an unrelated fn parameter, `src/librarian/tools/get.rs:35`), so they
   are neither advertised **nor** accepted — agents guessed the names and serde dropped them.
   That is `IC-15` silent-drop, a different class. Net: **2 confirmed, 1 live — the trigger has
   not fired.** Recorded as `system-retrospective-improvements:T-10`, because the spec's count
   was wrong in both directions at once and an inflated count fires the rewrite early exactly as
   a stale one never fires it at all.

## Fix

Two parts, and the second is the one that matters.

1. **Instance** — added `from_id` / `into_id` to `Artifact::input_schema`
   (`src/librarian/tools/artifact.rs`), action-labelled `graft:` so the existing forward sweep
   now probes them too. Surface cost +393 chars (54,583 → 54,976; headroom 1,936 → 1,543),
   which is an *addition* and is justified under the subtract-and-measure protocol's P-3 by the
   measured deficit above.

2. **Class** — added `param_probe::assert_required_are_advertised`
   (`src/librarian/tools/mod.rs`), the reverse direction of the existing `sweep`. It asserts the
   per-action `required()` table against `schema["properties"]`. It deliberately introduces **no
   third list**: `Spec::required` already states what each action requires, and `sweep` depends
   on it being complete (its base call must survive deserialisation), so the two representations
   already existed and this asserts they agree. Wired at **all four** probe sites — `artifact`,
   `librarian`, `artifact_event`, `artifact_refresh` — because one kill proves one site.

**Known limitation, stated because it is monotone under omission:** an action missing from
`Spec::required` altogether contributes no keys and is passed over silently. `sweep` catches
part of that indirectly (a base call dying at deserialisation makes base and probe outcomes
match, so the action's labelled keys report as unhonored), but only for keys carrying an
`<action>:` label. Adding a new action means adding it to `required`; neither assertion will
remind you. This is recorded at the function's doc comment, not only here.

**Fix commit:** `6894b67d` on **`experiments`** (full: `6894b67d07d354fc25876483fd068fc56062e60d`).
**patch-id:** `3cb9bc68a685c46252388dc21a3dd8d7beff9098` (`git show 6894b67d | git patch-id --stable`).

Both are recorded because they fail differently: the SHA is positional and dies when
`experiments` is rebased, while the patch-id is a content hash of the diff that survives rebase
*and* cherry-pick. Nothing is owed later — there is no promotion path to check.

## Tests added

- `librarian::tools::artifact::tests::every_required_param_is_advertised`
  (`src/librarian/tools/artifact.rs`) — the named regression test. Written before the fix; red
  on `["graft:from_id", "graft:into_id"]`, green after.
- The same assertion at three further sites, inside each tool's existing probe test:
  `src/librarian/tools/librarian.rs`, `src/librarian/tools/artifact_event.rs`,
  `src/librarian/tools/artifact_refresh.rs`.
- `param_probe::assert_required_are_advertised` also asserts `checked > 0`, so a `Spec` that
  supplies no keys for any action fails loudly rather than passing while checking nothing —
  the same monotone-under-convention-break reasoning that gives `assert_all_honored` its floor.

Gate green at fix time: clippy `--workspace --all-targets --features local-embed -- -D warnings`
clean; lean lane 3408 passed / 0 failed; default lane 4984 passed / 0 failed.

## Workarounds

Pass `from_id` and `into_id` anyway — they were always honored, just undiscoverable. The error
message names both fields, so a caller who attempts the action once learns the shape from the
failure.

## Resume

N/A — fixed and archived. Gate green at `6894b67d` (clippy clean, lean 3408/0, default 4984/0),
regression test `every_required_param_is_advertised` in place and shown able to fire.
## References

- `docs/superpowers/specs/2026-08-18-tool-surface-budget-design.md` (`0e0316e9036d7f16`) —
  § *Rejected* / § *Revisit-when*, whose instance count this bug corrects.
- `docs/trackers/system-retrospective-improvements.md` (`6f5ec09c63aef864`) — T-8, T-9, T-10.
- `docs/trackers/prompt-surface-compaction-session-log.md` (`03464a8808345846`) — F-1, the first
  confirmed instance of this class.
