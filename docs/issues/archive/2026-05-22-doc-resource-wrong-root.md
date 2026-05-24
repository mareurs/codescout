---
status: fixed
opened: 2026-05-22
closed: 2026-05-22
severity: medium
owner: marius
related: []
tags: [mcp, resources, doc-provider, server]
kind: bug
---

# BUG: doc:// MCP resources resolve relative to active project root, fail when active project ≠ codescout

## Summary
`build_resource_registry` joined every `doc://` URI's filesystem path to `agent.project_root()`. Since the source files ship inside the codescout repo (`src/prompts/...`, `docs/...`), any session whose active project was not code-explorer received a registration pointing at `<foreign_root>/src/prompts/...`. Reads then failed with MCP `-32603: source unavailable ... No such file or directory (os error 2)`.

## Symptom (Effect)

User invoked `readMcpResource("doc://librarian-guide")` while working in a foreign project (e.g. `~/work/mirela`). Got:

```
MCP error -32603: source unavailable for doc://librarian-guide: No such file or directory (os error 2)
```

`doc://progressive-disclosure` had the same failure mode. `doc://tool-misbehaviors` was a worse variant: the underlying file `docs/TODO-tool-misbehaviors.md` had been deleted from the repo, so the URI was broken even when the active project *was* code-explorer.

## Reproduction

```bash
# At commit f55f449d, with codescout MCP running.
# Activate any non-codescout project.
workspace(action="activate", path="/home/marius/work/mirela")
# Then request the doc resource.
readMcpResource("doc://librarian-guide")
# → -32603 source unavailable ... No such file or directory
```

## Environment
- Branch: `experiments`
- Commit at failure: `f55f449d5b9be186ea37e4dc995fe9f4073e54a8`
- Transport: any (path resolution is host-independent)

## Root cause

`src/server.rs::build_resource_registry` (pre-fix lines 798-825) constructed `DocSource { path: project_root.join("src/prompts/librarian-guide.md"), ... }`. `DocProvider::read` (`src/mcp_resources/doc.rs:36-46` pre-fix) ran `tokio::fs::read_to_string(&src.path)` at request time. With a non-codescout project active, `project_root` pointed at the user's workspace, where neither `src/prompts/librarian-guide.md` nor `docs/PROGRESSIVE_DISCOVERABILITY.md` exist. The `if let Some(project_root)` gate masked the design error: registration succeeded, only the read failed, and the error string suggested an environmental problem rather than a wrong-root bug.

Secondary mechanism: `src/prompts/librarian-guide.md` (228 lines) had drifted from `src/prompts/guides/librarian.md` (290 lines, used by `get_guide`). Even on the happy path, the resource served stale content.

## Evidence

- `grep "doc://librarian"` → confirmed 9 references; only `src/server.rs:815` and `src/util/librarian_guard.rs:24` registered it.
- `ls /home/marius/work/mirela/src/prompts/librarian-guide.md` → `No such file or directory` (exact symptom string).
- `diff -q src/prompts/librarian-guide.md src/prompts/guides/librarian.md` → "Files differ"; line counts 228 vs 290.

## Hypotheses tried
1. **Hypothesis**: resource not registered. **Test**: grep server.rs. **Verdict**: rejected — registration block found at line 815.
2. **Hypothesis**: `DocProvider::read` failed an IO call due to permissions. **Test**: re-read `read` impl. **Verdict**: rejected — `ENOENT` only fires when the path doesn't exist.
3. **Hypothesis**: `project_root` at registration time differs from the codescout source tree. **Test**: replay path under a foreign root. **Verdict**: confirmed — `ls /home/marius/work/mirela/src/prompts/librarian-guide.md` reproduces the verbatim error.

## Fix

Switch `DocProvider` from runtime path reads to compile-time `include_str!` embeds.

- `src/mcp_resources/doc.rs`: replace `DocSource::path: PathBuf` with `DocSource::content: &'static str`. `read` becomes infallible (returns embedded text; `NotFound` still possible for unregistered URIs).
- `src/server.rs`: extract `static_doc_sources()` returning the registered list; bodies embedded via `include_str!("../docs/PROGRESSIVE_DISCOVERABILITY.md")` and `include_str!("prompts/guides/librarian.md")`. Drop the `if let Some(project_root)` gate. Drop `doc://tool-misbehaviors` (underlying file deleted).
- `src/prompts/librarian-guide.md`: deleted; `doc://librarian-guide` now serves `src/prompts/guides/librarian.md` (single canonical copy, also used by `get_guide`).
- `docs/manual/src/concepts/librarian-guide-resource.md`: updated Source section to cite the new path + embedding strategy.

Implemented in the same working tree as this bug file (uncommitted at the time of writing).

## Tests added

- `mcp_resources::doc::tests::doc_provider_returns_embedded_content` — replaces the old `doc_provider_reads_existing_file` test; asserts the embedded path works.
- `mcp_resources::doc::tests::doc_provider_reports_unknown_uri` — replaces the old `doc_provider_reports_missing_source` test; the `SourceUnavailable` variant is no longer reachable from `DocProvider::read`, so the regression case shifts to "URI not registered".
- `server::tests::static_doc_sources_all_readable` — enumerates `static_doc_sources()`, calls `DocProvider::read(uri)` on each, asserts `Ok` + non-empty body. Catches future stale-path or stale-include regressions at `cargo test` rather than at MCP call time.

## Workarounds

Pre-fix: activate the codescout project (`workspace(action="activate", path="/home/marius/work/claude/code-explorer")`) before calling `readMcpResource("doc://librarian-guide")`. Restore prior workspace afterwards (Iron Law 4).

## Resume

N/A — fixed. After cherry-pick to `master`, migrate this file to `docs/issues/archive/`.

## References

- `src/server.rs::static_doc_sources` (post-fix)
- `src/server.rs::build_resource_registry`
- `src/mcp_resources/doc.rs`
- `src/prompts/guides/librarian.md`
- `docs/manual/src/concepts/librarian-guide-resource.md`
