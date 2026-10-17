---
kind: bug
status: open
title: 'BUG: the injected guide block follows a tool''s last line with no newline, so line-oriented readers of a reply lose that line'
tags:
- cluster/unclassified
opened: 2026-10-17
owner: marius
related:
- docs/issues/2026-10-17-symbols-include-body-on-kotlin-lsp-results-is-shifted-up-one-line.md
severity: low
---

# BUG: the injected guide block follows a tool's last line with no newline, so line-oriented readers of a reply lose that line

## Summary

codescout appends a `get_guide` body to a tool reply as a SEPARATE MCP content block that starts with `<!-- auto-injected get_guide(...)`. Clients that join the blocks without a separator put the marker on the same line as the tool's last line. A reader that strips the guide by line (`sed '/auto-injected/,$d'`, `grep -v`, `head`) then drops that last line: one body line, one overview row. This produced two false bug reports on 2026-10-17 (`symbols(include_body)` "shifted up one line", an overview "missing" its last member).

## Class fit

No class on the roster was walked one by one, so the sanctioned `cluster/unclassified` is used. What the case shows: two parts of one reply (the tool's text and an advisory) are separate to the server and merged by the reader, and the merge destroys content at the seam without any sign that it did.

## Symptom (Effect)

Plain `mcp call symbols ... include_body=true` output, measured 2026-10-17 (`grep -o` around the marker): `6:          }<!-- auto-injected`. Claude Code's own tool results show the same join, `}<!-- auto-injected get_guide(...)`, so it is not specific to the `mcp` CLI.

## Reproduction

```
mcp call symbols -p '{"name": "Catalog/add", "path": "src/main/kotlin/library/services/Catalog.kt", "include_body": true}' <alias> | sed '/auto-injected/,$d'
```
prints the body without its closing `}`. The same call with `-f json` and `content[0].text` is complete.

## Environment

codescout at branch worktree-agent-af85b780928eaa450, `mcp` CLI at ~/.local/bin/mcp.

## Root cause

Confirmed at the MCP layer: the guide is emitted as its own content block (`src/tools/guide_emit.rs`, around lines 110-122, per the 2026-10-17 review), so the server is not joining anything. The seam is made by the consuming client. measured 2026-10-17: `repro5.sh` plain output, and the reviewer's observation of Claude Code tool results.

## Evidence

The plain-output byte dump above; `repro.sh` (line-oriented strip) reproduces the false reports, `repro4.sh` (JSON) does not.

## Hypotheses tried

None needed.

## Fix

Proposed, NOT implemented: start the guide block with a newline (`"\n<!-- auto-injected ..."`), which removes the hazard for every line-oriented consumer whatever joins the blocks. It changes emission bytes and the byte-pin test `a_non_declaring_topic_is_byte_identical_to_today` would have to move with it, which is a decision for the owner of that pin, not a side effect of a symbols fix.

## Tests added

N/A: open, proposal only.

## Workarounds

Cut the guide with parameter expansion, `${RESULT%%<!-- auto-injected*}` (what `tests/mcp-smoke-kotlin.sh` does), or read `-f json` `content[0].text`.

## Resume

1. Decide on the leading newline. 2. If yes: change the emitter in `src/tools/guide_emit.rs`, update `a_non_declaring_topic_is_byte_identical_to_today` and any other byte pin, and check the smoke scripts' `%%` expansion still cuts at the marker.

## References

- `docs/issues/2026-10-17-symbols-include-body-on-kotlin-lsp-results-is-shifted-up-one-line.md`
- `tests/mcp-smoke-kotlin.sh` (`call`)
