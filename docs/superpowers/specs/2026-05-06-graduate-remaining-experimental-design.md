# Graduate Remaining Experimental Features Design

**Date:** 2026-05-06
**Branch:** experiments
**Type:** Single-batch documentation graduation

## Context

After today's earlier graduation of `call_graph`, `auto-reindex-on-edit`, and `hybrid-bm25-vector`, nine experimental features remain in `docs/manual/src/experimental/`. All nine have fully shipped, working code in the codebase. The librarian default flipped to enabled this session, which makes the `librarian-embedded.md` content stale and triggers a content fix as part of graduation.

## Pre-conditions

- All nine features have code in `src/` or `crates/librarian-mcp/src/` (verified)
- Librarian default = `true` as of this session (commit `795dee1`)
- Single batch, single commit — same risk profile (doc-only) for all moves

## Per-file mechanics (uniform pattern)

For each file:

1. `git mv docs/manual/src/experimental/<name>.md docs/manual/src/concepts/<name>.md`
2. Remove the leading `> ⚠ Experimental — may change without notice.` line plus the blank line after it
3. Remove the feature's entry from `docs/manual/src/experimental/index.md`

`librarian-embedded.md` gets one extra step (content fix): replace any wording about "opt in via `LIBRARIAN_ENABLED=1`" with "enabled by default; opt out via `LIBRARIAN_ENABLED=0` or `[librarian] enabled = false` in `.codescout/project.toml`."

After all moves, `experimental/index.md` retains the page (entry point for future experiments) but with an empty Available Features section replaced by `*No features currently in experimental.*`.

## File placements

| # | File | Target |
|---|------|--------|
| 1 | `artifact-move.md` | `concepts/artifact-move.md` |
| 2 | `librarian-guide-resource.md` | `concepts/librarian-guide-resource.md` |
| 3 | `librarian-tools-collapse.md` | `concepts/librarian-tools-collapse.md` |
| 4 | `librarian-embedded.md` | `concepts/librarian-embedded.md` (+ content fix) |
| 5 | `workspace-state-at.md` | `concepts/workspace-state-at.md` |
| 6 | `augmentation-render-template.md` | `concepts/augmentation-render-template.md` |
| 7 | `tracker-design.md` | `concepts/tracker-design.md` |
| 8 | `artifact-refresh-stale.md` | `concepts/artifact-refresh-stale.md` |
| 9 | `heartbeat-memory-fields.md` | `concepts/heartbeat-memory-fields.md` |

## SUMMARY.md updates

8 librarian features nest under the existing `[librarian-mcp](concepts/librarian-mcp.md)` entry:

```
- [librarian-mcp](concepts/librarian-mcp.md)
  - [Librarian Embedded](concepts/librarian-embedded.md)
  - [Librarian Tools Collapse (16 → 5)](concepts/librarian-tools-collapse.md)
  - [doc://librarian-guide Resource](concepts/librarian-guide-resource.md)
  - [artifact_refresh (list_stale)](concepts/artifact-refresh-stale.md)
  - [artifact (action="move")](concepts/artifact-move.md)
  - [tracker_design](concepts/tracker-design.md)
  - [workspace_state_at](concepts/workspace-state-at.md)
  - [Augmentation: Templates & Schemas](concepts/augmentation-render-template.md)
```

The heartbeat feature goes under Development → Debug Mode:

```
- [Debug Mode](concepts/diagnostic-logging.md)
  - [Heartbeat Memory Fields](concepts/heartbeat-memory-fields.md)
- [Troubleshooting](troubleshooting.md)
```

## Verification

```bash
# No ⚠ callouts remain in graduated files
grep -l "Experimental" docs/manual/src/concepts/{artifact-move,librarian-guide-resource,librarian-tools-collapse,librarian-embedded,workspace-state-at,augmentation-render-template,tracker-design,artifact-refresh-stale,heartbeat-memory-fields}.md
# Expected: no output

# All 9 appear in SUMMARY.md
grep -c "concepts/\(artifact-move\|librarian-guide-resource\|librarian-tools-collapse\|librarian-embedded\|workspace-state-at\|augmentation-render-template\|tracker-design\|artifact-refresh-stale\|heartbeat-memory-fields\)" docs/manual/src/SUMMARY.md
# Expected: 9

# experimental/index.md no longer lists any of them
grep "artifact-move\|librarian-guide-resource\|librarian-tools-collapse\|librarian-embedded\|workspace-state-at\|augmentation-render-template\|tracker-design\|artifact-refresh-stale\|heartbeat-memory-fields" docs/manual/src/experimental/index.md
# Expected: no output

# experimental/index.md retains a placeholder
grep "No features currently" docs/manual/src/experimental/index.md
# Expected: matching line
```

## Commit

```
docs: graduate 9 remaining experimental features
```

## Non-goals

- No content rewrites beyond the librarian-embedded opt-in/opt-out fix
- No file renames (all keep their existing slugs)
- No SUMMARY.md restructuring beyond adding the 9 new entries
- No CLAUDE.md changes
