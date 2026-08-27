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
- **Record the fix SHA *and* its patch-id — never a pending-master-SHA line.** `git show <sha> | git patch-id --stable`. The SHA is positional and dies when `experiments` is rebased (which happens after every ship); the patch-id is a content hash of the diff and survives rebase *and* cherry-pick. Record the pair once at fix time: there is no promotion path to check and nothing owed later. (Measured 2026-08-19: 10 of 63 archived bug files had already lost their SHA to a rebase; zero patch-id collisions across 3594 commits. Many archived files still carry the older master-SHA-owed form — stale instructions, not open debt; do not sweep them.)

**Run the reproduction before reading the fix plan — the plan is a hypothesis about the
reproduction.** Not to confirm the bug exists; to find out what the plan is actually about.
Promoted 2026-08-20 from `bug-fix-session-log:W-32` at four datapoints (with W-30). In one
2026-08-14 sweep it changed the fix three times: a bug filed as "top-level `extra` dropped"
was **five** params dropped by one mechanism, so a per-field fix would have shipped the same
defect a seventh time; a `delete` bug's severity turned on which key *neighboured* it
(scalar above → loud invalid YAML, sequence above → silent corruption), and only reproduction
separates them; and once the fix direction **inverted** — measuring fastembed's real 512-token
ceiling showed the prescribed change would over-chunk large-context models against a hard cap,
so deleting the dead code, not promoting it, was correct. Case 3 would have introduced a bug;
case 1 would have left four siblings broken behind a passing test.

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
### Cross-project scope — librarian umbrellas

An **umbrella** groups several repos so that `scope="umbrella"` on
`artifact(find/context/…)` queries trackers and artifacts across all of them — e.g.
surface codescout bug files from a sibling repo's session, or the reverse. Membership
resolves by path-prefix (`abs_path.starts_with(member)`).

**Umbrella ≠ workspace.** Cross-repo grouping belongs in an `[[umbrella]]`. A repo's
`.codescout/workspace.toml` `[[project]]` entries are its own **sub-projects**, with
roots relative to that repo — never sibling repos. Getting this wrong is silent: that
file is gitignored (`.gitignore:28`), so a mis-rooted entry redirects every per-project
memory write into the wrong repo with no review able to catch it.

A project may declare its own `[[umbrella]]` in `.codescout/workspace.toml`; a
**project-local umbrella takes precedence** over the global registry (the hub-and-spoke
case, where a main project owns its dependency set). The global registry is for
cross-linked peers with no single owner.

**Umbrella names, member lists, and registry paths are per-machine and are deliberately
NOT recorded in this file.** They live in `~/.config/librarian/workspace.toml`, which
sits outside every repo and differs per host — so committing them makes this file read
as *false* to anyone standing on a different machine. That is not hypothetical: it is
what PR #12 concluded, correctly for its host and wrongly for the repo. Two ways to find
out what actually resolves, both better than any doc:

- run a `scope="umbrella"` query and read the returned `scope` block — it reports what
  resolved, not what was intended;
- `memory(action="read", topic="local-environment", private=true)` — this host's values.
  Gitignored (`.gitignore:32`), and **not** shown by `memory(action="list")` unless you
  pass `include_private=true`.

**Related repo:** `prompt-engineering` (a.k.a. prompt-tdd) is the prompt-eval harness
that puts codescout's own skills/prompts under TDD against headless `claude -p` sessions.
Trackers of note there: `skill-eval-log`, `skill-eval-playbook`, and
`prompt-tdd-harness-backlog` (harness gaps the evals surfaced — several are
codescout-adjacent, e.g. the `AnthropicMcpRegistry` setup-file parity fix).

**Before running an eval, read `prompt-engineering:docs/trackers/prompt-tdd-operating-guide.md`.** It is
a ledger of ways the harness does what you asked in a way that reads as something else —
and the failure mode is a *number*, not an error, so nothing stops you publishing it. The
two that have bitten hardest: `Summary: 1/1 passed` is the **scenario** count (a `runs: 10`
arm reporting `0/1` has not failed ten times), and a checker missing its **exec bit**
reports as a clean `0/N`, character-identical to a genuine floor — which nearly published a
fabricated ceiling in hamsa A-25.

Don't hand-roll the scoring. `prompt-engineering:scripts/run_arms.py --config <cfg> --all`
runs every arm and prints rate + failure classes + distinct-answer count;
`prompt-engineering:scripts/score_arm.py <checker> <log>…` re-scores logs you already have. Checkers write each run to `$PROMPT_TDD_RUN_LOG`
when set — six lines, and it is what makes failures spot-readable, which every
pre-registration in `docs/trackers/prompt-hamsa-audit-log.md` requires before a count is
believed.
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
# 1. Add the structured entry — the server assigns the next T-N id
artifact(action="append_entry", id="f2ecdd76a6189efb",
  entry_collection="observations", id_prefix="T",
  entry={tool:"...", verdict:"...", ...})

# 2. Add analysis prose to the body (managed file — edit_markdown is refused)
artifact(action="update", id="f2ecdd76a6189efb", patch={body_edits: [{
  heading: "## Prompt improvement candidates", action: "insert_before",
  content: "### T-NNN — <title>\n..."}]})
```

**To change an existing row** (fix a verdict, flip a status):
```
artifact(action="update_entry", id="f2ecdd76a6189efb",
  entry_collection="observations", entry_id="T-17", fields={verdict:"wrong-tool"})
```

> ⚠️ Never hand-build the array — `artifact_augment(merge=true, params={observations:
> [...everything...]})` **replaces** the collection rather than merging into it, and the
> catalog is not in git. That call took this queue from 19 entries to 1 on 2026-08-16.
> `append_entry` / `update_entry` exist so it is never needed.

**User — browse:** open `docs/trackers/tool-usage-patterns.md`. The params table is **not in
the file** — a stored `render_template` is projected into `librarian(action="context")` only,
and no path writes it to disk (verified 2026-08-17: `render_params` has two callers;
`context.rs` renders into the context bundle, and `legibility_scan` writes a body with its
own compiled-in template for one other tracker). What the file holds is prose plus one
`### T-N` heading per entry. For the structured rows, query them:
`artifact(action="get", id="f2ecdd76a6189efb", entry_filter={"status": {"eq": "open"}})`.
Prompt improvement candidates are at the bottom —
these are the direct inputs to `src/prompts/source.md` (the `server_instructions` surface slice) edits.

### SDD Rulings — `docs/trackers/sdd-ruling-log.md`

When a plan runs under `superpowers:subagent-driven-development`, the controller is told to
**rule rather than stall** — decide conflicts, plan defects and scope calls mid-run instead
of parking on a question. Those are real delegated judgements, and they live in the run's
progress ledger, which **the process deletes at finish**. This tracker is where they land
instead.

**Deliberately not a ledger** — no `entry_prefix`, no ids, nothing to cite. The unit is the
**ruling line**; the table is for mining across runs. See `docs/TAXONOMY.md` §
*Trackers that deliberately own no prefix*.

**Claude — append when:** you finish an SDD run, **before** the workspace-deletion step.
One row per decision a human might reasonably have made differently — the same test the SDD
skill applies to what must be surfaced at finish. Columns: `ruling` (the line, complete
enough to judge without the plan open), `class`, `cost if wrong`, `verdict`.

**`verdict` is the point.** `held` teaches little; `WRONG` next to its class and cost is the
signal. Update it whenever a later run proves one wrong. Promote a repeated pattern into the
file's *Lessons extracted so far* section, then into CLAUDE.md or a skill once it has
two or more confirming runs.

**Seeded 2026-08-27** by the `get-guide-section-grain` run: 31 rulings, **5 wrong**. The
lessons already promoted out of it — worth reading before your next multi-agent run:

- *"Already fails loudly" is a claim about a code path, not about a feature* (3 datapoints
  in one run — the same reasoning error made twice, plus a third instance found by review).
- *A plan's reference code is a sketch* — 5 of 10 tasks carried a defect inherited from the
  plan, none caught by its author. The `plan-mandated` review label is what surfaces them.
- *Vacuous assertions cluster* — 4 found, the fourth only because the final reviewer was told
  to hunt for one. Ask "what mutation would make this test fail?", never "does it pass?".
- *Demand a deliberate break* — an assertion added to close a missing-guard finding was
  itself unable to fire.
- *Correct code in the wrong tree defeats every gate* — a leaked write compiled and passed
  its tests; only `git status` on the other checkout could see it.

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
artifact(action="append_entry", id="<tracker artifact id>", id_prefix="F",
  anchor_heading="## Template for new entries",
  title="<one-line title>", body="**Observed:** ...\n\n**Got:** ...")
# One write: the server allocates the id, writes `## F-N — <title>` (the only
# heading shape link_scan accepts as a definition), records the high-water mark,
# and stamps **Valid:** dated <today> unless the body declares a class.
# THEN add the Index / Wins Index row, using the id the call returned — never
# before, because a pre-written row consumes the id it names
# (statement-validity-session-log:F-3). Do not hand-allocate by grepping: a
# max-id is a fact about an instant, and a peer session can take it first.
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

**Cite a fix by SHA *and* patch-id — the SHA alone is not durable.** Both promotion paths stay available (cherry-pick for single fixes, fast-forward for large cohorts), and neither needs checking before you cite: `experiments` is rebased after every ship, so a cherry-picked commit's original is orphaned and eventually garbage-collected, while `git show <sha> | git patch-id --stable` is a content hash of the diff that survives both. Record the pair once at fix time — no decision, no follow-up reconciliation.

Full release cycle, standard ship sequence, SHA + patch-id citation rule, chained-git state-check, and concurrent-work reset safety → **`docs/RELEASE.md`**. SHA-citation + cross-repo `<repo>:<sha>` prefix discipline → memory `gotchas`. Commit style → memory `conventions`.
## Design Principles

codescout's conventions and design principles live in memory (auto-listed at session start) — read them when writing codescout code:

- **`conventions`** — pre-commit gate, error handling (`RecoverableError` vs `anyhow::bail!`), no-echo writes (`json!("ok")`), the `call_content()` MCP entry point, progressive disclosure / two modes, **Agent-Agnostic Design**, testing patterns (three-query sandwich, `EnvGuard`), prompt-surface consistency, commit style.
- **`architecture`** — module map, key abstractions, data flow, the three prompt surfaces.

Before adding or modifying any tool, read `docs/PROGRESSIVE_DISCOVERABILITY.md` — and if the tool can return a **negative result** (`0 matches`, an empty list, `not found`), also `docs/adrs/2026-08-27-negative-results-name-their-scope.md`: name the scope you examined when the zero is suspicious, stay **silent** when it is trustworthy, and claim only what you can prove. Full error decision tree: `get_guide("error-handling")`. Test isolation: `docs/conventions/test-env-isolation.md`.
## Prompt Surface Consistency

Three prompt surfaces (`server_instructions` + `onboarding_prompt` slices of `src/prompts/source.md`, and `build_system_prompt_draft()` in `builders.rs`) must stay tool-name-consistent. Which surfaces exist, when to bump `ONBOARDING_VERSION`, the **1900-character** slice cap + shared-branch verify hazard, and the writing style guide → **`src/prompts/README.md`** (short version: memory `conventions`). Stale tool names are gated by `prompt_surfaces_reference_only_real_tools` (3 surfaces) and `claude_md_contains_no_deprecated_tool_names` (this file).
## Companion Plugin: codescout-companion

A companion Claude Code plugin (`../claude-plugins/codescout-companion/`) is **always active** here. The rule that bites mid-task: **native `Read`/`Grep`/`Glob`/`Edit`/`Write` on source files and all native `Bash` are hard-denied — use codescout's MCP tools** (`symbols`, `grep`, `edit_code`, `read_file`/`read_markdown`, `run_command`). Full hook inventory, cross-repo flow, and concurrent-multi-workspace rules → **`docs/architecture/companion-plugin.md`**.
## Language-Specific LSP Issues

See codescout memory `gotchas` (LSP section) for Kotlin multi-instance conflicts,
cold start behavior, circuit breaker, and LSP mux details.
## Docs

Files:

- **`docs/PROGRESSIVE_DISCOVERABILITY.md`** — Canonical guide for output sizing, overflow hints, and agent guidance patterns. **READ THIS before adding or modifying any tool.**
- `docs/manual/src/architecture.md` — Component details, tech stack, design principles. (There is no docs/ARCHITECTURE.md — deliberately deleted, and left un-code-spanned
  here so the doc-ref audit does not read a statement of absence as a citation — see `docs/superpowers/specs/2026-05-06-doc-audit-design.md` § 1a.)
- **[`docs/PROBES.md`](docs/PROBES.md)** — One-page index of every measurement instrument: standalone scripts, built-in `librarian` scans, skill-driven analyses. **Start here before answering a question with a number** — an instrument may already exist, and each row names the blind spot that would make you mis-trust its output.
- `docs/ROADMAP.md` — Quick status overview
- `CONTRIBUTING.md` — Contributor-facing setup + PR checklist
- `docs/RELEASE.md` — Release cycle, ship sequence, git-workflow safety
- `docs/architecture/companion-plugin.md` — codescout-companion hook inventory + cross-repo flow
- `src/prompts/README.md` — prompt-surface rules: surfaces, `ONBOARDING_VERSION`, 1900-**character** cap, style guide

Memories (Claude auto-loads these; listed for reference):

- `architecture` — 8-project workspace map, cross-project deps, CI/shared infra; per-project: module structure, key abstractions, data flows
- `conventions` — Commit style, branch strategy, error handling rules, pre-commit requirements; per-project patterns
- `development-commands` — Full command reference (cargo, scripts, release)
- `language-patterns` — Rust anti-patterns and idiomatic patterns
- `gotchas` — Cross-project path resolution pitfalls, symbols truncation, Kotlin LSP, embedding model restrictions, memory leak
- `domain-glossary`, `project-overview`, `system-prompt`, `onboarding` — project self-description
