# TAXONOMY — ID prefixes used in this repo

A single-page reference for every monotonic-ID ledger in the project. Use it
when (a) appending a new observation and you're not sure which tracker, or
(b) onboarding and trying to navigate the accumulated session intelligence.

The accumulated rules are spread across `CLAUDE.md`, individual tracker
templates, and skill SKILL.md files. This page is the index, not the spec —
follow the links for the controlling convention.

## Main taxonomy

| Prefix | Lives in | Captures | Append tool | Promotes to |
|---|---|---|---|---|
| **F-N** | `docs/trackers/<topic>-session-log.md` | Per-work-stream friction observation: plan-vs-reality drift, surprise tool behavior, blocker | `edit_markdown(action="insert_before", heading="## Template for new entries")` | Bug file (`docs/issues/`) if reproducible; W-N pair if pattern caught it next time |
| **W-N** | Same file as F-N | Per-work-stream win: a discipline / scout / pattern that prevented a worse outcome with named counterfactual | Same | CLAUDE.md / ADR / skill SKILL.md after 2+ confirming datapoints |
| **R-N** | `docs/trackers/reconnaissance-patterns.md` | Meta — observations about the **recon skill itself**: hits, misses, vocabulary expansions | `edit_markdown(action="insert_before", heading="## Template for new entries")` | PR against `codescout-companion/skills/reconnaissance/SKILL.md` |
| **U-N** | `docs/trackers/codescout-usage-frictions.md` | Friction using codescout tools / MCP server: tool slips, prompt drift, hook false-positives | `edit_markdown(action="insert_after", heading="### U-<previous>")` | H-N hookify rule, CLAUDE.md note, or prompt-surface edit |
| **H-N** | `docs/trackers/codescout-usage-hookify.md` | Hook design proposal: warn → deny criteria, new gate ideas, false-positive carve-outs | `edit_markdown(action="insert_after", heading="### H-<previous>")` | Shipped hook in `claude-plugins/codescout-companion/hooks/` |
| **T-N** | `docs/trackers/tool-usage-patterns.md` (augmented artifact `b3fa993849ac83ab`) | Tool-selection quality observation: legitimate / debatable / wrong-tool call with prompt gap | `artifact_augment(merge=true, params={observations: [..., {id:"T-N", ...}]})` + body prose via `edit_markdown` | `src/prompts/source.md` edits (server-instructions surface) |
| **BUG (slug)** | `docs/issues/YYYY-MM-DD-<slug>.md` | Per-bug investigation file: Symptom / Repro / Root cause / Fix / Workaround | Create from `docs/issues/_TEMPLATE.md`; status field in frontmatter | Archived to `docs/issues/archive/` after the fix ships to master (verify with `git branch --contains <fix-sha>`) |

## Work-stream-specific prefixes (not durable taxonomy slots)

These appear inside individual session logs / specs and are scoped to one
work stream — not project-wide ID namespaces. The session log defines them
in its own header; they don't need slots here.

- **S-NN** — Session residuals: open follow-ups when a multi-session work
  stream wraps. Example: `docs/trackers/2026-05-07-retrieval-session-residuals.md`.
- **D-NN** — Design decisions inside a spec. Example: a multi-decision
  spec uses D-1 through D-N for the decisions enumerated in that document.

If you find yourself wanting to introduce a new project-wide prefix, ask first
whether it really earns a slot or whether it's a variant of one of the seven
above.

## How to choose

```
You observed something. Where does it go?
│
├─ Is it a bug? (wrong output, silent failure, corrupt state)
│   → BUG file in docs/issues/  (open one per the CLAUDE.md trigger rules)
│
├─ Is it a friction with codescout / MCP / hooks / Iron Laws?
│   → U-N in codescout-usage-frictions.md
│
├─ Is it a design idea for a hook / gate / IL refinement?
│   → H-N in codescout-usage-hookify.md
│
├─ Is it a tool-selection observation worth reviewing later?
│   → T-N in tool-usage-patterns.md (artifact b3fa993849ac83ab)
│
├─ Is it about the recon skill itself (hit / miss / proposal)?
│   → R-N in reconnaissance-patterns.md
│
└─ Is it a per-work-stream friction or win, scoped to one task / refactor?
    ├─ Friction → F-N in <topic>-session-log.md
    └─ Win      → W-N in same file (with counterfactual)
```

## Promotion ladder

```
F-N / W-N    (session log — per work stream, archived when wrapped)
  │
  ├──→ BUG file   if friction stabilizes into a reproducible bug
  │
  └──→ CLAUDE.md / ADR / SKILL.md   if win confirmed 2+ times across work streams
                                    (promote-when criterion fires)

U-N          (codescout usage frictions — durable across sessions)
  │
  ├──→ H-N hookify rule   if friction can be substrate-enforced
  │
  └──→ CLAUDE.md / prompt-surface edit   if convention or guidance fix

H-N          (hookify proposal)
  │
  └──→ Shipped hook in codescout-companion/hooks/  (PR + merge)

R-N          (recon-skill meta)
  │
  └──→ PR against codescout-companion skill SKILL.md   (promote-when fires)

T-N          (tool-usage patterns)
  │
  └──→ src/prompts/source.md edits   (server-instructions surface)

BUG          (per-bug investigation)
  │
  └──→ Fix lands on master + archive move to docs/issues/archive/
```

## SHA-citation rule

Every prefix above may cite git commits as evidence. After cherry-pick + rebase,
experiments-side SHAs orphan. Cite the **master SHA** captured immediately after
`git cherry-pick` lands on master — see `CLAUDE.md § After cherry-pick`.

For cross-repo citations (e.g. tracker in codescout pointing at a fix in
codescout-companion), prefix with the repo name: `codescout-companion:0b75991`.

## Status vocabularies (per prefix)

Different prefixes use slightly different status enums:

- **F-N statuses** — `open | mitigated | fixed-verified | wontfix-false-alarm | promoted-to-bug-tracker | pinned-as-eval-baseline` (canonical in `docs/templates/session-log.md`).
- **W-N statuses** — `validated | promoted-to-permanent-docs | archived`.
- **U-N statuses** — `open | fixed-shipped | promoted | wontfix` (informal).
- **H-N statuses** — `warn | deny | shipped | rejected`.
- **R-N verdicts** — `hit | miss | proposal | promoted`.
- **T-N verdicts** — `legitimate | debatable | wrong-tool`.
- **BUG statuses** — `open | investigating | fixed | mitigated | wontfix | zombie` (canonical in `docs/issues/_TEMPLATE.md`).

When in doubt, mirror the existing entries in that file — consistency beats
correctness here.
