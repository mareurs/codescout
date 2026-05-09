---
title: Bug Tracker Template — Design
status: approved
created: 2026-05-09
owner: marius
audience: claude (during work sessions)
related:
  - docs/issues/
  - docs/TODO-tool-misbehaviors.md
  - CLAUDE.md
---

# Bug Tracker Template — Design

## Goal

Standardize bug-investigation files in `docs/issues/<date>-<slug>.md` with a shared template, an at-a-glance index, and a CLAUDE.md rule that ensures Claude opens a tracker for every bug noticed during work. Deprecate `docs/TODO-tool-misbehaviors.md` in favor of this tracker.

## Audience

The template is for **Claude (the agent)** to author and update during work sessions, with the human collaborating. It is not a form for end users to fill out. Section prompts and discipline reminders are written for Claude reading the file mid-investigation.

## Out of scope

- Migrating existing `BUG-XXX` entries from `docs/TODO-tool-misbehaviors.md`. That file is deprecated in place; bulk migration happens in a separate session.
- Backfilling existing `docs/issues/*.md` files into the new template. Existing files stay as-is unless they get worked on.
- Adding a codescout-native index check tool. Deferred — only built if drift via discipline becomes a recurring problem.

## File layout

```
docs/issues/
  _TEMPLATE.md                    # the template, status: template
  INDEX.md                        # at-a-glance summary, hand-maintained
  YYYY-MM-DD-<slug>.md            # active bugs
  archive/
    YYYY-MM-DD-<slug>.md          # closed bugs after the fix has shipped to master
```

Slugs are short kebab-case noun-phrases (3–6 words): `memory-leak-x-session-freeze`, `mcp-cancel-disconnect`. Archive moves happen after the fix lands on `master`, not when status flips to `fixed` — so `experiments`-only fixes stay visible until they ship. To check whether a fix has shipped: `git branch --contains <fix-sha>` — if `master` is in the output, the fix is on master and the file can be moved to `archive/`.

## Frontmatter

Thin YAML at the top of every bug file:

```yaml
---
status: open       # open | investigating | fixed | mitigated | wontfix
opened: 2026-MM-DD
closed:            # YYYY-MM-DD when fixed/mitigated/wontfix
severity: medium  # low | medium | high
owner: marius
related:
  - src/lsp/client.rs
  - src/tools/symbol/hover.rs
tags: [lsp, kotlin]
---
```

Not librarian-artifact-shaped. Plain YAML, queryable via `grep`.

## Section list

Twelve sections, every bug file fills out all of them. Use `N/A` or `Unknown — under investigation` when a section doesn't yet apply (use of `N/A` without justification in `Tests added` means the bug isn't really closed).

| # | Heading | Required content |
|---|---------|------------------|
| 1 | `## Summary` | 1–3 sentences. What's broken, who's affected, the elevator pitch. |
| 2 | `## Symptom (Effect)` | Concrete observable behavior. Verbatim error string in a code fence. Exit code, timing if relevant. What was observed, not what it means. |
| 3 | `## Reproduction` | Minimal copy-pasteable steps. Include git commit (`git rev-parse HEAD`) and how to invoke. |
| 4 | `## Environment` | OS, language/runtime versions, MCP transport, project, branch. Anything that affects reproducibility. |
| 5 | `## Root cause` | Mechanism, in mechanism-language ("X holds a lock while Y waits on it"), not symptom-language. Cite `path:line` for every claim. `Unknown — see Hypotheses tried` if not yet found. |
| 6 | `## Evidence` | Logs, command output, file:line refs, diagnostic snippets that support the cause. Quote rather than summarize. |
| 7 | `## Hypotheses tried` | Numbered list. Each entry: **Hypothesis** / **Test** / **Verdict** (confirmed / rejected / deferred) / **Evidence link** (anchor to Evidence subsection). Append; never delete rejected ones. |
| 8 | `## Fix` | Plan first, implementation second. Commit SHAs and `path:line` for the actual change. If "Fix" is just a workaround, say so explicitly and keep status `mitigated`. |
| 9 | `## Tests added` | Regression test name + `path:line`. If intentionally absent, justify (timing-dependent, env-specific, manual-only). |
| 10 | `## Workarounds` | What users can do *now* to unblock themselves while a fix lands. |
| 11 | `## Resume` | For cross-session bugs: concrete next action, not a goal. Replaced each session. `N/A` once fixed. |
| 12 | `## References` | Files, dashboards, related issues, external links, session log paths. |

### Section order rationale

Triage info first (1–4: what is it, what does it look like, how do I trigger it, where does it apply), then *understanding* (5–7: cause, evidence, hypotheses), then *resolution* (8–10: fix, tests, workarounds), then *handoff/lookups* (11–12). A reader can stop reading at any logical boundary and still have a complete view at that level.

### Inline guidance

Each heading has an italicized capture-discipline prompt directly under it — visible at read time, replaceable by content. Italics over HTML comments specifically because comments render silently; Claude needs to *see* the discipline reminder when opening the file three sessions later. Full prompt content lives in `_TEMPLATE.md`.

**Example prompts** (anchoring the style for `_TEMPLATE.md`):

```markdown
## Root cause
*Mechanism, in mechanism-language ("X holds a lock while Y waits on it"),
not symptom-language. Cite `path:line` for every claim. If unknown, write
`Unknown — see Hypotheses tried` and link.*

## Hypotheses tried
*Numbered list. Each entry: **Hypothesis** / **Test** (what we did to check) /
**Verdict** (confirmed | rejected | deferred) / **Evidence link** (anchor
to the Evidence subsection). Append; never delete rejected ones — they
are how future-me avoids re-walking dead ends.*
```

Other sections follow the same pattern: a 1–3 sentence directive, italicized, that names what to capture and what to avoid. Extrapolate the remaining ten from this style.

## Status semantics

| Status | Meaning | When to set |
|---|---|---|
| `open` | Logged, investigation not started or paused. | At creation, or when work pauses without progress. |
| `investigating` | Actively being worked on this session. | At session start when about to dig in. |
| `fixed` | Root cause addressed, regression test added, verified. | After fix lands and test passes. |
| `mitigated` | Workaround in place; root cause not addressed (or only partial). | When bleeding stopped but cause not fixed. |
| `wontfix` | Intentionally not fixing. Justification in the file. | Rare. |

`closed: YYYY-MM-DD` set in frontmatter alongside any of `fixed` / `mitigated` / `wontfix`.

## When to open a bug file (trigger rules)

**Default rule: if Claude notices or finds a bug, Claude opens a tracker.** This applies to everything: codescout's own behavior, MCP tools, LSP, plugin hooks, build scripts, anything that misbehaves. The tracker can be minimal for trivial bugs (frontmatter + Summary + Symptom + Repro + Fix-with-commit, everything else `N/A`) or extensive for hairy ones — the single-template-with-N/A choice supports both.

Open one for:

- The user explicitly asks ("log this", "open a tracker").
- A bug blocking the current task (whether fix-now or parking-lot).
- A bug encountered incidentally that won't be fixed in the current session.
- A just-fixed bug whose investigation is worth preserving (non-obvious cause, evidence took work).
- Tool quirks / misbehaviors (formerly the `BUG-XXX` log).

Do NOT open one for:

- Pure typos / one-token corrections — commit message is enough.
- Feature ideas, deferred work, refactor proposals — those go in `docs/trackers/` or `docs/plans/`.
- Subjective dislikes that aren't bugs.

## Index — `docs/issues/INDEX.md`

Three-table layout with an Archive link.

```markdown
# Bug Tracker Index

## Active

| Bug | Severity | Status | Opened | Owner | Tags |
|-----|----------|--------|--------|-------|------|
| [<title>](2026-MM-DD-<slug>.md) | <sev> | <status> | <date> | <owner> | <tags> |

## Mitigated

| Bug | Severity | Mitigated | Workaround | Tags |
|-----|----------|-----------|------------|------|

## Recently closed (last 90 days)

| Bug | Severity | Closed | Fix commit | Tags |
|-----|----------|--------|-----------|------|

## Archive

Older closed bugs: see [`archive/`](archive/).
```

### Index maintenance — same edit pass as the bug file

| Moment | Edits |
|---|---|
| Create a bug file | Write the new file from `_TEMPLATE.md`, then `edit_markdown` INDEX.md to append a row in `## Active`. Same response. |
| Flip status | `edit_markdown` the bug file's frontmatter, then `edit_markdown` INDEX.md to move the row. Same response. |
| Ship to master + archive | `git mv` the file to `archive/`, then `edit_markdown` INDEX.md to update the row's link. Same response. |

The `_TEMPLATE.md` top-of-file comment includes a reminder that creating a bug from the template requires a paired INDEX update.

### Drift verification — existing codescout tools, no script

When verifying INDEX against bug files (e.g. before a commit, or picking up a stale session):

```
mcp__codescout__grep(pattern="^status:", path="docs/issues/")
mcp__codescout__read_markdown(path="docs/issues/INDEX.md")
mcp__codescout__tree(path="docs/issues/", recursive=true)
```

No external script. No hook. No new dependency. If drift via discipline becomes a recurring problem, the future fix is a codescout-native check (a tool or skill), not a sidecar bash script — but that work is out of scope for this design.

## Deprecation of `docs/TODO-tool-misbehaviors.md`

Add a banner at the top of the file:

```markdown
# Tool Misbehaviours — Living Log [DEPRECATED 2026-05-09]

> **Going forward, all new tool quirks and misbehaviors are tracked as bug
> files in `docs/issues/<date>-<slug>.md` using `docs/issues/_TEMPLATE.md`.**
> Do not add new `BUG-XXX` entries below — open a bug file instead.
> Existing entries stay here for historical reference; they will be
> migrated in a future bulk pass.
```

No content removal. Existing `BUG-XXX` entries stay readable for historical reference.

## CLAUDE.md rule

Add a `## Bug Tracking` section to `CLAUDE.md`. Wording:

```markdown
## Bug Tracking

If you notice or find a bug while working, open a bug tracker for it. This
applies to everything: codescout's own behavior, MCP tools, LSP, plugin
hooks, build scripts, anything that misbehaves.

- Template: `docs/issues/_TEMPLATE.md`
- Active bugs: `docs/issues/YYYY-MM-DD-<slug>.md`
- Index: `docs/issues/INDEX.md` (update in the same response as the bug file)
- Archive (after fix ships to master): `docs/issues/archive/`

Trigger rules and status semantics are documented at the top of
`docs/issues/_TEMPLATE.md`.

`docs/TODO-tool-misbehaviors.md` is deprecated — do not add new entries.
```

## Implementation order

1. **Create `docs/issues/_TEMPLATE.md`** — frontmatter, top comment (with INDEX-pairing reminder + trigger rules summary + status semantics summary, so the template is self-contained), all 12 sections with italicized capture-discipline prompts.
2. **Create `docs/issues/INDEX.md`** — empty Active / Mitigated / Recently closed tables and Archive link. Initial content: just the headers and the layout above.
3. **Add `## Bug Tracking` section to `CLAUDE.md`** — verbatim from the section above.
4. **Add deprecation banner to `docs/TODO-tool-misbehaviors.md`** — verbatim from the section above.

Each step is one file edit. No tests, no Rust changes, no script. Total scope: four file edits.

## Validation after implementation

- `_TEMPLATE.md` opens cleanly, italicized prompts render in any markdown viewer.
- `INDEX.md` parses (well-formed markdown tables).
- `CLAUDE.md` rule appears at session start (verify by `mcp__codescout__read_markdown(path="CLAUDE.md", heading="## Bug Tracking")`).
- Deprecation banner is the first content under the `# Tool Misbehaviours` heading in `docs/TODO-tool-misbehaviors.md`.

No automated test. The template's value is in *use*, not in passing checks.
