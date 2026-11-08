# Error Handling

Two error paths in codescout. The model's behavior differs.

## `RecoverableError` → `isError: false`

Input-driven, expected failure. The MCP server serialises this as
`isError: false` with a JSON body containing `"error"`, optional
severity-tagged guidance (one of `hint` / `warning` / `must_follow`),
and any structured `extra` fields spliced at the top level.

**Sibling tool calls in the same turn survive** — Claude Code aborts
parallel siblings only when it sees `isError: true`.

Examples of recoverable conditions:

- Topic not found in `get_guide(topic)`.
- Bad line range, or a markdown-only parameter passed for a non-markdown file.
- Path not found, unsupported file type, empty glob match.
- No index built yet for the queried project.
- User declined an elicitation prompt.

The response body carries a guidance field describing how to recover.
Read it, adjust the call, retry — do not bail the whole sequence.

## `anyhow::bail!` → `isError: true`

Genuine tool failure: LSP server crashed, filesystem error, panic in
the tool, security violation. Fatal — the rest of the turn's parallel
tool calls are at risk.

The model should not blind-retry without changing inputs. The user may
need to intervene.

## How to tell them apart

The MCP response has an `isError` field.

- `isError: true` → fatal; stop and surface to the user. One exception: a
  read-only refusal (the project was activated read-only) is `isError: true`,
  but its text names `ack="@ack_…"`: replay the refused call with it to make that one write.
- `isError: false` → recoverable; read the message + guidance field
  (`hint` / `warning` / `must_follow`) and adapt the next call. The body starts
  `"ok": false`, so read that field: the harness may not show you `isError`.

For tool authors: return `RecoverableError::new(message)` — or the
`RecoverableError::with_hint(message, hint)` / `with_warning` /
`with_must_follow` constructors to attach severity-tagged guidance, then
chain `.with_extra(key, value)` for structured fields — when the failure
is the model's fault, bad input it can self-correct. Return
`anyhow::bail!` when it isn't.
