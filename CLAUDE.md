# codescout

Rust MCP server giving LLMs IDE-grade code intelligence — symbol-level navigation, semantic search, git integration. Inspired by [Serena](https://github.com/oraios/serena).

You are a proficient Rust developer. You follow all known good/scalable patterns. You are honest and recognize your limits and your mistakes, you own them. If you are not sure, you always ask me for feedback.

## Development Commands

See codescout memory `development-commands` for the full command reference.

**Always run `cargo fmt`, `cargo clippy`, and `cargo test` before completing any task.**

**To test changes via the live MCP server, run `cargo build --release`, then restart the server with `/mcp`.** The MCP server launches `~/.cargo/bin/codescout` (see the `command` in `~/.claude/.claude.json`), which is a **symlink → `target/release/codescout`**, so a release build alone updates the live binary — no `cargo install` needed. Dev builds (`cargo test`/`cargo build`) are a separate artifact and are NOT picked up.

If the symlink is missing (e.g. after `cargo clean` removed `target/`, or a fresh checkout), recreate it once:

```bash
ln -sf "$(pwd)/target/release/codescout" ~/.cargo/bin/codescout
```

Without the symlink, `~/.cargo/bin/codescout` is a stale installed copy and `/mcp` reconnects keep loading old code even after a successful build. (Symlink, not hardlink: `cargo build` rename-replaces the file, so a hardlink would resolve to the old inode.)

## Bug Tracking

**Per-file bug tracking lives in `docs/issues/`.** Every bug noticed during work gets its own file, copied from `docs/issues/_TEMPLATE.md`.

- **Path:** `docs/issues/YYYY-MM-DD-<slug>.md` for active bugs; `docs/issues/archive/` only after the fix has shipped to `master` (verify via `git branch --contains <fix-sha>`).
- **Slug:** short kebab-case noun-phrase (3–6 words), e.g. `edit-code-insert-mid-function`, `reindex-cascade-delete-data-loss`.
- **Status field** in frontmatter: `open` | `investigating` | `fixed` | `mitigated` | `wontfix` | `zombie` (semantics in `_TEMPLATE.md`'s header comment). `zombie` = no longer observed, root cause unconfirmed; pair with `last_observed:` and a re-open trigger.
- **`closed:` date** in frontmatter alongside any of `fixed` / `mitigated` / `wontfix`.

**Trigger rules — open a bug file for ANY bug noticed during work:**
- ✓ User explicitly asks ("log this", "open a tracker")
- ✓ Bug blocking the current task (fix-now or parking-lot)
- ✓ Incidental bug we won't fix in the current session
- ✓ Just-fixed bug whose investigation is worth preserving
- ✓ Tool quirks / misbehaviors (formerly the BUG-XXX log, retired 2026-05-17 — archived at `docs/archive/old-trackers/TODO-tool-misbehaviors.md`)
- ✗ Pure typos / one-token corrections — commit message is enough
- ✗ Feature ideas / refactors — those go in `docs/trackers/` or `docs/plans/`
- ✗ Subjective dislikes that aren't bugs

**Capture discipline (while working):** add the file the moment the bug is noticed — don't wait until task end. Watch for wrong edits, corrupt output, silent failures, misleading errors from codescout's own MCP tools. Each bug file holds Symptom / Reproduction / Root cause / Evidence / Hypotheses tried / Fix / Tests added / Workarounds / Resume — see `docs/issues/_TEMPLATE.md`.

**Don't add to retired surfaces.** `docs/archive/old-trackers/TODO-tool-misbehaviors.md` and `docs/archive/old-trackers/bug-tracker.md` are historical reference only — do not append. Open a new `docs/issues/<date>-<slug>.md` instead.
## Session Intelligence Trackers

**One-page index of every ID prefix** (F-N / W-N / R-N / U-N / H-N / T-N / BUG) — file, scope, append tool, promotion path — lives in [`docs/TAXONOMY.md`](docs/TAXONOMY.md). Start there when you're not sure which tracker takes an observation.

### Querying active trackers (librarian)


The librarian indexes every `docs/trackers/*.md` file with `kind: tracker`,
and every `docs/issues/*.md` file with `kind: bug`. The canonical
"what's live right now" query — archived auto-hidden:

```
artifact(action="find", kind="tracker")
```

For bugs, swap the kind:

```
artifact(action="find", kind="bug", status="open")
```

Until 2026-05-18, bug files lacked `kind:` frontmatter and the default
classifier rule mapped `docs/issues/**/*.md → kind=tracker`, polluting
tracker queries. The migration added `kind: bug` to the bug template +
all 37 existing files. The classifier rule is kept as a defense-in-depth
fallback for any bug file that omits the field.

**Status vocabulary** (frontmatter `status:` field for trackers):

| Value | Meaning | Visibility |
|---|---|---|
| `active` | Living tracker, actively appended to | visible |
| `draft` | Scoped/watching, not yet active | visible |
| `archived` | Terminal — work-stream wrapped | **hidden by default** (`HIDDEN_STATUSES` in `find.rs`) |
| `superseded` | Replaced by a successor artifact | **hidden by default** |

`done`, `in-progress`, etc. are NOT special-cased — they appear as active.
When a tracker is wrapped, set `status: archived` AND `git mv` to
`docs/trackers/archive/`. The frontmatter status drives librarian visibility;
the path move keeps the filesystem clean.

**Frontmatter shape** (required for new trackers):

```yaml
---
kind: tracker
status: active           # or draft | archived | superseded
title: <human title>
owners: []
tags:
  - <topic>
---
```

The librarian re-allocates `id:` on next `librarian(action="reindex")` if omitted.

Two living trackers capture observations from real sessions. Keep them current — they feed
prompt improvements and skill refactors.

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

**This is a public repo.** Do not push incomplete or untested work.

### Branch Strategy

- **`master` is protected.** Only cherry-picked, thoroughly tested commits land here.
- **All experimental work goes on the `experiments` branch** (or a dedicated feature branch). Iterate freely there.
- **Cherry-pick to `master`** only after: all tests pass, clippy clean, manually verified via MCP (`cargo build --release` + `/mcp` restart).
- **`experiments` is never deleted.** After any merge to `master`, `experiments` continues from the same commit — no recreation, no force-reset.
- **Before any merge or cherry-pick to `master`**, invoke the Docs Lotus Frog (`/buddy:summon frog`) to: (1) audit experimental features eligible for graduation, and (2) identify documentation gaps in the commits being merged.
- Never commit directly to `master` for in-progress or exploratory work.

### Release Cycle

Full release checklist — run from `master`, never from `experiments` or feature branches.

```bash
# 1. Bump version in Cargo.toml
#    Edit version = "X.Y.Z" in Cargo.toml

# 2. Build release binary and verify
cargo build --release
cargo test
cargo clippy -- -D warnings

# 3. Commit the version bump
git add Cargo.toml Cargo.lock
git commit -m "chore: bump version to X.Y.Z"

# 4. Tag the release
git tag vX.Y.Z

# 5. Publish to crates.io
CARGO_REGISTRY_TOKEN=$(grep CARGO_REGISTRY_TOKEN .env | cut -d= -f2) cargo publish

# 6. Push commit + tag
git push
git push --tags

# 7. Create GitHub release with release notes
gh release create vX.Y.Z --title "vX.Y.Z" --notes "release notes here"

# 8. Rebase experiments on the new master
git checkout experiments && git rebase master
```

**Notes:**
- Token is stored in `.env` (gitignored): `CARGO_REGISTRY_TOKEN=...`
- Use semver: patch for bug fixes, minor for new features, major for breaking changes
- Release notes should list features, dep upgrades, and doc changes
- Always rebase `experiments` after the release push

### Standard Ship Sequence

When a bug fix or tested feature on `experiments` is ready to land in `master`:

```bash
# 1. Commit on experiments (tests passing, clippy clean)
git add <files> && git commit -m "..."

# 2. Cherry-pick to master and push
git checkout master
git cherry-pick <commit-sha>
git push

# 3. Rebase experiments back on master (drops the cherry-picked commit automatically)
git checkout experiments
git rebase master

# 4. Migrate closed bug files whose fix shipped (run AFTER the cherry-pick lands on master)
#    For each <fix-sha> just cherry-picked:
#    - find docs/issues/<date>-<slug>.md whose Fix section cites <fix-sha>
#    - confirm: git branch --contains <fix-sha>  → must show 'master'
#    - if status is `fixed` / `mitigated` / `wontfix`: git mv it to docs/issues/archive/
#    Skip files still `open` / `investigating` — they stay in docs/issues/ regardless.
#    Commit the moves separately: docs: archive bug files for fixes shipped in <date>

# 5. (Optional, recommended after large refactors or batched-bug sessions)
#    Verify doc refs still resolve — bug-file Resume sections cite paths that
#    src refactors may have moved (F-1 friction, multiple datapoints).
#    Run from any active project:
#      mcp call codescout librarian '{"action":"audit_doc_refs","emit_tracker":true}'
#    Inspect findings JSON. Per-finding actions:
#      - verdict=missing, severity=high → real drift; fix the doc OR archive the bug
#      - verdict=ambiguous_basename → doc cites a basename matching multiple files;
#                                      add a path prefix to disambiguate
#      - verdict=resolved_basename → audit auto-resolved by basename match; OK
#                                     (consider adding the prefix anyway for clarity)
#    The audit covers docs/**/*.md by default (which includes docs/issues/).
```

This is the default workflow for all completed work. The rebase step keeps `experiments`
clean — git detects the cherry-pick and skips the duplicate commit automatically. Step 4
keeps `docs/issues/` showing only bugs whose fix is unreleased — see `_TEMPLATE.md` rule
*"Archive moves happen after the fix has shipped to master, not when status flips to fixed."*
Step 5 is the drift-detection step — `audit_doc_refs` is the canonical lint for stale
path / link / line references across all markdown surfaces.


#### After cherry-pick: cite the master SHA, not the experiments-side original

When tracking a multi-fix shipping session (running tally in chat, notes in a tracker, F-N entries citing evidence), record the **master-side SHA** assigned by `cherry-pick` — not the original SHA on `experiments`. After the subsequent `git rebase master`, the experiments-side originals become orphans (rebase detects cherry-picks and drops them via `--reapply-cherry-picks` default-off). `git branch --contains <orphan-sha>` then returns empty for every fix, and the running tally fails the "are they all on master?" audit even though every fix shipped.

**Concrete:**

```bash
# 2. Cherry-pick — capture the new SHA, do not just use the original
master_sha=$(git rev-parse HEAD)   # immediately after `git cherry-pick`
echo "$master_sha"                  # record this in the tally, not the pre-cherry-pick SHA
```

Or, after the fact: `git log master --oneline --grep="<subject prefix>"` to recover the master SHA by commit message.

Lesson source: 2026-05-23 batched-bug session — 12 fixes shipped, running tally cited the 12 experiments-side SHAs, none survived the rebase. Recovery took a `git log --grep` sweep on master to rebuild the SHA mapping for the user's "are they all done?" check.

**Applies to every SHA-citing surface, not just chat:**

- **Tracker entries** — F-N / W-N / U-N / H-N / R-N — any `**Status:**`, `**Fix idea:**`, or evidence-citing line that names a SHA. After the cherry-pick lands on master, update the citation to the master SHA before committing the tracker entry.
- **`artifact_event` calls** — `anchor_commit` and `also_mutates` SHAs are written into the catalog DB and outlive rebases. Cite the master SHA.
- **`docs/issues/<bug>.md` Fix sections** — `_TEMPLATE.md` § "## Fix" mandates the master SHA. New bug files inherit this; older ones may still cite experiments-side SHAs — update opportunistically when touching the file.
- **ADRs / design docs** citing the implementation commit — same rule.

**Anti-pattern:** writing `Fixed in commit abc1234 on experiments` immediately after committing on experiments. After cherry-pick + rebase, that SHA orphans. Capture the master SHA AFTER the cherry-pick lands and cite that instead. If forensic context matters, prefix explicitly: `experiments-side abc1234, master-side def5678` — never let bare SHAs default to "whichever branch I happened to be on."

**Cross-repo callsites** — when a tracker entry in codescout cites a fix that landed in `codescout-companion` (or vice-versa), use the `<repo>:<sha>` prefix from the "Cross-Repo Commit References" section below: `codescout-companion:0b75991`. A bare SHA implies the current repo.
### Commit Discipline

- **Batch related changes** into a single well-tested commit rather than committing every incremental step.
- **Only commit when the full fix/feature is working** — all tests pass, clippy clean, manually verified if applicable.
- **Do not push after every commit.** Accumulate local commits during a work session; push once when the work is solid.
- When iterating on a fix, keep working locally until the fix is confirmed, then commit the final state — not every intermediate attempt.


### Chained Git Commands — End With a State-Check (added 2026-05-18)

When chaining 4+ git operations with `&&` (e.g. `checkout master && cherry-pick X && push && checkout experiments && rebase && push`), the output stream interleaves all the intermediate results — the final-state confirmation lines (the `..` push outputs) can scroll past mid-output and look like in-progress steps.

**Rule:** end any 4+ step git chain with:

```bash
git rev-parse master experiments origin/master origin/experiments
```

Four identical SHAs prove the ship completed; divergent SHAs catch a silent partial failure (e.g. push rejected, rebase paused on conflict you missed).

This is bookkeeping — does not change behavior — but it converts "scan the output stream for success" into "read four lines at the bottom." Encountered 2026-05-18 when a user re-asked "did we cherry-pick to master?" because the success line was buried mid-output.

### Concurrent-Work Rules (added 2026-05-17 after F-13 incident)

When working on a shared branch alongside another active agent or session:

- **Never `git reset` to a relative ref** (`HEAD~N`, `HEAD^`, `@{N}`). Relative refs evaluate at execution time, not observation time — the gap between `git log` (read) and `git reset` (write) is enough for another agent to move HEAD, and your reset will silently traverse their commit.
- **Always quote an explicit SHA** for destructive ops. Read `git reflog -N` in the *same command* as the reset; copy the target SHA from the reflog output.
- **Treat your last-observed HEAD as immediately stale.** If any time has elapsed since your last `git log` / `git status`, re-read in the same command as the destructive op.
- **Before any `git rebase`, `git reset`, `git push --force`, or `git commit --amend`** during concurrent work: scout `git reflog -10` first. If unexpected entries appear (commits you didn't author at the tip), pause and reconcile *before* the destructive op.

This rule comes from F-13: `git reset --soft HEAD~1` erased another session's T-13 commit because HEAD had moved between observation and action. Recovered via reflog-quoted SHA (W-7).

### Cross-Repo Commit References (added 2026-05-17 after F-4 incident)

When a tracker artifact stores commit SHAs as evidence (`evidence_commits`,
task `notes`, `anchor_commit`, etc.), default reading assumes the SHA
belongs to the **same repo the tracker lives in**. For cross-repo
references — common in this workspace, where work spans codescout,
codescout-companion, buddy, and claude-plugins — prefix the SHA with the
repo name:

```text
<repo>:<sha>
```

Example: `codescout-companion:0b75991`, `buddy:abc1234`. A bare SHA
implies the current repo. The convention is unenforced; readers
following citations must notice the prefix. Schema-level enforcement
(adding a `repo` field to `evidence_commits`) is deferred until a
third cross-repo confusion lands (currently 1 concrete: F-4 in
`docs/trackers/archive/artifact-code-linkage-session-log.md`).

When citing a cross-repo SHA in a goal-tracker's progress_log, also
include the repo name in the `note` body so readers don't have to
parse the SHA prefix to know which `git log` to consult.
## Design Principles

**Progressive Disclosure & Discoverability** — Every tool defaults to the most
compact useful representation. Details are available on demand via
`detail_level: "full"` + pagination. When results overflow, responses include
actionable hints and file distribution maps (`by_file`). See
`docs/PROGRESSIVE_DISCOVERABILITY.md` for the canonical patterns and
anti-patterns — **read it before adding or modifying any tool**.

**Token Efficiency** — The LLM's context window is a scarce resource. Tools
minimize output by default: names + locations in exploring mode, full bodies
only in focused mode. Overflow produces actionable guidance ("showing N of M,
narrow with..."), not truncated garbage.

**No Echo in Write Responses** — Mutation tools (`create_file`, `edit_file`,
`replace_symbol`, etc.) must never echo back what the LLM just sent. The caller
already knows the path, content, and size — reflecting them wastes tokens with
zero information gain. The only new information after a write is success/failure.
Return `json!("ok")` for writes; reserve richer responses for cases where the
tool discovers genuinely new information (e.g. LSP diagnostics after a write).

**Two Modes** — `Exploring` (default): compact, capped at 200 items. `Focused`:
full detail, paginated via offset/limit. Enforced via `OutputGuard`
(`src/tools/output.rs`), a project-wide pattern not per-tool logic.

**Tool Selection by Knowledge Level** — Know the name → LSP/AST tools
(`symbols`, `symbol_at`). Know the concept →
semantic search first, then drill down. Know nothing → `tree` +
`symbols` at top level, then semantic search.

**Agent-Agnostic Design** — Tool descriptions, error messages, and server
instructions are the primary interface for LLMs. They must feel natural for
Claude Code (our primary consumer) but work for any MCP client (Gemini CLI,
Cursor, custom agents). In particular:
- Error hints should name codescout tools (`replace_symbol`, `insert_code`),
  not host-specific tools (`Edit`, `Write`). The LLM should never be tempted to
  sidestep codescout by falling back to its host's native file editing.
- The companion plugin (`codescout-companion`) adds Claude Code–specific
  enforcement (PreToolUse hooks) but the server itself must be self-contained:
  its gate logic, error messages, and instructions should guide any LLM toward
  the right tool without relying on external hooks.
- **Project workflows, prompts, and standards live in the repo, not in
  `claude-plugins/`.** codescout is consumed by multiple agents (Claude Code,
  Copilot, Antigravity, etc.). The source of truth for any project artifact —
  research quality criteria, save workflows, tracker conventions, etc. — must
  be a repo file (`docs/...`, `CLAUDE.md`, etc.) any agent can read. Plugin
  content (skills, slash commands) is allowed *as a thin UX wrapper* over
  repo-resident content, never as the source of truth. When in doubt: would a
  Copilot user be locked out? If yes, move it to the repo.

## Testing Patterns

**Cache-invalidation tests use a three-query sandwich** — not two. The structure is:
1. Query → record baseline state
2. Mutate the underlying data (disk, cache, external system) without going through the normal notification path
3. Query again → assert result is **stale** (same as baseline) — this proves the bug exists
4. Trigger the invalidation (e.g. `did_change`, cache flush)
5. Query again → assert result is **fresh** (reflects the mutation)

A two-query test (baseline → post-invalidation) only confirms the happy path. The stale-assertion in step 3 is what makes it a *regression* test — it will fail if the underlying system ever changes to eagerly re-read on every query, alerting you that the invalidation logic has become wrong or unnecessary.

See `did_change_refreshes_stale_symbol_positions` in `src/lsp/client.rs` for the canonical example.

**Test helpers that build env-reading objects must isolate env per test.**
Any helper that constructs an `Agent` (or any object that resolves config
from process-global env like `LIBRARIAN_DB`, `LIBRARIAN_WORKSPACE`,
`LIBRARIAN_CWD`) must return an `EnvGuard` and the calling test must
carry `#[serial_test::serial]`. Exemplars: `EnvGuard` in
`src/librarian/mod.rs::tests` and `src/server.rs::guide_hint_tests`. See
[`docs/conventions/test-env-isolation.md`](docs/conventions/test-env-isolation.md)
for the full rule + diagnostic shape + known cross-module gap.

## Key Patterns

Load-bearing rules I keep getting wrong otherwise:

- `RecoverableError` for expected, input-driven failures → `isError: false` (sibling calls survive)
- `anyhow::bail!` for genuine tool failures → `isError: true` (fatal)
- Write tools return `json!("ok")` — never echo content back
- `call_content()` is the MCP entry point, NOT `call()` — it handles buffer routing

## Prompt Surface Consistency

The project has **three prompt surfaces** that reference tool names. Two are sliced out of a single editable file via `<!-- @surface NAME -->` markers; the third is code-generated:

- `src/prompts/source.md` — single editable doc, sliced at build time (`build.rs` + `src/prompts/source.rs::extract_surface`) into:
  - **`server_instructions` surface** — injected once at every MCP session start (not per-request)
  - **`onboarding_prompt` surface** — one-time onboarding when a project is first activated
- `build_system_prompt_draft()` in `src/prompts/builders.rs` — generated per-project

See `src/prompts/README.md` for the surface contract + editing rules.

**When tools get renamed/consolidated, all three need coordinated updates.** Files
closer to the change get updated; distant ones accumulate stale refs ("distance
from change" problem). The test
`server::tests::prompt_surfaces_reference_only_real_tools` catches stale
tool-name mentions across all three surfaces at build time — if it fails,
either fix the stale reference or (if the token is a non-tool identifier like
a param name) add it to the test's allowlist.

**Any change to tool behavior or signatures requires a prompt surface review.**
This includes: adding new tools, renaming tools, changing parameter semantics,
adding new error/fallback modes, or modifying response shapes. Ask yourself:
"Does the LLM need to know about this change to use the tool correctly?" If yes,
update all three surfaces in the same commit.

### Onboarding Version

Bump `ONBOARDING_VERSION` in `src/tools/onboarding.rs` when changing prompt surfaces
that produce the **stored per-project system prompt** — i.e. the `onboarding_prompt` surface
of `source.md` or `build_system_prompt_draft()` in `builders.rs`. The bump triggers automatic system prompt
regeneration for all projects onboarded with the previous version.

**Do NOT bump for `server_instructions` surface changes.** That surface is injected fresh at
every MCP session start (each `/mcp` connect re-reads the sliced text). There is no cached
copy — changes take effect immediately on the next connect without any version bump.

### Which surface needs a bump?

| Surface | How delivered | Bump needed? |
|---|---|---|
| `server_instructions` surface (slice of `source.md`) | Loaded fresh at every MCP session start | **No** — live on next connect |
| `onboarding_prompt` surface (slice of `source.md`) | Drives stored system prompt generation | **Yes** — cached per project |
| `build_system_prompt_draft()` in `builders.rs` | Same — generates stored system prompt | **Yes** — cached per project |

### Bump when

- Tool names change (rename, consolidate)
- Tool parameter semantics change in the `onboarding_prompt` surface of `source.md` or in `builders.rs`
- Onboarding prompt templates change in ways that affect the generated system prompt

### Do NOT bump for

- Any change to the `server_instructions` surface of `source.md` (no matter how significant)
- Bug fixes that don't change tool behavior
- Internal refactors
- Memory template changes (memories are re-read during refresh anyway)

**Style guide for prompt surface edits:**
see `src/prompts/README.md` for the 7 writing rules. Load that only when actually
editing a prompt surface — it's not needed otherwise.
### Verify the slice before committing (shared-branch hazard)

The `server_instructions` slice is under a hard **2200-byte cap**, enforced by
`prompts::redesign_invariants::source_md_under_cap`. The gate is load-bearing —
it catches over-cap edits that no manual review sees (Claude Code silently
truncates the MCP `initialize.instructions` field at ~2 KB, cutting *inside*
the slice rather than just the dynamic suffix).

- **Run `cargo test --lib prompt` before any prompt-surface edit is ready to
  commit.** If `source_md_under_cap` fails, do NOT raise the cap or bless the
  snapshot to match — move content to a `get_guide(topic)` and leave a pointer
  in the slice (`src/prompts/README.md` rule 8).
- **On a shared branch, re-measure the slice on *current* HEAD.** A concurrent
  commit can grow the slice under you. `git log --oneline -1` first, then
  re-check the byte count, before trusting any earlier measurement or running
  `UPDATE_PROMPT_SNAPSHOTS=1` — otherwise you bless the over-cap state into the
  fixture and ship a truncated slice.

Datapoints: the gate has fired twice on over-cap prompt edits — F-4 (2026-05-28)
and F-8/W-5 (2026-05-31) in `docs/trackers/prompt-guide-refactor-session-log.md`.
## Companion Plugin: codescout-companion

This project has a companion Claude Code plugin at **`../claude-plugins/codescout-companion/`** that is **always active** when working on codescout. You must be aware of it.

**What it does:**
- `SessionStart` hook (`hooks/session-start.sh`) — injects tool guidance + memory hints into every session
- `SubagentStart` hook (`hooks/subagent-guidance.sh`) — same for all subagents
- `PreToolUse` hook on `Grep|Glob|Read|Bash|Edit|Write` (`hooks/pre-tool-guard.sh`) — **hard-denies (`permissionDecision: deny`) native Read/Grep/Glob/Edit/Write on source files and native Bash**, redirecting to codescout MCP tools

**Full hook inventory** (per `hooks/hooks.json`) — beyond the three above, the plugin wires:

*PreToolUse (guards — hard `permissionDecision: deny`):*
- `mcp__codescout__(edit_code|edit_file|edit_markdown|create_file)` → `worktree-write-guard.sh` — blocks codescout write tools when in a git worktree until `workspace(activate)` has run (clears the `.cs-worktree-pending` marker).
- `Bash` → `git-worktree-guard.sh` — denies worktree-ambiguous destructive git verbs from Bash; requires `git -C <path>` (single-worktree repos carved out).
- `mcp__.*__read_file` → `il4-deny-hook.sh` — IL4: hard-denies `read_file` on `.md` paths, redirecting to `read_markdown`.

*PreToolUse (advisory — `exit 0` + injected hint):*
- `mcp__.*__run_command` → `il3-warn-hook.sh` — IL3: warns (does not block) when piping unbounded `run_command` output to a log-trimmer; points at the `@cmd_*` buffer. (`il3-deny-hook.sh` exists on disk but is **not** registered — IL3 is warn-only.)
- `Task` → `pre-task-hint.sh` — on the first subagent dispatch of a session, points at the `reconnaissance` skill.
- `mcp__codescout__edit_code` → `pre-edit-hint.sh` — on the first shape-changing edit of a session, points at recon-for-shape-changes.

*PostToolUse (state sync):*
- `EnterWorktree` → `worktree-activate.sh` — injects workspace guidance, drops the `.cs-worktree-pending` write-block marker, symlinks `.codescout/` into the worktree.
- `mcp__.*__workspace` → `cs-activate-project.sh` — records the declared workspace (statusline) and removes `.cs-worktree-pending` (unblocks write tools).

*Stop:*
- `goal-stop-hook.sh` — queries codescout goal-tracker artifacts at turn end and surfaces refresh-staleness in the stop reason; fail-open; disable via `.claude/codescout-companion.json {"goal_stop_hook": false}`.

**Critical implication for working on this codebase:**
The `PreToolUse` hook will **block** any attempt to use the native `Read`, `Grep`, or `Glob` tools on source code files (`.rs`, `.ts`, `.py`, etc). You will see `PreToolUse:Read hook error` if you try.

**You MUST use codescout's own MCP tools to read source code:**
- `mcp__codescout__symbols(path)` — see all symbols in a file/dir
- `mcp__codescout__symbols(name=..., include_body=true)` — read a function body
- `mcp__codescout__search_pattern(pattern)` — regex search
- `mcp__codescout__semantic_search(query)` — concept-level search
- `mcp__codescout__read_file(path)` — for non-source files (markdown, toml, json)

**Cross-repo work (companion: hardened 2026-05-21):**
The Bash branch of `pre-tool-guard.sh` no longer allows a `cd`-escape. **All native `Bash` is hard-denied and redirected to `run_command`**, whose cwd is sandboxed to the active project. For a sibling repo's git, run from the project root via `run_command(command="git -C /abs/path <subcommand>")` — no `cd` needed. For non-git work in a sibling (or out-of-shape commands like `pushd` / `bash -c '...'`), switch the codescout workspace explicitly:

```
workspace(action="activate", path="/path/to/sibling", read_only=false)
# ...do the work...
workspace(action="activate", path="/home/marius/work/claude/code-explorer", read_only=false)
```

Per Iron Law 4, restore the original workspace before turn end. The MCP server is shared state — leaving it pointed at a sibling project pollutes the next session. Bug history at `docs/issues/2026-05-20-cross-repo-git-ops-friction.md`.

**Configuration:**
- Auto-detects codescout from `.mcp.json` or `~/.claude/settings.json`
- Can be overridden via `.claude/codescout-companion.json`
- `block_reads: false` in that config to disable blocking (dev/debug use)

### Concurrent multi-workspace: one server, one active project

codescout's active project is **process-global** — one slot per server process. Parallel
subagents within a single session share that one server, so if they `workspace(activate)`
*different* paths concurrently it is **last-writer-wins**: a subagent can silently read
another's worktree. The project `name` is identical across worktrees of one repo, so only the
full `project_root` reveals the swap.

**Rule:** for parallel multi-workspace work, use **separate Claude Code windows** (separate
processes → separate active-project slots → no race). Within a single session, do **not** have
concurrent subagents activate *different* workspaces — keep them in the parent's active
workspace, or serialize the activations. A `concurrent_activation_warning` now appears on
`activate` responses when a rapid foreign switch is detected (mitigation, not a fix).

Root-cause fix (per-request workspace pinning) is tracked in
`docs/plans/2026-05-30-per-request-workspace-pinning.md`; bug at
`docs/issues/2026-05-30-shared-server-global-active-project-race.md`. Separate worktrees of one
repo across separate processes are fine — the kotlin per-worktree LSP isolation bug is fixed
(`docs/issues/2026-05-30-cross-worktree-kotlin-jvm-shared-system-path.md`).
## Language-Specific LSP Issues

See codescout memory `gotchas` (LSP section) for Kotlin multi-instance conflicts,
cold start behavior, circuit breaker, and LSP mux details.

**Tracking:** `docs/issues/2026-03-24-kotlin-lsp-concurrent-instances.md`

## Docs

Files:

- **`docs/PROGRESSIVE_DISCOVERABILITY.md`** — Canonical guide for output sizing, overflow hints, and agent guidance patterns. **READ THIS before adding or modifying any tool.**
- `docs/ARCHITECTURE.md` — Component details, tech stack, design principles
- `docs/ROADMAP.md` — Quick status overview
- `CONTRIBUTING.md` — Contributor-facing setup + PR checklist

Memories (Claude auto-loads these; listed for reference):

- `architecture` — 8-project workspace map, cross-project deps, CI/shared infra; per-project: module structure, key abstractions, data flows
- `conventions` — Commit style, branch strategy, error handling rules, pre-commit requirements; per-project patterns
- `development-commands` — Full command reference (cargo, scripts, release)
- `language-patterns` — Rust anti-patterns and idiomatic patterns
- `gotchas` — Cross-project path resolution pitfalls, symbols truncation, Kotlin LSP, embedding model restrictions, memory leak
- `domain-glossary`, `project-overview`, `system-prompt`, `onboarding` — project self-description
