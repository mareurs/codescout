---
status: fixed
opened: 2026-06-14
closed: 2026-06-14
severity: high
owner: marius
related:
  - docs/issues/archive/2026-05-09-read-file-buffer-midpoint-empty.md
tags: ["read_file", "buffer", "offset", "limit", "footgun", "silent-wrong-content"]
kind: bug
---

# BUG: `read_file(@buf, offset=N, limit=M)` silently ignores both params and returns buffer start

## Summary

`read_file` accepts `offset`/`limit` as native-`Read`-style line navigation (offset = a
1-indexed start line, limit = a line count) out of caller habit — but historically neither
the buffer path nor the real-file path honored them correctly:

- **Buffer handles** (`@file_*`/`@cmd_*`/`@tool_*`): both params were silently dropped;
  the tool returned the beginning of the buffer as if no range was requested.
- **Real files**: `offset` was silently ignored (the overflow path always sliced from
  line 1); `limit` only acted as a "first N lines" cap via `OutputGuard`.

No error was ever emitted — the silent wrong-content result was indistinguishable from a
correct response, and each mis-sliced call re-ingested the same opening chunk, bloating
context. Telemetry across 4 projects (see ## Fix) showed **191** such calls, 100%
native-Read line-nav intent — 30 on buffers (all `outcome=success`) + 161 on real files.
The original report framed this as buffer-only; the real-file `offset` silent-drop is the
larger blast radius.

**Fixed (2026-06-14):** `offset`/`limit` are now normalized to `start_line`/`end_line` at
the top of `ReadFile::call`, before the buffer fork, so both paths serve them correctly.
## Symptom (Effect)

```
# Buffer @file_c4f60cf0 contains ~200+ lines (large source file).

Call A: read_file("@file_c4f60cf0", offset=456, limit=32)
  EXPECTED: lines 456–487
  ACTUAL:   lines 1–172  (the start of the buffer, paginated at inline budget)

Call B: read_file("@file_c4f60cf0", offset=225, limit=30)
  EXPECTED: lines 225–254
  ACTUAL:   lines 1–172  (byte-identical to Call A — same pagination from start)

Working form: read_file("@file_c4f60cf0", start_line=456, end_line=487)
  → returns the correct slice.
```

No error or warning is returned for the failing calls. The tool's own `hint` field in
buffer overflow envelopes advertises `start_line`/`end_line` (correct), but the
`read_full_file` overflow hint for real files says `offset`/`limit` — the mismatch
is the proximate cause of model confusion.

## Reproduction

1. Obtain a large buffered result (any `read_file` on a large source file, or any
   `run_command` / tool call that overflows the inline budget → produces `@file_*` /
   `@cmd_*` / `@tool_*` handle).
2. Call `read_file("<@handle>", offset=100, limit=50)`.
3. Observe: result is identical to `read_file("<@handle>")` with no range applied.
   `total_lines` reflects the full buffer; `content` starts at line 1.

Git HEAD at time of report: run `git rev-parse HEAD` in
`/home/marius/work/claude/codescout`.

## Environment

- Tool: `mcp__codescout__read_file` via MCP stdio transport
- Handle kinds affected: `@file_*`, `@cmd_*`, `@tool_*` (all buffer ref prefixes)
- Observed: 2026-06-14 in an active backend-kotlin session

## Root cause

> **Correction (2026-06-14, post-fix):** Defect 1's framing below — that `offset`/`limit`
> are "never parsed" — is imprecise. They are absent from `read_file`'s `input_schema`, and
> `read_from_buffer` ignores them, but `OutputGuard::from_input` (`src/tools/output.rs:96-97`)
> DOES parse both; `read_full_file` (the real-file path) reads them via the guard. So `offset`
> was silently ignored on BOTH paths, not just buffers (E-1/E-2's grep was scoped to
> `read_file.rs` + `output_buffer.rs` and missed `output.rs`). See
> `docs/trackers/bug-fix-session-log.md` F-22 and `docs/trackers/reconnaissance-patterns.md` R-31.

Two compounding defects, both in `src/tools/read_file.rs`:

### Defect 1 — `offset`/`limit` are not schema parameters and are never parsed

`input_schema` (L29–44) defines only `path`, `file_path`, `start_line`, `end_line`,
`json_path`, `toml_key`, and `force`. `offset` and `limit` are absent from the schema.

`call` (L77) routes all `@*` handles unconditionally to `read_from_buffer` before any
param parsing occurs:

```rust
// src/tools/read_file.rs:77-80
if path.starts_with("@file_") || path.starts_with("@cmd_") || path.starts_with("@tool_") {
    return read_from_buffer(path, &input, ctx);
}
```

`read_from_buffer` (L165–308) reads `start_line`/`end_line` via `optional_u64_param`
at L226–227. It never reads `offset` or `limit`. When neither `start_line` nor
`end_line` is present in the input, execution falls through to the full-buffer
pagination block (L279–308), which paginates from line 1 at the inline budget.

Result: `offset` and `limit` are fully ignored, no error is returned, and the model
receives the buffer start — silently, regardless of what was requested.

### Defect 2 — hint text in `read_full_file` advertises `offset`/`limit` for real-file overflows

`read_full_file` (L555–671) emits these strings when a real file overflows the
exploring-mode line cap (L621–629):

```rust
"Or use offset/limit to read a line range."
"File has {} lines. Use offset/limit to read specific ranges."
```

These hint strings point the model at parameters that do not exist. A model that
reads this hint during a real-file overflow and then switches to a buffer handle in
the same session will call `read_file(@buf, offset=N, limit=M)` — hitting Defect 1.

The hint text in buffer overflow envelopes is correct (it shows `start_line`/`end_line`
at L252–261 and L287–296 in `read_from_buffer`). The problem is the real-file path
emitting inconsistent, incorrect hint text.

## Evidence

### E-1: Schema audit
`grep -n '"offset"\|"limit"\|optional_u64_param' src/tools/read_file.rs` returns:
- L7: import of `optional_u64_param`
- L85–86: `optional_u64_param` for `start_line`/`end_line` (real-file path)
- L226–227: `optional_u64_param` for `start_line`/`end_line` (buffer path)
- L624, L629: literal strings containing `"offset/limit"` in hint messages only

No parse call for `"offset"` or `"limit"` exists anywhere in the file.
`src/tools/output_buffer.rs` also contains zero references to `offset` or `limit`.

### E-2: Code path trace for buffer reads
Buffer fork at `src/tools/read_file.rs:77` is an unconditional early return;
`read_from_buffer` (L165–308) only extracts `start_line`/`end_line` (L226–227);
no `offset`/`limit` parse exists anywhere in the buffer code path.

### E-3: Live session repro (2026-06-14)
`read_file("@file_c4f60cf0", offset=456, limit=32)` → lines 1–172.
`read_file("@file_c4f60cf0", offset=225, limit=30)` → lines 1–172 (byte-identical).
`read_file("@file_c4f60cf0", start_line=456, end_line=487)` → correct slice.

## Hypotheses tried

1. **Hypothesis:** `offset`/`limit` might be aliased to `start_line`/`end_line`
   upstream in the MCP dispatch layer.
   **Test:** `grep -rn '"offset"\|"limit"' src/tools/read_file.rs src/tools/output_buffer.rs`
   **Verdict:** Rejected. No aliasing exists in either file.

2. **Hypothesis:** The params might be consumed as byte offsets rather than line numbers.
   **Test:** Full read of `read_from_buffer` (L165–308) — only `start_line`/`end_line`
   consumed via `optional_u64_param`.
   **Verdict:** Rejected. The params are fully ignored; no mapping of any kind occurs.

## Fix

**Implemented 2026-06-14 (on `experiments`; master SHA TBD after ship).**

Chosen direction (data-driven — see telemetry below): make `offset`/`limit` *work* as
native-`Read` line aliases rather than reject them. Normalization happens once, at the
dispatch layer, so it covers BOTH the buffer and real-file paths and never collides with
`OutputGuard`'s generic offset/limit pagination semantics.

### What changed (`src/tools/read_file.rs`)

1. **`normalize_line_nav_aliases(&mut input)`** — new helper, called at the very top of
   `ReadFile::call` *before* the `@*` buffer fork. Gated on `start_line`/`end_line` both
   absent (explicit line params always win). Maps `offset` → `start_line` (1-indexed) and
   `limit` → a line count so `end_line = offset + limit - 1`. With only `limit`, `offset`
   defaults to line 1 (preserves the prior "first N lines" cap behavior).
2. **`input_schema`** — `offset`/`limit` added as documented native-Read aliases.
3. **Hint strings** in `read_full_file` — switched from `offset/limit` to
   `start_line`/`end_line` (the canonical advertised form).
4. **Tests** — see ## Tests added.

Because normalization injects `start_line`/`end_line`, the real-file path routes through
`read_with_line_range` and never reaches `read_full_file`'s `OutputGuard` for these calls
— structurally avoiding the same-param-two-meanings collision.

### Why "make it work" (option a) over a reject-on-buffer error (option b)

Telemetry from `.codescout/usage.db` across backend-kotlin, eduplanner-ui, MRV-poc, and
codescout (2026-05-15 → 2026-06-14):

| metric | count |
|---|---|
| `read_file` calls with `offset`/`limit` | 191 |
| …on buffers (silently broken) | 30 (100% `outcome=success`) |
| …on real files (offset dropped, limit caps from line 1) | 161 |
| sample intent | `offset:1000 limit:62`, `offset:366 limit:30` — native-Read line nav |

100% of usage was native-Read line-nav intent; zero used `offset`/`limit` as result
pagination. Rejecting them (option b) would force every caller to hand-translate to
`start_line` and leave the 161 real-file cases unaddressed; making them work serves all 191.
Note: `pika_observations` had ZERO entries for this despite 30 occurrences — it keys on
errors, and this is a silent `success` (follow-up to harden pika tracked in the session log).
## Tests added

Added to `src/tools/read_file.rs` tests module (all passing; full lib suite 2733 green,
clippy clean):

- `normalize_line_nav_aliases_maps_offset_and_limit` — offset=100, limit=50 → start_line=100, end_line=149.
- `normalize_line_nav_aliases_limit_only_defaults_offset_to_one` — limit=30 → start_line=1, end_line=30.
- `normalize_line_nav_aliases_offset_only_leaves_end_line_unset` — offset=42 → start_line=42, end_line unset (downstream 50-line window applies).
- `normalize_line_nav_aliases_explicit_start_line_wins` — start_line present → offset/limit ignored.
- `normalize_line_nav_aliases_noop_without_aliases` — neither present → no-op.
- `read_file_buffer_offset_limit_returns_slice_not_head` — @buffer offset=100 limit=50 → lines 100..=149, not the head.
- `read_file_buffer_offset_string_typed_maps_to_range` — string-typed offset="200"/limit="10" coerced → lines 200..=209.
## Workarounds

**No longer needed as of the 2026-06-14 fix** — `offset`/`limit` now work as
`start_line`/`end_line` aliases on both paths.

Pre-fix workaround (historical): use `start_line`/`end_line` exclusively when slicing any
`@*` buffer handle — `read_file("@file_...", start_line=456, end_line=487)` was always
correct, whereas `read_file("@file_...", offset=456, limit=32)` silently returned the start.
## Resume

**Done (2026-06-14).** Fix implemented + verified on `experiments`:

- Dispatch-layer `offset`/`limit` → `start_line`/`end_line` normalization (both paths). ✓
- Hint strings corrected to `start_line`/`end_line`. ✓
- `read_markdown` audited — no `offset`/`limit` hint strings present (Resume item 3 was clean). ✓
- Tests added; `cargo fmt`/`clippy -D warnings`/`test` green (2733 lib tests). ✓
- Prompt surfaces reviewed — no changes needed (`offset`/`limit` are schema-discoverable per
  `src/prompts/README.md` rule 5); no `ONBOARDING_VERSION` bump. ✓

**Pending:** commit + cherry-pick to master (per Standard Ship Sequence), then archive this
file and add the master SHA to ## Fix. Tracking: `docs/trackers/bug-fix-session-log.md` F-22.
## References

- `src/tools/read_file.rs` — key lines: L29–44 (schema), L77–80 (buffer fork),
  L165–308 (`read_from_buffer`), L555–671 (`read_full_file`), L621–629 (misleading hints)
- `src/tools/output_buffer.rs` — buffer store; contains no `offset`/`limit` handling
- Related (different symptom, same tool): `docs/issues/archive/2026-05-09-read-file-buffer-midpoint-empty.md`
  — covered `start_line`/`end_line` returning empty past midpoint (closed wontfix 2026-05-17);
  the pinned regression test there confirms `start_line`/`end_line` work correctly on buffers,
  consistent with this report.
