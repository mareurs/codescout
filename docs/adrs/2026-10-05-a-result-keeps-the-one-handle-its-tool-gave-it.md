---
id: dfe04791a1ead0cc
kind: adr
status: active
title: ADR-2026-10-05 — A result keeps the one handle its tool gave it
owners:
- marius
tags:
- design-principle
- progressive-disclosure
- tool-contract
- run_command
topic: tool-contracts
---

# ADR-2026-10-05 — A result keeps the one handle its tool gave it

## Status

Accepted — active. Decided 2026-10-05 after the two `run_command` byte-bound bugs
(`e833abeb`, archived under `docs/issues/archive/2026-10-05-run-command-*`).

## Context

`Tool::call_content` (`src/tools/core/types.rs`) is the only non-test site that mints a
`@tool_*` handle. It buffered any result over the inline limit, whether or not the tool had
already buffered the result itself. `run_command` stores its raw stream behind `@cmd_*` and
returns a summary envelope that carries that handle as `output_id`. When the envelope was
itself over the limit, the caller held two handles for one result, and the second was a copy
of a summary it had never been shown. Measured before the fix: `@tool_0b92312d` with the
tool's own `@cmd_0b92312a` named only inside the summary line.

A usage count from this repo's `usage.db` (2026-09-21 to 2026-10-03): 95 organic `run_command`
calls emitted `@tool_*`, 69 of them naming a `@cmd_` handle.

## Decision

A result that already carries a string `output_id` keeps exactly that handle.
`clip_prebuffered_envelope` clips the envelope's oversized top-level string fields, largest
first and only as much as needed, until it fits `INLINE_BYTE_BUDGET`. Each clipped field gets
a marker (`<shown> of <total> bytes shown; cut to fit the response budget; the whole stream is <handle>` for a `@cmd_` handle's stdout, `the whole stream is <handle>.err` for its stderr, and `this field is built by the tool and stored nowhere, <handle> holds the output it was built from` for every other field and handle kind). The fit is measured on the serialized envelope.

An envelope that cannot fit with every field at `PREBUFFERED_FIELD_FLOOR` (1,000 B) is
returned unchanged, and `call_content` buffers the ORIGINAL under `@tool_*` as before. A
result without its own `output_id` is never clipped. `force_inline` tools are never clipped.

The elision helper moved to `util::text::elide_middle_bytes`, so `core` does not depend on a
`run_command`-specific module that already depends on `core`.

## Alternatives considered

- **Fix the hint only** (original option (a)). Rejected: two handles remain, and the agent
  still picks the wrong one.
- **Keep `@tool_*` and point its hint at the `@cmd_*` handle.** Rejected: it keeps the second
  handle and spends a buffer slot on a copy.
- **Remove `@tool_*` for every tool.** Rejected: `symbols` and `doc` rely on `json_path`
  into `@tool_*`.
- **Hard error when a pre-buffered envelope is oversized.** Rejected: loud, but it gives the
  agent nothing.

## Consequences

- Easier: one handle per result for every tool that returns an `output_id`.
- Harder: the size guarantee for such a tool is now partly the tool's job. The backstop only
  clips top-level string fields, so an envelope whose bulk is nested falls back to `@tool_*`.
- **Known gap.** `read_file`, `read_markdown` and `memory` return their handle as `file_id`,
  so this rule does not fire for them. `read_file` of a whole source file returned
  `@tool_*` and `@file_*` together when this ADR was written. The sibling sweep
  (`docs/issues/archive/2026-10-06-sibling-sweep-the-byte-bound-defect-recurred-across-tools.md`)
  bounds its summary, so a read returns one handle, and a byte-edge sweep of the merged tip found no
  response with two. These tools still do not pass through `clip_prebuffered_envelope`: each bounds
  its own summary.

## Change scenarios absorbed

- A `run_command` diagnostic field grows large (`wip_authors`, the compacted test response).
- A future tool pre-buffers and forgets to bound its summary.

## Revisit when

A tool needs a recoverable envelope that is not a copy of its own buffer, or a second tool
family adopts `output_id`.

## Evidence

23 mutations of the helper, the three `run_command` summary sites and the clip, run one at a
time on the final bytes in a private worktree, all killed. Two gaps the runs found are fixed:
a character-boundary fixture that never cut mid-character, and a missing `force_inline` test.
`./scripts/gate.sh` `FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0`. Confidence: high on the rule, medium
on the 1,000-byte floor (a judgment, not a measurement).
