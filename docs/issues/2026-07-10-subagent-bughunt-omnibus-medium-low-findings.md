---
status: open
opened: 2026-07-10
closed:
severity: medium
owner: marius
related:
- docs/issues/2026-07-10-librarian-recoverable-error-downcast-never-matches.md
- docs/issues/2026-07-09-residual-workspace-pin-gaps-post-edit-code-fix.md
tags: [omnibus, windows, error-handling, edit_code, tree, list_overview, onboarding]
kind: bug
---

# BUG: omnibus — medium/low findings from the 2026-07-10 three-arm subagent bug-hunt

## Summary
Verified medium/low findings from the 3-arm × 3-direction subagent bug-hunt (session 5efbda5f), collected in one file per the omnibus precedent (`2026-07-10-silent-cap-missing-overflow-signals-audit.md`). Each entry cites the reporting agent(s); items marked **verified** were re-read at the bytes by the main agent, others are **traced** (agent-cited lines, not independently re-read).

## Findings

### F7 — tree.rs non-glob listing emits unnormalized native paths (Windows) — MEDIUM, verified
`src/tools/tree.rs:138` (`list_dir_impl`): `entries.push(format!("{}{}", entry.path().display(), suffix))` — no `to_forward_slash`, while `glob_impl` (:228) normalizes. On Windows: backslash absolute paths defeat `post_process`'s forward-slash literal-substring root-stripping → absolute paths leak; `common_path_prefix`/tree-mode indentation (both `/`-based) silently degrade. Reported independently by A2, B2, C2 (identical file:line). Fix: `to_forward_slash(entry.path())` at :138.

### F8 — list_overview single-file branch echoes raw caller path — MEDIUM, verified
`src/tools/symbol/list_overview.rs` single-file branch emits `"file": rel_path` where `rel_path` is the raw input string (:195), while sibling branches normalize (`relative_forward_slash`/`to_forward_slash`). Absolute/`./`-prefixed/backslash inputs echo verbatim. Under a `workspace=` pin + absolute path, composes with the KNOWN post_process unpinned-root gap (`5695424c48c90964` §2) into a cross-workspace absolute-path leak (B2's framing). Reported by A2 + B2. Fix: normalize against the resolved (pinned) root like the sibling branches.

### F9 — edit_code rename hand-rolls relative-path idiom — LOW-MED (Windows), traced
`src/tools/symbol/edit_code.rs` `do_rename` (~:342-360) and `src/symbol/edit.rs::text_sweep` (~:448): `path.strip_prefix(&root).unwrap_or(path).display().to_string()` into structured `corruption_hints[].file` / `TextualMatch.file` instead of `relative_forward_slash`. Backslashes on Windows, inconsistent with every other tool. Reported by A2 + C2. `src/tools/symbol/references.rs:318` same idiom, prose-hint only (lower). Fix: adopt the shared helper.

### F10 — edit_code post-edit corruption check disabled exactly when extraction fails — MEDIUM (narrow trigger), verified
`src/tools/symbol/edit_code.rs:706-711`: `post_ast = extract_symbols(&full_path).ok()`; on `Err`, `post_count` falls back to `pre_count` → "no change", bypassing the dropped-symbol rollback; the sibling-drop check is skipped when `post_ast` is `None`. tree-sitter rarely errors on malformed source (best-effort trees), so real-world trigger is unsupported-language/IO-race — but the safety net is off precisely in the anomalous cases. Reported by B3. Fix: on post-extraction failure, surface a warning field in the response ("corruption check skipped: <err>").

### F11 — librarian tools mix bail!/RecoverableError for equivalent input errors — MEDIUM, traced
Sites (C3, each confirmed in-function): `create.rs:13-19,93` (bad rel_path, path collision → bail; unknown repo → Recoverable in same fn); `get.rs:84,96,102` (removed param, exclusive selectors, start>end → bail; invalid links_direction → Recoverable); `graph.rs:23` (depth range), `link.rs:20-22` (src/dst not found — the guide's canonical recoverable example); `scope.rs` (umbrella-not-found/no-members bail deep under find/context while find.rs:371-381 special-cases only scope-all). Currently subsumed by the downcast bug (all librarian errors hard-fail anyway) — fix AFTER routing, else invisible. Also frontmatter: `src/tools/markdown/frontmatter.rs:89,158` propagate bare through `edit_markdown`'s otherwise-Recoverable validation (C3).

### F14 — onboarding swallows sub-project memory-write failures — LOW, traced
`src/tools/onboarding.rs:~798-800`: `let _ = store.write("onboarding", …)` / `("language-patterns", …)` per sub-project (root project uses `?`). Workspace onboarding reports success with missing sub-project memories on write failure. Reported by A3. Fix: collect failures into the response summary.

### F19 — buffered read_file envelopes carry wrong json_path hint — LOW-MED, traced
`Tool::json_path_hint` default `"$.field"` (`src/tools/core/types.rs:521-523`), no `ReadFile` override; buffered `json_path`/`toml_key` extraction envelopes hint `read_file("<ref>", json_path="$.field")` but the wrapper's real fields are `content`/`path`/`value_type`/… — following the hint errors. Reported by A1. Fix: override `json_path_hint` for ReadFile (`"$.content"`).

### F20 — .lock files rejected for toml_key — LOW, traced
`detect_file_type` (`file_summary.rs:27-50`) classifies `.lock` as `Config`, so `read_toml_yaml_key` (:445-474) rejects `Cargo.lock` ("only supported for TOML and YAML") despite valid TOML. Reported by A1. Fix: map `.lock` → Toml (or sniff).

### F18 — grep(mode="files") gross miscount — UNVERIFIED, needs repro
C3 observed `grep(mode="files")` report "73 bail! in src/tools" where a direct pattern match found 4. Not reproduced by the main agent. If real, mode="files" per-file counts are wildly wrong on some inputs. Next: reproduce with `grep(pattern="bail!", path="src/tools", mode="files")` vs `mode="lines"` count and diff the counting logic (`src/tools/grep.rs`).

### Gray-zone (logged, judgment call, no action queued)
- `check_tool_access` writes/indexing-disabled hard error with correctable-sounding message (B3 #3) — see path-security bug's Fix note.
- `abs_path` response fields carry stripped (relative) values post-`post_process` while the name promises absolute (B2 #3) — naming/doc tension only.
- `audit_doc_refs/mod.rs:~369,512` `to_string_lossy` rel_path derivation could write backslash rel_paths into the catalog on Windows (A2 #5, suspect, untraced downstream).
- A2's "glob leaks absolute paths" (tree.rs:228) scored FALSE-POSITIVE for the active project — `post_process` root-stripping relativizes them; only the pinned-workspace case leaks, which is the KNOWN `5695424c48c90964` §2.

## Reproduction / Environment / Hypotheses
Per-finding above; branch `experiments`, 2026-07-10.

## Fix
Per-finding above. Suggested order: F11 after the downcast fix; F7/F8/F9 as one Windows-path sweep (same class as the fixed `260752eb609a8903`); F19/F20 quick wins; F18 repro first.

## Tests added
N/A — nothing fixed yet.

## Workarounds
N/A per finding.

## Resume
Reproduce F18; then the Windows-path sweep (F7/F8/F9) mirroring commit `edb44a9b`'s helper adoption; F19/F20 alongside the buffer-nav bug fix.

## References
- Experiment: session 5efbda5f, 9 subagents (3 arms × 3 directions), reports in session transcripts.
- Siblings filed same day: librarian-downcast, path-security-doc, buffer-nav-params, extract-toml-key branch order.
