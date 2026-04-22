# Tool Misbehaviours — Living Log

**Reader:** developers (including Claude) working on codescout's own MCP tools.

**Purpose:** catch unexpected behaviour from codescout's tools (`edit_file`, `replace_symbol`, `find_symbol`, `semantic_search`, `edit_markdown`, `run_command`, …) *before* it gets normalised as "just how the tool works". Log it while the context is fresh; fix it or let it inform future work.

**Scope:** bugs, silent failures, misleading errors, corrupt output. Not feature requests — those go to GitHub issues.

## Before starting any task

Skim the "Mitigated quirks" section below so you know which sharp edges still exist. If you hit a new one, add an entry **before continuing**.

## Adding an entry

Use the template at the bottom. Keep it one entry per observation, even if you think it's a duplicate — historians decide that later. Mention commits and tests where possible.

## Mitigated quirks (live caveats)

These are fixed in the happy path but still have edge cases worth knowing about. Full write-ups in `docs/archive/bug-reports/2026-03-to-2026-04-tool-misbehaviors.md`.

### BUG-030 — `replace_symbol` on `mod tests` can eat an adjacent function body

- **Mitigation (2026-03-20):** `validate_symbol_position` guard detects stale LSP positions and surfaces a `RecoverableError`. Happy path works.
- **Still watch for:** stale LSP positions on large files mid-edit — if `replace_symbol` ever reports "symbol not found" after a big write, `/mcp` reconnect re-indexes.

### BUG-032 — `remove_symbol` can leave orphaned `impl` block code after enum removal

- **Mitigation (2026-03-20):** same `validate_symbol_position` guard catches the stale-position case.
- **Still watch for:** adjacent/nested `impl Trait for Type` next to inherent `impl Type` — range computation may still grab the wrong brace set (also noted on BUG-037). Workaround: `create_file` for those cases.

### BUG-021 — partial state after parallel `edit_file` calls (by design)

- **Crash fixed** by rmcp 1.2.0 cancellation-race fix.
- **Still applies:** never dispatch parallel write tool calls. Two independent writes have no transaction semantics; if one is denied by the permission dialog and the other succeeds, files end up half-applied.

## Open

*(none at time of archive — 2026-04-22)*

## Archive

Fixed / superseded entries: `docs/archive/bug-reports/`.

## Template for new entries

```markdown
### BUG-XXX — <tool>: <one-line symptom>

- **Observed:** YYYY-MM-DD
- **Tool:** `tool_name`
- **Severity:** Low / Medium / High (Low = cosmetic; High = data loss or crash)
- **What I did:** minimal repro, with actual args.
- **Expected:** …
- **What happened:** …
- **Probable cause / root cause:** (leave blank if unknown — investigation can be a separate entry)
- **Workaround:** …
- **Fix:** commit or test name, or "open"
- **Status:** Open / Fixed (YYYY-MM-DD, commit `abcdef0`) / Mitigated
```
