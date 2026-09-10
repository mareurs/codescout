---
kind: bug
status: fixed
tags:
- cluster/doc-contradicted-by-code
closed: null
fix_patch_id: 371bee7c5081481311866cd797d7591abb2c5bc3
fix_sha: dac1068a5e9f9f3265da091dcda1fa20f9da8592
fixed: 2026-09-09
opened: 2026-09-02
owner: marius
related: []
severity: low
---

# BUG: `workspace`'s schema requires `action`, and its own `post_compact` contract, the code, and the companion hook all omit it

## Summary

The advertised `workspace` schema lists `action` under `required`. The same schema's
`post_compact` description says *"Implies action='status' when action is omitted"*, the code
implements exactly that, and the companion plugin's post-compaction banner instructs
`workspace(post_compact=true)` with no action. Thirty-one calls in the last thirty days took
that path and succeeded. The `required` declaration is false, and it works today only because
Claude Code does not enforce `required` client-side.

## Symptom (Effect)

Wire schema (`tools/list`, 2026-09-02, `target/debug/codescout` via
`scripts/probe_tool_surface.py`):

```json
"required": ["action"],
"properties": {
  "action": {"enum": ["activate","status","list_projects"], "description": "Operation to perform."},
  "post_compact": {"type": "boolean",
     "description": "Flush all LSP clients after context compaction. Implies action='status' when action is omitted."},
  …
}
```

The companion hook, `../claude-plugins/codescout-companion/hooks/session-start.mjs:338`:

```
→ Call workspace(post_compact=true) as your FIRST action to flush stale LSP position caches.
```

usage.db, 30 days to 2026-09-02:

```
SELECT count(*) FROM tool_calls WHERE tool_name='workspace' AND input_json LIKE '%post_compact%';                                        → 120
SELECT count(*) FROM tool_calls WHERE tool_name='workspace' AND input_json LIKE '%post_compact%' AND input_json NOT LIKE '%"action"%';   → 31   (all error_msg NULL)
```

## Reproduction

`git rev-parse HEAD` → `4dc0daa2`.

```
python3 scripts/probe_tool_surface.py --json | python3 -c "import json,sys; t=[x for x in json.load(sys.stdin) if x['name']=='workspace'][0]; print(t['inputSchema']['required'])"
```

→ `['action']`. Then `workspace(post_compact=true)` → a status response, no error.
(Note: the call flushes every LSP client; clients restart lazily on the next LSP call.)

## Environment

Linux, codescout `experiments` @ `4dc0daa2`, Claude Code over stdio. Claude Code is the
only client measured; a client that validates `required` would refuse the hook's call.

## Root cause

`src/tools/config/mod.rs:46` declares `"required": ["action"]`. `src/tools/config/mod.rs:51-57`:

```rust
let post_compact = input.get("post_compact").and_then(|v| v.as_bool()).unwrap_or(false);
let action = match input.get("action").and_then(|v| v.as_str()) {
    Some(a) => a,
    None if post_compact => "status",
    None => { return Err(RecoverableError::with_hint("workspace requires 'action' parameter", …)) }
```

The code's contract is *"action, or post_compact"*. The schema's is *"action"*. No test
compares a tool's `required` list against what its `call` actually refuses without:
`all_tools_have_valid_schemas` (`src/server.rs`) checks `is_object` and `type == "object"`;
`param_probe::assert_required_are_advertised` (`src/librarian/tools/mod.rs:578`) runs the
other direction — required-by-code ⇒ advertised — and only over the librarian family.

Measured 2026-09-02: the wire dump, the two usage.db counts, the hook line.

## Evidence

### Wire
`scripts/probe_tool_surface.py` output, saved at
`/tmp/claude-1000/-home-marius-work-claude-codescout/2cb44cd3-8673-4604-a8ac-5adea75ca54b/scratchpad/tools_list.json`.

### The live caller
`session-start.mjs:338` (companion plugin), and its test `session-start.test.sh:233` which
asserts the banner contains exactly `workspace(post_compact=true)`.

### usage.db
120 / 31 as above; the 31 span 2026-08-20 … 2026-08-27 at least, every one OK.

## Hypotheses tried

1. **Hypothesis:** Claude Code fills `action` from the enum when `required` is unmet.
   **Test:** the 31 rows — `input_json` has no `action` key at all. **Verdict:** rejected;
   the client passes the call through unvalidated.

## Fix

Fixed in `dac1068a` (patch-id `371bee7c5081481311866cd797d7591abb2c5bc3`) — Task 3 of the
tool-surface collapse. `"required": ["action"]` was removed from the `workspace` schema and the
`action` property's description now states the real contract: *"Operation to perform. Required
unless post_compact=true, which implies status."*

The same commit shipped the schema-shape gate this file's sibling asked for —
`required_names_no_key_that_has_a_declared_alias` (`src/server.rs:2842`) — so the class is
guarded rather than only this instance repaired.

**Verified 2026-09-09 at the runtime, not only in the diff:** `workspace(post_compact=true)` with
no `action` returns `{"flushed": true, …}`. `src/tools/config/mod.rs:45-62` carries no `required`
array for this tool — the `"required": ["path"]` at `:161` belongs to a different tool and is not
this one's.

**Stale repro line, noted so the next reader does not lose time:** this file's § *Reproduction*
pipes `scripts/probe_tool_surface.py --json` into a snippet that indexes the result as a list of
tools. The probe now returns an object (`surface`, `calls`, `total`, `schema`, `desc`, `annot`,
`unmodelled`, `prose`, `machine`), so that line raises `TypeError` rather than printing the
required array. Read the schema at `src/tools/config/mod.rs` instead.

**Cause A is NOT closed by this.** The sibling
`docs/issues/2026-09-02-advertised-required-overstates-what-the-code-enforces.md` owns the alias
half and stays open; the gate existing is not the same claim as the alias instances being gone.
## Tests added

None yet. Owed: the bidirectional `required` probe above; a `workspace`-specific pin that
`{post_compact: true}` with no `action` succeeds and `{}` fails.

## Workarounds

None needed today — the client does not enforce `required`.

## Resume

Edit `src/tools/config/mod.rs:46` and the `action` description; write the registry-wide `required`
probe under `src/server.rs` tests; `cargo test --lib workspace`.

## References

- `../claude-plugins/codescout-companion/hooks/session-start.mjs:338`
- `docs/trackers/prompt-surface-compaction-session-log.md`, 2026-09-02 review section.
- `docs/trackers/issue-clusters.md` `IC-11`.
