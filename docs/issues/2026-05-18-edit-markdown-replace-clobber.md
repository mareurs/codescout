---
status: fixed
opened: 2026-05-18
closed: 2026-05-18
severity: medium
owner: marius
related: []
tags: [edit_markdown, codescout-tool, footgun, action-replace]
kind: bug
---

# BUG: `edit_markdown action="replace"` with a heading clobbers the whole section body

## Summary

`edit_markdown(action="replace", heading=X, content=Y)` replaces the **entire body** of section X (from the line after the heading until the next sibling heading) with Y. Agents whose mental model from `insert_after`-shaped APIs assume "replace text near X" lose the original body wholesale on the call. No warning, no diff preview, return is plain `"ok"`. Caught here by post-edit verify; class is recurring (sibling bug `2026-05-09-edit-markdown-insert-after-h1.md` covers a different footgun on the same tool).

## Symptom (Effect)

The call:

```
mcp__codescout__edit_markdown(
  path="docs/observations.md",
  action="replace",
  heading="## The Plugin Closes the Loop",
  content="<new section content + trailing closer line>"
)
```

returned `"ok"`. Post-call `read_markdown` showed the body of `## The Plugin Closes the Loop` was now:

```
## The Plugin Closes the Loop

*Add new observations below as they emerge during development.*

---
```

The original ~30-line body (SessionStart hook narrative, SubagentStart hook, PreToolUse hook, marketplace install hint) was destroyed and replaced with content that was supposed to ADD a new section *after* it. No error, no warning — `"ok"`.

## Reproduction

Branch: `experiments`. Commit: `a70816b5`.

```
mcp__codescout__edit_markdown(
  path="docs/observations.md",
  action="replace",
  heading="## The Plugin Closes the Loop",
  content="<any content>"
)
```

The tool will set the entire body of `## The Plugin Closes the Loop` (from the line after the heading until the next sibling heading) to `<any content>`. Use `read_markdown` after to confirm the section's prior body is gone.

To insert AFTER an existing section instead, use `action="insert_after"` with `at="end-of-section"` (default) or `at="after-heading-line"`. That is the intended primitive for adjacent additions.

## Environment

- OS: Linux 7.0.0-15-generic
- codescout: live MCP, version current as of `a70816b5`
- Tool: `mcp__codescout__edit_markdown`

## Root cause

The semantic is **by design**, not a bug in the tool's implementation: `action="replace"` with a `heading` argument is documented to replace the section body wholesale. The defect is the **mental-model mismatch surface** — the tool description and parameter layout do not foreground the destructive scope, so an agent expecting localized text replacement (the `action="edit"` shape) or adjacent insertion (the `action="insert_after"` shape) reaches for `action="replace"` and gets wholesale body replacement.

See `src/tools/markdown/edit_markdown.rs` (the `EditMarkdown::description()` text and the `apply_replace` code path) — the text is honest about the semantic but the danger is not foregrounded, and the absence of any safety check on the new/old size ratio means a one-line `content` can erase a 200-line section silently.

## Evidence

### Pre-clobber section content (read mid-session before the bad edit)

~30 lines of SessionStart / SubagentStart / PreToolUse hook narrative plus a `> Reference: claude-plugins marketplace install` blockquote. Full content was recoverable from the in-session `read_markdown` cache; otherwise would have required git history, which was not available because `docs/observations.md` was untracked at the time (see commit `a70816b5` for the un-gitignore + creation).

### Post-clobber section content

```
## The Plugin Closes the Loop

*Add new observations below as they emerge during development.*

---
```

The only retained text is the trailing closer line that was supposed to land at the end of the *new* section, not as the body of the *existing* section.

### Recovery

Restored via a second `edit_markdown action="replace"` with the original content reconstructed from the earlier in-session `read_markdown` snapshot. Committed atomically with the new section addition in `a70816b5`.

## Hypotheses tried

N/A — root cause is by-design tool semantic, not unknown.

## Fix

Option A shipped (commit pending on `experiments`).

**Three surfaces touched in `src/tools/markdown/edit_markdown.rs`:**

1. **`description()` (line 363-367)** — kept short (under the 300-char cap enforced by `server::tests::tool_descriptions_stay_under_budget`). Top-level tool listing stays clean.

2. **`long_docs()` (line 369-394)** — added an *Action semantics — pick the right verb* table foregrounding the destructive scope of `replace` (OVERWRITES entire body) and listing the right verb per use case (`insert_after` for adjacent sections, `edit` for surgical mods, `remove` for whole deletion). Closes with a "Common footgun" callout naming the exact mental-model error caught by this bug — reaching for `replace` when meaning `insert_after`. `long_docs` has no budget cap, so the warning lives in full.

3. **Schema action descriptions (top-level at line 392-395, batch-mode inner at line 431-432)** — per-variant docs in the enum's `description` field now spell out what each action does. Agents picking actions from the MCP schema see the destructive-scope warning at the point of choice, not just in long_docs. The `content` field description also got a "REPLACES the entire existing section body" note for `replace`.

**Why not Option B (force flag + size threshold):** Per the bug file's own recommendation. Option A is cheap, observation-only — no schema parameter inflation, no magic threshold. If the footgun recurs across sessions with Option A live, escalate to Option B. Two-concretes discipline: ship the lighter intervention first, gather evidence.

**Gates passed:**
- `cargo fmt --check` ✅
- `cargo clippy --all-targets -- -D warnings` ✅
- `cargo test --lib --bins` → 2383 passed, 0 failed, 7 ignored (was 2382 + 1 failed when description exceeded the 300-char budget; reverted to short description and pushed warning to long_docs)
- `server::tests::prompt_surfaces_reference_only_real_tools` ✅
- `server::tests::tool_descriptions_stay_under_budget` ✅ (re-tightened the budget caught the first attempt — sibling guard worked as designed)
- BUG-043 regression tests still pass (`replace` subsection-consumption refusal unchanged).

**No `ONBOARDING_VERSION` bump.** The change is to live tool description/schema (refreshed every MCP connect via the tool registration), not to `onboarding_prompt.md` or `build_system_prompt_draft()`. Per `CLAUDE.md § Onboarding Version`, those are the only surfaces that require a version bump.

**No source-of-truth changes to `src/prompts/*.md`.** A grep across the prompt directory found 5 mentions of `edit_markdown`; two of them use `action="replace"` on `## codescout Memories`, which is the legitimate "refresh stale memory table" use case — those examples model the safe path, not the footgun. No edits needed.
## Tests added

N/A — no fix shipped yet. When Option A lands, no test needed (description-only). When Option B lands, three regression tests required:

1. `action="replace"` with `force=false` and `len(new) < 0.2 * len(old)` returns a `RecoverableError` whose hint mentions `insert_after` and `force=true`.
2. Same call with `force=true` succeeds.
3. `action="replace"` with `len(new) >= 0.2 * len(old)` succeeds without `force` — no regression on legitimate body rewrites.

## Workarounds

- **For inserting a new section after an existing one:** `action="insert_after"` with `at="end-of-section"` (default) or `at="after-heading-line"`. Never `action="replace"`.
- **For modifying part of an existing section's body:** `action="edit"` with `old_string` / `new_string` for surgical text replacement.
- **For deleting a section:** `action="remove"`.
- **Verify-after-edit on any markdown write:** read the affected heading back with `read_markdown(path, heading="...")` after every `edit_markdown` call. One extra round-trip per edit catches clobbers in-session; without it the section data is lost silently until later. This is the Frog discipline that surfaced the bug in the first place.

## Resume

If picking this up: open `src/tools/markdown/edit_markdown.rs` and locate the `EditMarkdown::description()` text plus the `action` parameter's per-variant description in the JSON schema. Confirm whether the destructive scope of `replace` is foregrounded prominently (it currently is not — the description states the semantic but does not warn about the clobber risk relative to `insert_after`). Draft the Option A description rewrite; run the `server::tests::prompt_surfaces_reference_only_real_tools` test after to confirm no surface drift, then run `cargo test edit_markdown` to confirm no regression on the action semantics themselves.

If escalating to Option B: scaffold the threshold check in `EditMarkdown::call` before the body-replacement path; thread a `force: Option<bool>` through the schema; emit `RecoverableError::with_hint` on threshold breach (`sibling_call_hint` pattern from `src/tools/mod.rs`).

## References

- Session log entry: `docs/trackers/bug-fix-session-log.md` — F-4
- Sibling tool bug: `docs/issues/2026-05-09-edit-markdown-insert-after-h1.md` (different action, same tool family, similar mental-model surface)
- Commit where the clobber landed and was recovered: `a70816b5` (`docs(observations): un-gitignore + add "When the Substrate Catches Itself"`)
- Tool source: `src/tools/markdown/edit_markdown.rs` — `EditMarkdown::description`, `apply_replace` path
- Discipline that caught it: Frog Phase 3 self-critique (`~/.claude/buddy/skills/docs-lotus-frog/SKILL.md`, `### Phase 3 — Self-Critique`)
