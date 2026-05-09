# Bug Tracker Template Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stand up a reusable per-bug tracker template at `docs/issues/_TEMPLATE.md`, paired with an at-a-glance `INDEX.md`, a `CLAUDE.md` trigger rule, and a deprecation banner on `docs/TODO-tool-misbehaviors.md`.

**Architecture:** Four documentation-only file edits, no Rust changes, no automated tests. Each edit produces one focused artifact with a single responsibility. The template is self-contained (its top comment carries trigger rules + status semantics + INDEX-pairing reminder so any future reader has full context without needing CLAUDE.md). All edits land in one commit on `experiments`.

**Tech Stack:** Markdown + YAML frontmatter only. No code.

**Spec:** [`docs/superpowers/specs/2026-05-09-bug-tracker-template-design.md`](../specs/2026-05-09-bug-tracker-template-design.md) (committed as `b308c70`).

**Important note on TDD:** The writing-plans skill's default TDD template assumes code with tests. This work is documentation-only. Per the spec's `## Validation` section: "No automated test. The template's value is in *use*, not in passing checks." Each task substitutes "verify the file structure via `read_markdown`" for the failing-test step.

---

## Chunk 1: Create the template, index, CLAUDE.md rule, deprecation banner

### Task 1: Create `docs/issues/_TEMPLATE.md`

**Files:**
- Create: `docs/issues/_TEMPLATE.md`

**Responsibility:** Source of truth for the bug-file structure. Contains frontmatter (with `status: template`), a top HTML comment block carrying conventions (trigger rules summary, status semantics, INDEX-pairing reminder, archive-detection note), all twelve section headings with italicized capture-discipline prompts.

- [ ] **Step 1: Verify the file does not yet exist**

Run: `mcp__codescout__tree(path="docs/issues/")`
Expected: entries show `memory-leak-x-session-freeze.md`, `2026-04-16-mcp-cancel-disconnect.md`, `archive/`, but no `_TEMPLATE.md`.

- [ ] **Step 2: Write the template file**

Use `mcp__codescout__create_file` (or `Write`) at `docs/issues/_TEMPLATE.md`. Content:

```markdown
---
status: template
opened: YYYY-MM-DD
closed:
severity: medium
owner: marius
related: []
tags: []
---

<!--
BUG TRACKER TEMPLATE — do not edit content; copy this file.

To open a bug:
  1. Copy this file to docs/issues/$(date -I)-<slug>.md
  2. Replace this comment block with the bug content.
  3. Append a row to docs/issues/INDEX.md ## Active in the SAME response.

Trigger rules — open a tracker for ANY bug noticed during work:
  ✓ User explicitly asks ("log this", "open a tracker")
  ✓ Bug blocking the current task (fix-now or parking-lot)
  ✓ Incidental bug we won't fix in the current session
  ✓ Just-fixed bug whose investigation is worth preserving
  ✓ Tool quirks / misbehaviors (formerly the BUG-XXX log)
  ✗ Pure typos / one-token corrections — commit message is enough
  ✗ Feature ideas / refactors — those go in docs/trackers/ or docs/plans/
  ✗ Subjective dislikes that aren't bugs

Status field semantics:
  open          — Logged, investigation not started or paused.
  investigating — Actively being worked on this session.
  fixed         — Root cause addressed, regression test added, verified.
  mitigated     — Workaround in place; root cause not addressed.
  wontfix       — Intentionally not fixing; justification in the file.
  `closed:` stays empty at creation — fill in YYYY-MM-DD only when
  status flips to fixed/mitigated/wontfix.

Archive trigger: move the file into docs/issues/archive/ AFTER the fix
ships to master, not when status flips to fixed. Detect with:
  git branch --contains <fix-sha>
If `master` is in the output, the fix is on master.

Use `N/A` or `Unknown — under investigation` for sections that don't
yet apply. `N/A` in `Tests added` requires justification — empty Tests
added without justification means the bug isn't really closed.
-->

# BUG: <one-line summary>

## Summary
*1–3 sentences. What's broken, who's affected, the elevator pitch.*

## Symptom (Effect)
*Capture the EXACT observable behavior. Verbatim error string in a code
fence (no paraphrasing). Exit code if any. Timing if relevant. What was
observed, not what it means.*

## Reproduction
*Minimal copy-pasteable steps. Include git commit (`git rev-parse HEAD`)
and how to invoke (`cargo run --release` / `/mcp` / etc). If not yet
reproducible, write `Not yet reproducible — best lead: …` and stop.*

## Environment
*OS, language/runtime versions, MCP transport, project, branch. Anything
that moves the reproducibility line.*

## Root cause
*Mechanism, in mechanism-language ("X holds a lock while Y waits on it"),
not symptom-language. Cite `path:line` for every claim. If unknown, write
`Unknown — see Hypotheses tried` and link.*

## Evidence
*One subsection per piece of evidence. Include the source of the evidence
(`.codescout/diagnostic-XXXX.log`, session JSONL path, command output).
Quote rather than summarize — copy the relevant lines into a code fence.*

## Hypotheses tried
*Numbered list. Each entry: **Hypothesis** / **Test** (what we did to check) /
**Verdict** (confirmed | rejected | deferred) / **Evidence link** (anchor
to the Evidence subsection). Append; never delete rejected ones — they
are how future-me avoids re-walking dead ends.*

## Fix
*Plan first, implementation second. When implemented, list commit SHAs and
where the actual change lives (e.g. `src/server.rs:202-358`). If "Fix" is
just a workaround, say so explicitly and keep status `mitigated`, not `fixed`.*

## Tests added
*Regression test name + `path:line`. If the test is intentionally absent,
say why (timing-dependent, env-specific, manual-only). Empty `Tests added`
without justification means the bug isn't really closed.*

## Workarounds
*What users can do RIGHT NOW to unblock themselves while a fix lands.*

## Resume
*Concrete next action, not a goal. Bad: "investigate the LSP path". Good:
"diff src/lsp/client.rs between commits X and Y; check if `did_change` is
sent before `hover` query. Run `cargo test did_change_refreshes` to anchor
behavior." Wipe and replace each session. `N/A` once fixed.*

## References
*Files, dashboards, related issues, external links, session log paths.*
```

- [ ] **Step 3: Verify the file structure**

Run: `mcp__codescout__read_markdown(path="docs/issues/_TEMPLATE.md")`
Expected: heading_count = 12 (one per section), frontmatter shows `status: template`. The HTML comment renders fine.

- [ ] **Step 4: Verify it doesn't break anything**

Run: `mcp__codescout__run_command("cargo test --quiet -- --test-threads=4 2>&1 | tail -20")`
Expected: existing tests still pass (this is a docs-only addition; no tests should be affected).

(Do NOT commit yet — batch all four edits into one commit at the end.)

---

### Task 2: Create `docs/issues/INDEX.md`

**Files:**
- Create: `docs/issues/INDEX.md`

**Responsibility:** At-a-glance summary tables (Active / Mitigated / Recently closed) plus an Archive link. Hand-maintained by Claude in the same edit pass as any bug-file change.

- [ ] **Step 1: Verify the file does not yet exist**

Run: `mcp__codescout__tree(path="docs/issues/")`
Expected: `_TEMPLATE.md` present (from Task 1), `INDEX.md` absent.

- [ ] **Step 2: Write the index file**

Use `mcp__codescout__create_file` at `docs/issues/INDEX.md`. Content:

```markdown
# Bug Tracker Index

Live summary of `docs/issues/`. Hand-maintained — every bug-file edit
must include a paired update here in the same response.

## Active

| Bug | Severity | Status | Opened | Owner | Tags |
|-----|----------|--------|--------|-------|------|

## Mitigated

| Bug | Severity | Mitigated | Workaround | Tags |
|-----|----------|-----------|------------|------|

## Recently closed (last 90 days)

| Bug | Severity | Closed | Fix commit | Tags |
|-----|----------|--------|-----------|------|

## Archive

Older closed bugs: see [`archive/`](archive/).
```

(All three tables start empty. Existing files in `docs/issues/` will be backfilled in a separate session — out of scope for this plan, per the spec's "Out of scope" section.)

- [ ] **Step 3: Verify the file structure**

Run: `mcp__codescout__read_markdown(path="docs/issues/INDEX.md")`
Expected: heading_count = 4 (Active, Mitigated, Recently closed, Archive). Each table has the right column headers.

---

### Task 3: Add `## Bug Tracking` section to `CLAUDE.md`

**Files:**
- Modify: `CLAUDE.md` (add a new `## Bug Tracking` section)

**Responsibility:** A trigger rule visible at every session start, pointing at the template/index/archive. Section appears between `## Tool Misbehavior Log — MANDATORY` and `## Session Intelligence Trackers` to maintain topical grouping (the deprecated misbehavior log is conceptually replaced by the new bug tracker, and the bug tracker sits naturally next to other living trackers).

- [ ] **Step 1: Read the current heading map of CLAUDE.md**

Run: `mcp__codescout__read_markdown(path="CLAUDE.md")`
Expected: confirm `## Tool Misbehavior Log — MANDATORY` exists. Confirm `## Session Intelligence Trackers` exists. Note their line numbers for the next step.

- [ ] **Step 2: Insert the new section after `## Tool Misbehavior Log — MANDATORY`**

Run:
```
mcp__codescout__edit_markdown(
  path="CLAUDE.md",
  action="insert_after",
  heading="## Tool Misbehavior Log — MANDATORY",
  content="\n## Bug Tracking\n\nIf you notice or find a bug while working, open a bug tracker for it. This\napplies to everything: codescout's own behavior, MCP tools, LSP, plugin\nhooks, build scripts, anything that misbehaves.\n\n- Template: `docs/issues/_TEMPLATE.md`\n- Active bugs: `docs/issues/YYYY-MM-DD-<slug>.md`\n- Index: `docs/issues/INDEX.md` (update in the same response as the bug file)\n- Archive (after fix ships to master): `docs/issues/archive/`\n\nTrigger rules and status semantics are documented at the top of\n`docs/issues/_TEMPLATE.md`.\n\n`docs/TODO-tool-misbehaviors.md` is deprecated — do not add new entries.\n"
)
```

- [ ] **Step 3: Verify the section was inserted in the right place**

Run: `mcp__codescout__read_markdown(path="CLAUDE.md", heading="## Bug Tracking")`
Expected: returns the new section verbatim. The siblings list in the response should show `## Tool Misbehavior Log — MANDATORY` (preceding) and `## Session Intelligence Trackers` (following).

- [ ] **Step 4: Confirm full file is still well-formed**

Run: `mcp__codescout__read_markdown(path="CLAUDE.md")`
Expected: heading_count incremented by exactly 1 from before the edit. No accidental section duplication or out-of-order headings.

---

### Task 4: Add deprecation banner to `docs/TODO-tool-misbehaviors.md`

**Files:**
- Modify: `docs/TODO-tool-misbehaviors.md` (heading rename + banner inserted as first content)

**Responsibility:** Redirect future tool-quirk reports to the new tracker without removing existing historical entries. The banner is the first thing any reader sees under the title.

- [ ] **Step 1: Read the current top of the file**

Run: `mcp__codescout__read_markdown(path="docs/TODO-tool-misbehaviors.md", heading="# Tool Misbehaviours — Living Log")`
Expected: returns the current title + intro paragraph(s). No existing deprecation banner.

- [ ] **Step 2: Update the title to include the deprecation marker**

Use `mcp__codescout__edit_file` for a literal-string heading update:

```
mcp__codescout__edit_file(
  path="docs/TODO-tool-misbehaviors.md",
  old_string="# Tool Misbehaviours — Living Log",
  new_string="# Tool Misbehaviours — Living Log [DEPRECATED 2026-05-09]"
)
```

- [ ] **Step 3: Insert the deprecation banner as the first content under the title**

Run:
```
mcp__codescout__edit_markdown(
  path="docs/TODO-tool-misbehaviors.md",
  action="insert_after",
  heading="# Tool Misbehaviours — Living Log [DEPRECATED 2026-05-09]",
  content="\n> **Going forward, all new tool quirks and misbehaviors are tracked as bug\n> files in `docs/issues/<date>-<slug>.md` using `docs/issues/_TEMPLATE.md`.**\n> Do not add new `BUG-XXX` entries below — open a bug file instead.\n> Existing entries stay here for historical reference; they will be\n> migrated in a future bulk pass.\n"
)
```

- [ ] **Step 4: Verify the banner is the first content under the title**

Run: `mcp__codescout__read_markdown(path="docs/TODO-tool-misbehaviors.md", heading="# Tool Misbehaviours — Living Log [DEPRECATED 2026-05-09]")`
Expected: response shows the blockquote banner before the existing `## Before starting any task` section.

- [ ] **Step 5: Verify the rest of the file is intact**

Run: `mcp__codescout__read_markdown(path="docs/TODO-tool-misbehaviors.md")`
Expected: heading_count is the same as before the edit (the banner is content, not a new heading). All existing `BUG-XXX` entries still listed in the heading map.

---

### Task 5: Pre-commit sanity sweep

**Files:** none — verification only.

**Responsibility:** Confirm documentation-only changes haven't broken anything Rust-side, then verify all four edits are present and well-formed.

- [ ] **Step 1: Format check**

Run: `mcp__codescout__run_command("cargo fmt --check")`
Expected: exit_code 0, no diff. (Documentation changes shouldn't touch any `.rs` file, so this is a sanity check that nothing else snuck in.)

- [ ] **Step 2: Clippy**

Run: `mcp__codescout__run_command("cargo clippy -- -D warnings 2>&1 | tail -20")`
Expected: exit_code 0, "no warnings" or equivalent.

- [ ] **Step 3: Test suite**

Run: `mcp__codescout__run_command("cargo test --quiet 2>&1 | tail -10", timeout_secs=180)`
Expected: all tests pass; test count unchanged from pre-edit baseline.

- [ ] **Step 4: Scope-creep check via diff stats**

Run: `mcp__codescout__run_command("git diff --stat")`
Expected: changes only in `docs/issues/_TEMPLATE.md`, `docs/issues/INDEX.md`, `CLAUDE.md`, `docs/TODO-tool-misbehaviors.md`. No `.rs`, no `Cargo.*`, nothing else. If anything else appears, investigate before committing.

- [ ] **Step 5: Files-on-disk check**

Run: `mcp__codescout__tree(path="docs/issues/")`
Expected: entries include `_TEMPLATE.md`, `INDEX.md`, plus the pre-existing files (`memory-leak-x-session-freeze.md`, etc.) and `archive/`.

- [ ] **Step 6: Heading-map spot checks**

Run all four in parallel:
```
mcp__codescout__read_markdown(path="docs/issues/_TEMPLATE.md")
mcp__codescout__read_markdown(path="docs/issues/INDEX.md")
mcp__codescout__read_markdown(path="CLAUDE.md", heading="## Bug Tracking")
mcp__codescout__read_markdown(path="docs/TODO-tool-misbehaviors.md", heading="# Tool Misbehaviours — Living Log [DEPRECATED 2026-05-09]")
```

Expected: each call returns content matching its task's spec. No errors.

---

### Task 6: Commit

**Files:** all four touched in Tasks 1–4.

- [ ] **Step 1: Confirm working tree state**

Run: `mcp__codescout__run_command("git status")`
Expected:
- New files: `docs/issues/_TEMPLATE.md`, `docs/issues/INDEX.md`
- Modified: `CLAUDE.md`, `docs/TODO-tool-misbehaviors.md`
- Nothing else.

- [ ] **Step 2: Write the commit message to a temp file**

Use `mcp__codescout__create_file` (or `Write`) at `/tmp/codescout-bug-tracker-commit-msg.txt`. Content:

```
docs(issues): introduce bug tracker template + index

Adds docs/issues/_TEMPLATE.md (12-section per-bug template with
italicized capture-discipline prompts, self-contained conventions
block) and docs/issues/INDEX.md (three-table summary, hand-maintained
in the same edit pass as any bug-file change).

Adds a § Bug Tracking section to CLAUDE.md so the trigger rule is
visible at every session start.

Marks docs/TODO-tool-misbehaviors.md as DEPRECATED 2026-05-09 with a
banner redirecting future tool-quirk reports to the new tracker.
Existing BUG-XXX entries stay for historical reference; bulk
migration happens in a separate session.

Spec: docs/superpowers/specs/2026-05-09-bug-tracker-template-design.md (b308c70).

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
```

(Writing the message to disk first sidesteps quoting/HEREDOC fragility when passing multi-line content through `run_command`.)

- [ ] **Step 3: Stage and commit using the temp file**

Run:
```
mcp__codescout__run_command("git add docs/issues/_TEMPLATE.md docs/issues/INDEX.md CLAUDE.md docs/TODO-tool-misbehaviors.md && git commit -F /tmp/codescout-bug-tracker-commit-msg.txt")
```
Expected: exit_code 0; output shows the commit hash and the four-file change summary.

- [ ] **Step 4: Clean up the temp commit-message file**

Run: `mcp__codescout__run_command("rm /tmp/codescout-bug-tracker-commit-msg.txt")`
Expected: exit_code 0.

- [ ] **Step 5: Verify commit landed**

Run: `mcp__codescout__run_command("git log --oneline -3 && git status")`
Expected: top line shows the new commit on `experiments`. Status is `nothing to commit, working tree clean`.

---

## Out of scope (do not do as part of this plan)

- Migrating existing `BUG-XXX` entries from `docs/TODO-tool-misbehaviors.md` to per-file bug trackers. Separate session.
- Backfilling `docs/issues/INDEX.md` with rows for the existing `docs/issues/*.md` files (`memory-leak-x-session-freeze.md`, etc.). Separate session.
- Cherry-picking the implementation commit to `master`. Per the project's standard ship sequence, this happens later after the template has seen real use on `experiments`.
- Adding a codescout-native index drift check tool. Deferred per the spec — only built if drift via discipline becomes a recurring problem.

## Verification after merge to master

Once the implementation commit lands on `master` (separate session):

- New session start picks up the `## Bug Tracking` section in CLAUDE.md.
- `mcp__codescout__tree(path="docs/issues/")` shows `_TEMPLATE.md` and `INDEX.md`.
- `mcp__codescout__read_markdown(path="docs/issues/_TEMPLATE.md")` returns the 12-section template structure.
