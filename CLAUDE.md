# codescout

Rust MCP server giving LLMs IDE-grade code intelligence — symbol-level navigation, semantic search, git integration. Inspired by [Serena](https://github.com/oraios/serena).

You are a proficient Rust developer. You follow all known good/scalable patterns. You are honest and recognize your limits and your mistakes, you own them. If you are not sure, you always ask me for feedback.

## Development Commands

**Run `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test` before completing any task.** On our stack the live-MCP release build is `cargo rb` (not `cargo build --release`); after it, run `/mcp` to reconnect. Full command reference (every crate + fixture, `cargo rb` vs lean build) → memory `development-commands`; the binary symlink gotcha → memory `gotchas` (MCP Binary Symlink).
## Bug Tracking

**Per-file bug tracking lives in `docs/issues/`.** Every bug noticed during work gets its own file, copied from `docs/issues/_TEMPLATE.md`. Path, slug, the `status:` vocabulary (`open | investigating | fixed | mitigated | wontfix | zombie`), and the archive flow are documented in **`get_guide("tracker-conventions")` § Bug files**.

Two behaviors are load-bearing and easy to skip:

- **Capture on notice** — add the bug file the moment a bug is noticed (wrong edits, corrupt output, silent failures, misleading errors from codescout's own MCP tools), not at task end.
- **Archive once the fix is verified on `experiments`** — gate green plus a regression test. Reaching `master` is **not** required; `experiments` is never deleted. Archive via `artifact(action="move", …)`, never a bare `git mv`.
- **The pending-master-SHA line is for cherry-picks only.** If the fix will reach `master` by *cherry-pick*, label the SHA `experiments` and keep a `## Resume` line saying the master-side SHA still needs recording — the original orphans on the next rebase, and nothing re-reads `archive/` to repair it. If the promotion is a *fast-forward*, do **not** write that line: there is no second SHA, and the instruction sends a later session hunting for one that will never exist. (Measured 2026-08-08: 24 archived bug files carry the cherry-pick form, written before the cohort's path was settled as fast-forward. They are stale instructions, not open debt — the SHA in each is already correct.)

**Open a bug file for ANY bug noticed during work** — including incidental bugs we won't fix and tool quirks/misbehaviors. *Not* for pure typos (commit message suffices) or feature ideas/refactors (→ `docs/trackers/` or `docs/plans/`). Don't append to retired surfaces (`docs/archive/old-trackers/*`) — open a new `docs/issues/<date>-<slug>.md`.
## Session Intelligence Trackers

**One-page index of every ID prefix** (F-N / W-N / R-N / U-N / H-N / T-N / BUG) — file, scope, append tool, promotion path — lives in [`docs/TAXONOMY.md`](docs/TAXONOMY.md). Start there when you're not sure which tracker takes an observation.

### Querying active trackers (librarian)

Frontmatter shape, status vocabulary, archiving-through-the-catalog, and the
canonical `artifact(action="find", kind="tracker"|"bug", …)` queries live in
**`get_guide("tracker-conventions")`** — call it when creating, querying, or
archiving a tracker/bug. The one-page index of every ID prefix
(F-N / W-N / R-N / T-N / U-N / H-N / BUG) is `docs/TAXONOMY.md`.

> ⚠️ Archive trackers **through the librarian** — `artifact(action="update", …, patch={status:"archived"})` then `artifact(action="move", …)` — never a bare `status:` edit + `git mv`: `id = sha256(abs_path)`, so a hand-move orphans the catalog row's events/augmentation. Full rationale in `get_guide("tracker-conventions")`.

Two living trackers capture observations from real sessions. Keep them current — they feed
prompt improvements and skill refactors.
### Cross-project scope — the librarian umbrella (name is per-machine)

codescout participates in a librarian **umbrella** — a group of sibling projects
that `scope="umbrella"` queries span, so an `artifact(find/context/…)` from here
can surface a peer's trackers and bug files, and the reverse.

**The umbrella's name and membership are per-machine and are NOT documented here.**
They live in the global registry (`~/.config/librarian/workspace.toml`, or
`%APPDATA%\librarian\workspace.toml` on Windows) and differ between hosts. This
paragraph used to name a specific umbrella and four specific members; on
2026-08-08 that was found to match no host — the name did not exist in the
registry and two of the four members were wrong. A machine-specific fact stated
as a project-wide one cannot stay true, and nothing gates it.

To see what actually resolves, ask the tool rather than the doc: any
`artifact(action="find")` response carries a `scope` block naming the resolved
umbrella, and the registry file lists every `[[umbrella]]` with its members.

Peers that have appeared in these umbrellas include `claude-plugins` (the
companion plugin, see below), `researcher`, and **`prompt-engineering`** (a.k.a.
prompt-tdd) — the prompt-eval harness that puts codescout's own skills/prompts
under TDD against headless `claude -p` sessions. Trackers of note in that last
one: `skill-eval-log`, `skill-eval-playbook`, and `prompt-tdd-harness-backlog`
(harness gaps the evals surfaced — several are codescout-adjacent, e.g. the
`AnthropicMcpRegistry` setup-file parity fix). Confirm membership before relying
on any of them being in scope.

Umbrella membership resolves by path-prefix (`abs_path.starts_with(member)`).
A project may also declare its own `[[umbrella]]` in its
`.codescout/workspace.toml`; a **project-local umbrella takes precedence** over
the global registry (for the hub-and-spoke case where a main project owns its
dependency set). The global file is for cross-linked peers with no single owner.

**Umbrella ≠ workspace.** Cross-repo grouping belongs in the umbrella. A repo's
`.codescout/workspace.toml` `[[project]]` entries are its own **sub-projects**,
with roots relative to that repo — never sibling repos. Getting this wrong is
silent: that file is gitignored, so a mis-rooted entry redirects every
per-project memory write into the wrong repo with no review able to see it. It
happened here on 2026-07-07 and was found on 2026-08-08 —
`docs/issues/2026-08-08-gitignore-projects-rule-premise-false-on-a-real-host.md`.
### Skill Frictions — `docs/trackers/skill-frictions.md`

Rough edges found while using project skills (`/claude-traces`, `/analyze-usage`, etc.).
Entries are numbered F-NNN with root cause, impact, and fix idea.

**Claude — append when:**
- A skill command fails unexpectedly or requires a workaround
- A skill's documented behavior diverges from reality
- A friction recurs across sessions (escalate priority)

**How to append (Claude):**
```
edit_markdown("docs/trackers/skill-frictions.md",
  action="insert_after", heading="## `/<skill-name>`",
  content="### F-NNN — <title>\n**When:** ...\n**Got:** ...\n**Fix idea:** ...")
```

**User — browse:** open `docs/trackers/skill-frictions.md` directly; entries are grouped by
skill. Mark fixed entries with a `(FIXED <date>)` note rather than deleting them.

### Tool Usage Patterns — `docs/trackers/tool-usage-patterns.md`

Observed tool calls from real sessions judged against the ideal — our internal Langfuse for
tool selection quality. Entries are T-NNN with tool, verdict (legitimate / debatable /
wrong-tool), and prompt gap. Feeds Iron Law and Anti-Patterns updates.

This file is a **librarian artifact** (id: `f2ecdd76a6189efb`). Params hold the structured
T-N table; body holds full per-observation analysis. For the deep-dive on the
augmented-artifact pattern (body / params / render_template, the `merge=false`
foot-gun, why managed files refuse direct `read_markdown`), see
[`docs/architecture/augmented-artifacts.md`](docs/architecture/augmented-artifacts.md).

**Claude — append when:**
- Analyzing a session and a tool choice is noteworthy (right or wrong)
- A new pattern emerges that isn't already covered by an existing T-N entry

**How to append (Claude):**
```
# 1. Add structured entry to params
artifact_augment(id="f2ecdd76a6189efb", merge=true,
  params={observations: [...existing..., {id:"T-NNN", tool:"...", verdict:"...", ...}]})

# 2. Add analysis prose to body
edit_markdown("docs/trackers/tool-usage-patterns.md",
  action="insert_before", heading="## Prompt improvement candidates",
  content="### T-NNN — <title>\n...")
```

**User — browse:** open `docs/trackers/tool-usage-patterns.md`; the live params table is
rendered at the top by the librarian. Prompt improvement candidates are at the bottom —
these are the direct inputs to `src/prompts/source.md` (the `server_instructions` surface slice) edits.

### Ad-Hoc Session Logs — `docs/trackers/<topic>-session-log.md`

Per-work-stream observation log used during multi-session efforts (reviews, multi-task
plans, refactors). Two-sided: frictions (F-N) and wins (W-N). Distinct from **Skill
Frictions** (durable across projects) and **Tool Usage Patterns** (a librarian artifact) —
session logs are scoped to a single work stream and archived when it wraps.

The canonical template lives at `docs/templates/session-log.md`. Copy it to
`docs/trackers/<topic>-session-log.md` on the first reconnaissance pass of a
multi-session work stream. The Status vocabulary and category conventions are pinned in
the template so they mean the same thing across sessions and across agents.

This surface is driven by the **reconnaissance** skill (codescout-companion). Any agent
that can read markdown can use the template — no plugin required. Claude Code users get
slash-command access via `/codescout-companion:reconnaissance`.

**Claude — append when:**
- A scout discovers drift between plan and reality (→ F-N entry)
- A practice prevented a worse outcome and you can name the counterfactual (→ W-N entry)
- A friction surfaces during reconnaissance Phase 2 (compare reality to plan)

**How to append (Claude):**
```
edit_markdown("docs/trackers/<topic>-session-log.md",
  action="insert_before", heading="## Template for new entries",
  content="## F-N — <title>\n**Observed:** ...\n**Got:** ...")
# Also append a row to the Index / Wins Index table at the top of the file.
```

**User — browse:** open `docs/trackers/<topic>-session-log.md`. Index tables at the top
show all entries; full body holds evidence. Promote `Status: validated` wins to
permanent surfaces (CLAUDE.md, ADRs, skills) when their `Promote-when` criterion fires.

**Eval (Claude only):** the trigger string for the reconnaissance skill is scored
against `docs/evals/reconnaissance-trigger.md`. Re-score before any future SKILL.md
description change — empirical baseline (2026-05-17) is 6/7 at threshold.


**Verify-open cadence (added 2026-05-25 after W-7 promotion):** Before any "what's open?"
report or backlog triage, run a verify-open pass on session-log entries with `Status: open`
older than 14 days — reconcile the body status against current code + the bug-file archive.
Distributed fixes leave entries zombie-open by default: a fix shipping under a `fix(ci): ...`
or `feat(...): ...` commit message rather than one naming the tracker entry doesn't trip any
automated gate. Evidence: the W-7 scout pass (2026-05-25, `docs/trackers/bug-fix-session-log.md`)
flipped 3 of 4 nominally-open F-N entries to `fixed-verified` / `mitigated` in a single pass —
75% zombie-open rate in one tracker. Pairs with Standard Ship Sequence step 4 (bug-file archive
discipline at the `docs/issues/` level) and the `audit_doc_refs` CI gate (doc-link drift at the
markdown-reference level) — three independent bookkeeping surfaces leak the same way under the
same root cause (fix-then-forget), and the project's hygiene discipline is now complete across
all three.

## Git Workflow

**`master` is protected** — all experimental work on `experiments`; promote to `master` only after tests + clippy + MCP verify; `experiments` is never deleted; never commit in-progress work directly to `master`.

**Two promotion paths, and the difference decides how you cite SHAs.** *Cherry-pick* (single fixes) mints a **new** SHA on `master`, and the `experiments`-side original orphans on the following rebase — so the master SHA must be recorded afterwards. *Fast-forward* (large cohorts) moves `master` onto the **exact** commits: no new SHAs, the `experiments` SHA already **is** the master SHA, nothing to re-cite. Check which one you are in before recording any SHA — `git rev-list --left-right --count master...experiments`, a `0` on the left means fast-forward is available.

Full release cycle, standard ship sequence, after-cherry-pick master-SHA rule, chained-git state-check, and concurrent-work reset safety → **`docs/RELEASE.md`**. SHA-citation + cross-repo `<repo>:<sha>` prefix discipline → memory `gotchas`. Commit style → memory `conventions`.
## Design Principles

codescout's conventions and design principles live in memory (auto-listed at session start) — read them when writing codescout code:

- **`conventions`** — pre-commit gate, error handling (`RecoverableError` vs `anyhow::bail!`), no-echo writes (`json!("ok")`), the `call_content()` MCP entry point, progressive disclosure / two modes, **Agent-Agnostic Design**, testing patterns (three-query sandwich, `EnvGuard`), prompt-surface consistency, commit style.
- **`architecture`** — module map, key abstractions, data flow, the three prompt surfaces.

Before adding or modifying any tool, read `docs/PROGRESSIVE_DISCOVERABILITY.md`. Full error decision tree: `get_guide("error-handling")`. Test isolation: `docs/conventions/test-env-isolation.md`.
## Prompt Surface Consistency

Three prompt surfaces (`server_instructions` + `onboarding_prompt` slices of `src/prompts/source.md`, and `build_system_prompt_draft()` in `builders.rs`) must stay tool-name-consistent. Which surfaces exist, when to bump `ONBOARDING_VERSION`, the 2200-byte slice cap + shared-branch verify hazard, and the writing style guide → **`src/prompts/README.md`** (short version: memory `conventions`). Stale tool names are gated by `prompt_surfaces_reference_only_real_tools` (3 surfaces) and `claude_md_contains_no_deprecated_tool_names` (this file).
## Companion Plugin: codescout-companion

A companion Claude Code plugin (`../claude-plugins/codescout-companion/`) is **always active** here. The rule that bites mid-task: **native `Read`/`Grep`/`Glob`/`Edit`/`Write` on source files and all native `Bash` are hard-denied — use codescout's MCP tools** (`symbols`, `grep`, `edit_code`, `read_file`/`read_markdown`, `run_command`). Full hook inventory, cross-repo flow, and concurrent-multi-workspace rules → **`docs/architecture/companion-plugin.md`**.
## Language-Specific LSP Issues

See codescout memory `gotchas` (LSP section) for Kotlin multi-instance conflicts,
cold start behavior, circuit breaker, and LSP mux details.

**Tracking:** the concurrent-instance failures were fixed in `dc44ac3d` (per-instance
system-path, circuit breaker, diagnostics); that bug file was pruned as a dupe in
`c6184884`, so the memory above is the canonical account. Still open:
`docs/issues/2026-06-19-kotlin-lsp-uncapped-jvm-heap.md`.

## Docs

Files:

- **`docs/PROGRESSIVE_DISCOVERABILITY.md`** — Canonical guide for output sizing, overflow hints, and agent guidance patterns. **READ THIS before adding or modifying any tool.**
- `docs/manual/src/architecture.md` — Component details, tech stack, design principles. (There is no docs/ARCHITECTURE.md — deliberately deleted, and left un-code-spanned
  here so the doc-ref audit does not read a statement of absence as a citation — see `docs/superpowers/specs/2026-05-06-doc-audit-design.md` § 1a.)
- `docs/ROADMAP.md` — Quick status overview
- `CONTRIBUTING.md` — Contributor-facing setup + PR checklist
- `docs/RELEASE.md` — Release cycle, ship sequence, git-workflow safety
- `docs/architecture/companion-plugin.md` — codescout-companion hook inventory + cross-repo flow
- `src/prompts/README.md` — prompt-surface rules: surfaces, `ONBOARDING_VERSION`, 2200-byte cap, style guide

Memories (Claude auto-loads these; listed for reference):

- `architecture` — 8-project workspace map, cross-project deps, CI/shared infra; per-project: module structure, key abstractions, data flows
- `conventions` — Commit style, branch strategy, error handling rules, pre-commit requirements; per-project patterns
- `development-commands` — Full command reference (cargo, scripts, release)
- `language-patterns` — Rust anti-patterns and idiomatic patterns
- `gotchas` — Cross-project path resolution pitfalls, symbols truncation, Kotlin LSP, embedding model restrictions, memory leak
- `domain-glossary`, `project-overview`, `system-prompt`, `onboarding` — project self-description
