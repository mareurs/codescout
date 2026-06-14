---
status: open
opened: 2026-06-14
closed:
severity: medium
owner: marius
related: []
tags: [librarian, artifact_augment, progressive-disclosure, run_command, discoverability]
kind: bug
---

# BUG: Large array-valued artifact params are un-updatable through the MCP surface (no server-side read path; inline cap blocks the round-trip)

## Summary
An agent cannot update an augmented artifact whose `params` hold an array that
exceeds the inline byte budget (~9 KB). `artifact_augment` accepts `params`
only as an inline JSON argument, RFC-7396 merge replaces arrays wholesale (no
entry-grain write), and any tool *result* ≥ `INLINE_BYTE_BUDGET` is buffered
server-side and never returned inline — so the array can be neither read back
into context nor re-emitted as an argument. A working escape hatch exists (the
CLI `artifact-augment --params @file`) but is not surfaced anywhere near the
point of failure. Observed in MRV-poc; the agent burned 6 tool calls and gave
up.

## Symptom (Effect)
Updating a 29-entry, ~9.6 KB findings array on tracker `35991f2994e603ea`
("MRV UAT Acceptance Findings") to sync 7 drifted `dev_status` values. Every
attempt to bring the patched array back into context to pass as `params`
buffered:

```
cat /tmp/uf_patched.json            → buffered_bytes 11108  (@tool ref, no inline content)
split into two halves, print        → buffered_bytes 11174  (both halves over cap)
read_file(@buf, start_line=3)       → "1 lines … (truncated)" + buffered_bytes 14112
```

Agent's verbatim conclusion:

```
I'm now 6 tool-calls deep trying to force a ~10 KB array through a tool arg
that can't accept one, for a mirror that has no automated reader. That's
disproportionate. Let me conclude honestly rather than keep fighting it.
```

## Reproduction
1. `git rev-parse HEAD` → `fb2809f6` (branch `experiments`).
2. Augment any artifact with `params = {arr: [...]}` where the serialized
   params exceed ~9 KB (`INLINE_BYTE_BUDGET = 9000`).
3. From an MCP client, try to change one element: you must resend the whole
   `arr` (merge replaces arrays wholesale), but you cannot read the current
   value back inline to edit it, and you cannot emit a >9 KB argument you
   never have in context. Closed loop.

## Environment
codescout MCP server (same binary as the `codescout` CLI), `experiments`
@ `fb2809f6`. Triggered from Claude Code working in
`/home/marius/work/stefanini/southpole/MRV-poc` with codescout as MCP server.

## Root cause
Three compounding mechanisms, none individually a bug:

1. **No server-side read path for `params` on the MCP tool.**
   `ArtifactAugment::input_schema` (`src/librarian/tools/augment.rs:50-92`)
   exposes `params` as an inline `object` only — no `params_path` /
   `params_ref`. To build that argument from file/buffer-resident data, the
   model must first pull the data into its own context as text.

2. **The read-back is capped.** Any tool *result* ≥ `INLINE_BYTE_BUDGET`
   (9000 B) / `TOOL_OUTPUT_BUFFER_THRESHOLD` (10000 B after envelope) is
   diverted to an `@tool_*` buffer and replaced by a compact summary
   (`src/tools/core/types.rs:18-27`). The array was 9657 B — above the budget,
   so no tool can return it inline. (The cap guards *results*, not *arguments*:
   a 9.6 KB `params` argument would be accepted — the model just can't get
   9.6 KB into context to type it.)

3. **No entry-grain write forces the whole-array resend.**
   `apply_merge_patch` (`src/librarian/catalog/augmentation.rs:140-150`) is a
   one-level object-key merge — arrays and nested objects are replaced
   wholesale. There is read-at-grain for entries (`entry_collection` +
   `entry_filter`) but no symmetric write-at-grain. So the smallest payload
   that fixes 7 of 29 entries is all 29 — exactly the size that can't
   round-trip.

(1)+(3) force a large resend; (2) makes it impossible. The escape hatch — the
CLI `artifact-augment` reading `--params @file` server-side via
`read_at_or_stdin` (`src/cli/mod.rs:81`, dispatched in
`src/cli/artifact_augment.rs:84`) — exists but is undiscoverable at the failure
point (Layer 3 / discoverability gap).

## Evidence
### Source — the trap is real
- `src/librarian/tools/augment.rs:50-92` — `params` is inline `object`, no file/ref param.
- `src/tools/core/types.rs:18-27` — `MAX_INLINE_TOKENS=2500`, `TOOL_OUTPUT_BUFFER_THRESHOLD=10000`, `INLINE_BYTE_BUDGET=9000`.
- `src/librarian/catalog/augmentation.rs:140-150` — one-level merge; arrays replaced wholesale.

### Source — the escape hatch exists
- `src/cli/artifact_augment.rs:84` — `--params` resolved via `read_at_or_stdin`.
- `src/cli/mod.rs:81-96` — `@<path>` → `fs::read_to_string`; `-` → stdin; else literal.
- Live `./target/release/codescout artifact-augment --help` confirms `--params <PARAMS>  Params JSON (\`@<file>\` / \`-\` / literal JSON)`.

### The workaround actually works (this session)
Wrapped `/tmp/uf_patched.json` as `{"findings": [...]}` and ran:
```
codescout artifact-augment 35991f2994e603ea --merge --params @/tmp/uf_full.json \
  --project /home/marius/work/stefanini/southpole/MRV-poc --json   → "ok"
```
Re-read of live params: `live == intended target: True`, 7 entries flipped to
`fixed-verified`, 29-entry set intact. One `run_command`, zero round-trip.

### Adjacent foot-gun noticed
`artifact(find/get)` does not declare a `workspace` param; passing
`workspace=<MRV-poc>` was silently dropped and the call hit the active project
(codescout). Only `scope.applied.git_root` in the response revealed the swap.
Use the CLI `--project` (cannot be silently ignored) for cross-project reads.

## Hypotheses tried
1. **Hypothesis:** `artifact_augment` has a file/buffer param I missed.
   **Test:** read `input_schema` + grep `params_path|params_ref|from_file|stdin`.
   **Verdict:** rejected — MCP tool is inline-only; only the CLI has `@file`/stdin.
2. **Hypothesis:** `merge=true` lets you patch one array element.
   **Test:** read `apply_merge_patch`. **Verdict:** rejected — one-level merge,
   arrays replaced wholesale.
3. **Hypothesis:** the CLI shares the catalog and validation.
   **Test:** read `cli/artifact_augment::run` — it dispatches the same
   `ArtifactAugment.call()` against `--project`'s catalog. **Verdict:** confirmed;
   exercised live (see Evidence).

## Fix

**A + B implemented** on the `experiments` working tree (uncommitted as of 2026-06-14;
status flips to `fixed` with the master-side SHA after cherry-pick — see CLAUDE.md
§ "After cherry-pick"). C deferred.

- **A (discoverability) — done.** Added the oversized-params / CLI hint to
  `ArtifactAugment::description()` (`src/librarian/tools/augment.rs`), the
  Augmentation Lifecycle section of `src/prompts/guides/librarian.md`, and the
  Anti-patterns section of `src/prompts/guides/progressive-disclosure.md`. Tool
  descriptions and get_guide topics are not gated prompt surfaces → no
  ONBOARDING_VERSION bump; live on next session.
- **B (params_path) — done.** Added `params_path: Option<String>` to `Args` +
  `input_schema`, resolved at the top of `call()` via `std::fs::read_to_string`
  (filesystem path only — the librarian `ToolContext` has no `output_buffer`, so
  `@buffer` refs are not resolvable from the artifact side; documented inline).
  Mutually exclusive with `params`; invalid JSON and the conflict both return
  `RecoverableError`.
- **C (entry-grain array writes) — deferred.** Belongs in `docs/plans/`. Only fix
  that eliminates whole-array resends; not needed now that params_path + the CLI
  close the round-trip gap.

Verified: 18/18 augment tests, 87/87 prompt tests, clippy clean, release build
compiles. NOT yet committed.
## Tests added

`src/librarian/tools/augment.rs` tests module:
- `params_path_reads_params_from_file` — create/replace path reads params from a file.
- `params_path_works_with_merge` — the MRV-poc scenario: merge replaces the array from a
  file-resident payload.
- `params_and_params_path_conflict_errors` — both `params` and `params_path` set → error.
- `params_path_invalid_json_errors` — malformed file content → error.
## Workarounds
**Use the codescout CLI via `run_command` — it reads params server-side, no
round-trip:**
```
codescout artifact-augment <ID> --merge --params @/path/to/params.json \
  --project <PROJECT_ROOT> --json
```
- Wrap a bare array under its params key: `{"<entry_collection>": [...]}` — a
  bare-array patch under `--merge` is a **silent no-op** (`apply_merge_patch`
  only merges when both sides are objects).
- Use `--merge` (not full replace) so the existing `prompt` / `render_template`
  / `params_schema` are preserved; a non-merge call resets omitted fields to
  None.
- `-` reads stdin if you'd rather pipe than point at a file.

## Resume

Commit ONLY the isolated files for this fix — `src/librarian/tools/augment.rs`,
`src/prompts/guides/librarian.md`, `src/prompts/guides/progressive-disclosure.md`, and
this bug file — with a targeted `git add <those paths>`, NOT `git add -A`: the working
tree is shared with a concurrent refactor session (≈20 unrelated files modified:
`onboarding.rs`, `tools/core/types.rs`, `server.rs`, `lsp/*`, `semantic/*`, …). After
cherry-pick to master, capture the master-side SHA, update this Fix section, and flip
status to `fixed`. Live-dogfood after `/mcp` restart: `artifact_augment(id=..., 
params_path="/abs/file.json", merge=true)` against a >9 KB params file.
## References
- `src/librarian/tools/augment.rs`, `src/librarian/catalog/augmentation.rs:140`
- `src/tools/core/types.rs:18-27`, `src/tools/output_buffer.rs:222`
- `src/cli/artifact_augment.rs`, `src/cli/mod.rs:81`
- `docs/prompts/guides/progressive-disclosure.md`, `get_guide("librarian")`
- Triggering session: Claude Code in `/home/marius/work/stefanini/southpole/MRV-poc` (2026-06-14)
