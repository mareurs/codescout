# Audit Doc Refs

`librarian(action="audit_doc_refs", …)` scans markdown files for stale code
references and emits findings as an `audit_issues` tracker at
`docs/trackers/doc-ref-audit.md` (auto-created on first run).

## When to use

Manual cadence in v1 — run when a doc-heavy PR is about to merge or when you
suspect drift. No CI integration; the `fail_on` flag is present for downstream
repos that wire their own gates.

## What it scans

Only inside code-spans (`` `…` ``), fenced blocks (` ``` `), and link targets
(`[label](target)`). Plain prose is never parsed.

## Reference kinds

| ref_kind | Pattern | Example |
|---|---|---|
| `file_path` | extension-bearing path, no `:` | `` `src/mrv/chat_app.py` `` |
| `file_line` | path with `:NN` suffix | `` `scripts/eval.py:807` `` |
| `file_symbol` | path with `:Class/method` or `:fn` | `` `src/cli.py:cmd_generate` `` |
| `module_path` | dotted ident in code span only (≥1 dot, lowercase) | `` `mrv.chat_app` `` |
| `link` | URL position in `[…](…)` | `[foo](src/foo.py)` |

## Verdicts

| verdict | meaning | default severity |
|---|---|---|
| `resolved` | reference matches current code/filesystem | n/a |
| `missing` | file path does not exist | high |
| `symbol_missing` | LSP returned no match for symbol | high |
| `file_missing` | file_symbol's path component is gone | high |
| `line_oob` | cited line past EOF | med |
| `anchor_missing` | `#section` link target does not exist in target md | med |
| `unknown` | parser identified candidate but resolution ambiguous | low |
| `external` | http/https link — informational, dropped from tracker | n/a |

## Severity drops

| Location | Drop | Why |
|---|---|---|
| `docs/archive/**` or `*.archive.md` | one level | archive is meant to rot |
| Memory files | two levels | memory is temporally pinned by design |
| `docs/issues/**` | one level | issue trackers document historical state |

Memory globs:
- `.buddy/memory/**`
- `**/.buddy/memory/**`
- `**/buddy/memory/**`
- `**/projects/**/memory/**`

Override via `severity_overrides.memory_globs`.

## Suppression

Set an issue's `status` to `wontfix` in the tracker. The merger never
auto-flips wontfix back to open.

## v1 limitations

- **LSP not yet plumbed through the librarian's ToolContext.** As a result,
  `file_symbol` and `module_path` candidates resolve as `unknown` with
  `scan_meta.degraded: true`. This is the v1 fallback — real LSP integration
  is a Phase 2 candidate.
- **Classifier overfires on absolute Unix paths** like `/mcp`, `/proc/...`,
  `/tmp/...` when they appear inside code spans. These show up as `missing`
  findings; suppress with `wontfix` if surfacing them is more noise than signal.
- **OutputGuard cap of 50 findings inline.** The full set always lives in the
  tracker; the inline response is capped to keep the LLM-visible payload sane.

## Example

```jsonc
librarian({
  "action": "audit_doc_refs",
  "scope": "project",
  "paths": ["docs/**/*.md", "CLAUDE.md"],
  "emit_tracker": true,
  "fail_on": "never"
})
```

Default behavior: scans `docs/**/*.md`, `CLAUDE.md`, `**/CLAUDE.md`,
`**/README.md` (`audit_doc_refs::DEFAULT_AUDIT_GLOBS`). Auto-creates
`docs/trackers/doc-ref-audit.md` if absent. `fail_on=never` keeps the action
diagnostic (exit 0); set `fail_on=high` to exit nonzero on any high-severity
non-resolved finding.
