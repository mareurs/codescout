# codescout

Rust MCP server giving LLMs IDE-grade code intelligence — symbol-level navigation, semantic search, git integration. Inspired by [Serena](https://github.com/oraios/serena).

You are a proficient Rust developer. You follow all known good/scalable patterns. You are honest and recognize your limits and your mistakes, you own them. If you are not sure, you always ask me for feedback.

## Development Commands

**Run `cargo fmt`, `cargo clippy --workspace --all-targets --features local-embed -- -D warnings`, `cargo test --workspace --no-default-features`, `cargo test --workspace` before completing any task.** **The lean lane runs THIRD and the default one LAST, and the order is load-bearing.** We share one `target/`, and `tests/cli_artifact.rs` resolves `target/debug/codescout` **by path at run time** — so the lean lane leaves a librarian-less binary there, and a concurrent default-features run then execs it and dies with `unrecognized subcommand 'artifact'` on 10 of 11 tests, reading exactly like a librarian feature-gating regression in whatever you just committed. It is not one (`docs/issues/2026-08-30-shared-target-dir-feature-clobber-reds-the-cli-tests.md`). Ending on the default-features lane rebuilds the binary correctly, so **following the gate no longer arms the trap for everyone else** — which the documented order did *by construction*, because the lean lane was terminal and you walk away from it. Measured 2026-08-30: after the lean lane `artifact --help` exits **2** — the subcommand is gone, not empty — and the default lane restores all **8** verbs — the block prints **nine** lines, because clap appends its own `help`, so count verbs rather than lines or you will re-derive this sentence as wrong. It is 8-or-nothing by construction, so a partial count is not a weaker version of this bug but a different one: the `#[cfg(feature = "librarian")]` sits on the whole `Commands::Artifact` variant (`src/main.rs:175`), and none of `Verb`'s eight variants is individually gated. Two caveats kept deliberately. This closes the **terminal state**, not the **window** — during the lean lane the binary really is librarian-less, and the reorder only bounds that by the rest of your own run while you are still present. And it is **not free**: 71.8s documented vs 80.4s reordered vs 79.6s for the rejected appended-`cargo build` fix, so both remedies cost ~8s and the reorder wins on structure rather than speed — it removes the proposition whose exit code could be misread, where an appended build step is one more thing that can silently not have run. The long clippy form is the gate, not garnish: bare `cargo clippy -- -D warnings` lints only the root package's **non-test** targets with default features, so it passes trees CI fails — `.github/workflows/ci.yml` runs both (`:50` and `:61`), and only the second reaches `#[test]` code and `codescout-embed`'s feature-gated `local` module. Measured 2026-08-27: ten task gates, ten task reviews and a whole-branch review all missed a `doc_lazy_continuation` lint sitting in a test's doc comment (`prompt-surface-measurement-session-log:F-45`). The lean `cargo test --workspace --no-default-features` is there for the same reason one command over: every other gate command runs **with** default features, so an unconditional module reaching a `#[cfg(feature = "librarian")]` one compiles clean locally and fails only in CI's `no-features` *test-matrix* lane — the slow 3-OS job, not the fast clippy job, which never runs that config at all. Four instances of that class in one day (2026-08-27); the one that shipped left `experiments` HEAD failing the lean build, so every session that merged inherited the break. **It is `test`, not `check`, and that upgrade is load-bearing.** A `check` — even with `--all-targets` — compiles the lean test targets and never runs them, so a lean-only *runtime* failure is invisible to it; the older form of this line omitted `--all-targets` as well, so it did not even compile them. Measured 2026-08-30: `2c6f2677` turned three pre-existing **ungated** tests red under `--no-default-features`, and the lane stayed red for over a day while every documented gate command ran green across at least three sessions. `cargo test --workspace --no-default-features` was the only command that saw it — 3219 passed / 3 failed, green again at `f3dbfdf4` (3360/0). A fourth instance of the class arrived inside that very fix: an `expect_err` that compiled clean by default and failed to compile **lean**, because `Arc<dyn CodeEmbedder>` is not `Debug`. It subsumes the `check` it replaces, so the gate is still four commands; ~20s incremental (the check was ~10s, the test phase measured 6.96s on top). `--workspace` matches what CI actually runs (`.github/workflows/ci.yml:174`). ****Both test lanes carry `--workspace`** — added 2026-08-30, and that was a swap, not a fifth command. (Named by what they *are*, not by position: an ordinal is a positional reference, correct for exactly one arrangement and silently wrong after any reorder — the same defect a bare SHA has, which is why this project pairs one with a patch-id. A paragraph that tells you its own order is load-bearing must not then refer to its commands by number.)** Bare `cargo test` builds only the **root package's** targets, so every test in a workspace member is invisible to it; this is not a `tests/`-directory quirk, an inline `#[cfg(test)]` module is equally unreachable. Measured that day: `codescout-embed` compiles **56** tests with `remote-embed` (52 executing — 4 `ollama_*` are `#[ignore]`d) and **19** without, so **37 were reached by neither test command**, bare `cargo test` building 0 of them and the lean lane building the member with `remote-embed` **off**. The 33 live guards among those included the regression tests for three bugs fixed that same day — one of them a crypto-provider install whose absence aborts the process for every external consumer of the crate, whose own pre-existing test could not fail because its siblings installed the provider first (`BL-66`, `BL-68`, `bug-fix-session-log:W-85`). CI's `default` matrix lane (`flags: ""`) always covered them, so this was never missing coverage — it was `W-81`'s axis, *how long until the check tells someone*, and the answer was "at push time". The swap is safe because `--workspace` **strictly subsumes** the bare form: verified by `-- --list` set-difference, **0** tests present in bare and absent from workspace (4924 → 4980). Cost **+2.6s** warm (23.5s → 26.1s, two runs each, stable), so the gate is still four commands; that `+2.6s` is the default lane **alone**, not the gate total — the whole gate measures 71.8s in the pre-reorder order and 80.4s reordered. On our stack the live-MCP release build is `cargo rb` (not `cargo build --release`); after it, run `/mcp` to reconnect. Full command reference (every crate + fixture, `cargo rb` vs lean build) → memory `development-commands`; the binary symlink gotcha → memory `gotchas` (MCP Binary Symlink).

## Testing Discipline — what a green suite is evidence for

The gate above tells you how to get green. This tells you what green is worth. Each law below
carries its own measurement and date, and there is deliberately **no count here** — a tally of
the section's own contents is a premise that every addition falsifies. This sentence read *"all
four"* for an hour after a fifth law landed, which is § *Observer Blindness*'s
premise-moved-conclusion-didn't firing inside the section that documents it. Removing the number
removes the class; correcting it would only have reset the clock.

**A test cannot detect a change its assertion is MONOTONE under.** Absence assertions
(`is_empty()`, `!exists()`) are monotone under **removal** — a dead mechanism produces exactly
the silence they assert. Existence assertions ("a region *containing* X is found") are
monotone under **widening** — over-reporting satisfies them. Both look like guards, and
neither fires in its own direction, so a property held by one of each is covered **zero**
times, not weakly. For each test ask which direction its assertion is monotone under, and
mutate the *other* way. Measured (`e6414362`): a locator widened to swallow its whole section
killed **none of six tests** — the silence test and the positive test were both blind, and
pairing them bought nothing.

**And a test cannot detect what its RECORDING filters out — which is the harder twin, because
the standard remedy is a no-op against it.** The law above is about *members*: a population
selected so no member can falsify. Its fix is to add one that could — the nine dead-code probes
that all had visible callers become informative the moment a tenth is drawn from the dispatch
side. This one is about *observations*: the refuting outcome leaves **no artifact**. Measured
2026-09-01 — a ledger was about to publish *"of the near-miss numbers, four were caught by
re-derivation and zero by inspection"*, and that zero is unfalsifiable by construction, because
a reader who doubts a figure and re-counts it produces nothing. The wrong number never ships,
so nothing is recorded, so the population contains only the cases where doubt failed to occur.
**"Widen the sample" fixes member-selection and is a no-op here at any corpus size** — a bigger
corpus of ledger entries still contains zero catches-by-reading, forever. That is what makes it
worse than a small sample: the reflex answer looks responsive and changes nothing.

The remedy is to instrument the **doubt**, not the correction: when a re-derivation *confirms*,
publish the confirmation. Nobody is inclined to, because there is no finding to report — which
is exactly why the negative case has no record. One null result was published that day only
because someone said so out loud, and it is a **denominator**, not a catch: a confirming
re-derivation never entered the wrong-number population and bears on the base rate rather than
on the hit rate. Absorbing it as a near-catch would make the population look self-correcting.

**Mutate once per guarded SITE, not once per feature.** A mutation run answers a question
about one *line*. Where a law is implemented at N call sites, one kill proves exactly one site
is guarded and says nothing about the other N−1. Measured: `artifact_augment` had two
shape-writing paths, and mutating each separately killed **different** tests, neither failing
under the other's mutation — so a single mutation would have supported "covered" with the
second site unguarded.

**Loudness is a property of a PATH, not of a failure.** An alarm nothing reaches is exactly as
informative as no alarm: `BL-66` *aborts the process* — maximally loud — and survived anyway,
because every in-tree construction site installs the provider first. When adding a guard,
alarm, error return or `panic!`, name the concrete caller that reaches it **and** the observer
who acts on what it emits. "An external consumer we do not have in-tree" is a legitimate
reason to keep one and is **not** coverage of our own risk — say so at the test, or the green
tick gets read as protection. The tell: ask what an observer would *see differently* if this
were broken right now; if the answer is "nothing", the guard is decoration however loudly it
is written.

**The law reaches past guards, to features.** `ListFunctions` and `ListDocs`
(`src/tools/ast.rs:10-11`) both `impl Tool` and are registered nowhere — `src/server.rs:326` is
where a tool joins the registry (`Arc::new(Grep)`), and neither name occurs in that file. Their
module holds **18 tests**, 12 of which call the two tools by name. No agent can reach any of it.
Keep `BL-66` above rather than swapping it out: that is the **alarm** shape, a `panic!` nothing
arrives at, and it is the harder half to see. This is the **feature** shape, and it is cleaner in
exactly one way — `BL-66` had an out-of-tree consumer to argue about, and these have none, so the
green tick protects precisely nothing. Note what the unit is: the defect is the *tests*, not the
tools. **Derive the count, don't cite it** — one population yielded **three** defensible numbers inside
an hour, and each is the right answer to a different question. **18** tests live in `mod tests`
(19 functions less the `project_ctx_with_file` fixture). **13** exercise the tools by name — 12
there, plus `tests/integration.rs::workflow_analyze_ast`. **17** guard unreachable code, because
the four formatter tests cover `format_list_functions` / `format_list_docs`, whose only
production caller is `ListFunctions::format_compact` at `src/tools/ast.rs:84`. A first pass
published **15**: 13 with two wrong inclusions — a JSON fixture containing the *string*
`"ListFunctions"`, and an e2e helper (`run_list_functions`, reached from `run_single`'s
`"list_functions"` arm) mistaken for a test. None of these is a mistake about the code; they are
four different units. **A count of a defect population must arrive with its unit or not at all**
— and note that 13, 15, 17 and 18 are near enough to each other that no reader would query any of
them, which is § *Observer Blindness*'s closing rule firing on the example added to illustrate it.

**Annotate a fixture's load-bearing detail, on the fixture line.** The assertion states what
must be true; nothing states which part of the *setup* is what makes the test able to tell.
Say what breaks if the detail goes — not in the test name, not in the assertion message, and
never as a bare "do not edit". A tidy-up that removes it leaves the test passing and no longer
discriminating, which no assertion can catch because that change is monotone too.

*(§ SDD Rulings' "vacuous assertions cluster" and "demand a deliberate break" are instances of
these, found before the general form was. Full evidence, and the two superseded formulations
that got there — including a "pair an absence test with a positive one" remedy that is
falsified above — are `reconnaissance-patterns` R-132 and R-133.)*
## Bug Tracking

**Per-file bug tracking lives in `docs/issues/`.** Every bug noticed during work gets its own file, copied from `docs/issues/_TEMPLATE.md`. Path, slug, the `status:` vocabulary (`open | investigating | fixed | mitigated | wontfix | zombie`), and the archive flow are documented in **`get_guide("tracker-conventions")` § Bug files**.

Four behaviors are load-bearing and easy to skip:

- **Declare the defect class** — every bug file carries exactly one reserved `cluster/<slug>` tag in frontmatter, from the closed set defined in [`docs/trackers/issue-clusters.md`](docs/trackers/issue-clusters.md) (`IC-N`, artifact `1b5a080fe2efcb6b`). That tag is what makes *"which architectural problem do these bugs share?"* a query instead of a re-reading, and what lets a class reach its promotion threshold (**≥3 instances spanning ≥2 subsystems**) rather than being re-noticed and forgotten — three open files today carry a sentence of the form *"this is the third instance of one mechanism"* and had nowhere to put the count. Slugs are claim-shaped (`blast-radius-exceeds-visibility`), never topic-shaped (`concurrency`, which spans three classes). **Write it through the catalog** — `artifact(action="update", id=…, patch={tags:[…]})` or `codescout artifact update <id> --tags …`. A direct frontmatter edit does **not** reach the catalog (BL-48), so the tag sits on disk while every `find` reports the bug unclassified.

- **Capture on notice** — add the bug file the moment a bug is noticed (wrong edits, corrupt output, silent failures, misleading errors from codescout's own MCP tools), not at task end.
- **Archive once the fix is verified on `experiments`** — gate green plus a regression test. Reaching `master` is **not** required; `experiments` is never deleted. Archive via `artifact(action="move", …)`, never a bare `git mv`.
- **Record the fix SHA *and* its patch-id — never a pending-master-SHA line.** `git show <sha> | git patch-id --stable`. The SHA is positional and dies when `experiments` is rebased (which happens after every ship); the patch-id is a content hash of the diff and survives rebase *and* cherry-pick. Record the pair once at fix time: there is no promotion path to check and nothing owed later. (Measured 2026-08-19: 10 of 63 archived bug files had already lost their SHA to a rebase; zero patch-id collisions across 3594 commits. Many archived files still carry the older master-SHA-owed form — stale instructions, not open debt; do not sweep them.) **A merge commit has no patch-id** — `git show <merge>` emits no diff, so the pipeline returns empty and exits `0`, giving you no error and no value. Cite the merged branch's constituent commits instead; never record an empty patch-id field. Full rule → `get_guide("tracker-conventions")` § *Bug files*.

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
## Observer Blindness — when care is the wrong instrument

Some defect classes are invisible to the party best placed to catch them **by construction**, and they return a **plausible answer rather than an error** — so nothing downstream fires either. For these, "be careful" is not a weak remedy, it is the **wrong instrument**. Measured 2026-08-30: four instances of one class in one evening across three sessions, and **every one was committed by an author actively writing about that class** — one reintroduced a bare integer in the commit fixing a bare integer, ten minutes after withdrawing an ordinal for identical reasons; another shipped an unanchored ordinal inside a commit whose whole argument was that unanchored positional references rot. Knowing the class prevented none of the four. A standing policy caught one.

**So when you meet one, do not resolve to check harder — name three things and build the third.** (a) Who structurally cannot see it, *and the reason*: "was careless" disqualifies it, "holds the parameter that would reveal it" qualifies it. (b) Who can — this predicts the **reviewer**, and it is one who does *not share the author's context* rather than a more careful one, which is why self-review is structurally unavailable here and peer review is a different instrument rather than a redundancy. (c) The check that runs **when nobody is worried**. Best shape is making the correct path end in a safe state so compliance leaves nothing armed (`73066479` — the lean lane runs third precisely so the gate cannot be *followed correctly* and still arm the next session); next best is an unconditional policy tied to a trigger that happens anyway (*verify before contradicting a peer*); and for any published claim, ship its **derivation** rather than its value, so a reader re-checks it instead of re-deriving it under a counting rule of their own choosing.

Record classes as `OB-N` in **[`docs/trackers/observer-blindness.md`](docs/trackers/observer-blindness.md)** (artifact `3922c2a0fd0dfcfc`) — admission tests, the field block, and the mining greps live in the file; the one-line index is `docs/TAXONOMY.md`. **An instance is a bug file, an `F-N` or an `R-N`; only the class is an `OB`.** A row reading `**Mechanism status:** none yet` is a design worklist item, and is exactly what `H-N` (hooks) and `I-N` (`docs/trackers/test-escape-hardening.md`) consume — that tracker reached the same conclusion from the cost side, *"lenses must move LEFT into standing mechanisms so they catch by default without a human remembering"*, and the two are complements rather than copies.

## Parsers Over a Namespace — owe an escape and a disambiguator

A parser that interprets every token in its namespace is correct on every input it *accepts*; the
defect is the input it makes **unrepresentable**. That is why ordinary testing does not reach this
class — you cannot write a test for a case you cannot express, so the suite exercises the inputs
the grammar admits and passes. Promoted 2026-08-31 from `issue-clusters:IC-6` at **27 instances
across five subsystems** (file-format navigation, markdown editing, the citation resolver, four
shell gates, symbol navigation) — the largest class in this corpus, and one that sat at n=2 until
the archive was counted.

**Two halves, and a parser owes both.** *No escape*: `---` read as frontmatter wherever it
appears; a nested triple-backtick fence closing an enclosing quadruple one; content whose first
line looks like a heading deleting the heading it was replacing; a documentation example of
citation syntax counted as a real citation. *No disambiguator*: two byte-identical headings, both
permanently unaddressable; two symbols sharing a `name_path`; three ledgers owning one prefix,
kept apart by zero-padding alone; a qualified citation truncated at 31 characters so two file
stems become one. They fail in opposite directions — the first **refuses** work you can describe,
the second silently does it to the **wrong target** — so answering one is not answering the other.

**The heredoc tell.** Four independent shell gates — IL-3's pipe limiter, the dangerous-command
gate, the source-file gate, and `run_command`'s pipe instrumentation — each separately decided a
heredoc body was command text, and each was fixed separately. A construct that exists *precisely*
to mean "this is data, not syntax" will be misread by every scanner in the process, on its own
schedule. Ask what your parser's heredoc is.

**Before shipping one, answer two questions in the code rather than in your head:** how does a
caller write this token literally, and what happens when two collide? *"It cannot happen"* is a
claim about today's corpus and decays with it — three ledgers sharing a prefix was impossible
until the third existed. Where no escape is affordable, say so **at the refusal site**: a
documented limitation and a silent reinterpretation cost a reader very different amounts. The
corpus states the point better than this section can — an entry id cannot be *mentioned* without
citing it, the only escape being a fenced block, so
`docs/issues/2026-08-31-an-entry-id-cannot-be-mentioned-without-citing-it.md` is this class
holding about the very ledger that records it.
## Design Principles

codescout's conventions and design principles live in memory (auto-listed at session start) — read them when writing codescout code:

- **`conventions`** — pre-commit gate, error handling (`RecoverableError` vs `anyhow::bail!`), no-echo writes (`json!("ok")`), the `call_content()` MCP entry point, progressive disclosure / two modes, **Agent-Agnostic Design**, testing patterns (three-query sandwich, `EnvGuard`), prompt-surface consistency, commit style.
- **`architecture`** — module map, key abstractions, data flow, the three prompt surfaces.

Before adding or modifying any tool, read `docs/PROGRESSIVE_DISCOVERABILITY.md` — and if the tool can return a **negative result** (`0 matches`, an empty list, `not found`), also `docs/adrs/2026-08-27-negative-results-name-their-scope.md`: name the scope you examined when the zero is suspicious, stay **silent** when it is trustworthy, and claim only what you can prove. Full error decision tree: `get_guide("error-handling")`. Test isolation: `docs/conventions/test-env-isolation.md`.
## Prompt Surface Consistency

Three prompt surfaces (`server_instructions` + `onboarding_prompt` slices of `src/prompts/source.md`, and `build_system_prompt_draft()` in `builders.rs`) must stay tool-name-consistent. Which surfaces exist, when to bump `ONBOARDING_VERSION`, the **1900-character** slice cap + shared-branch verify hazard, and the writing style guide → **`src/prompts/README.md`** (short version: memory `conventions`). Stale tool names are gated by `prompt_surfaces_reference_only_real_tools` (3 surfaces) and `claude_md_contains_no_deprecated_tool_names` (this file).
## Companion Plugin: codescout-companion

A companion Claude Code plugin (`../claude-plugins/codescout-companion/`) is **always active** here and its hooks fire on every call — you will see `[cs-hint]` advisories. **But do not read its redirects as hard denials, and never infer a tool's availability from this file — call the tool once instead.** Measured 2026-08-27 in the `~/.claude-sdd` profile: native `Bash`, `Read` and `Edit` all reach source files unblocked. Reading the hook source would tell you the opposite (`pre-tool-guard.mjs:177` is `enforce("This call is blocked…")`, and its own `cat *.rs` branch at `:165` let the call through anyway), so the source is not the ground truth here — the probe is. Enforcement is per-profile, and the guard has a documented stand-down at `BREAKER_THRESHOLD = 3`, so "hard-denied" was never true of runtime. `Grep`/`Glob` are a different case again: absent from the tool list entirely rather than denied.

**Shell — both `run_command` and native `Bash` are permitted right now, deliberately.** An eval comparing the two is in flight, so `security.shell_command_mode` is a live arm and not a verdict; don't flip it as a drive-by, ask. Two things `Bash` does not get: the IL-3 unbounded-pipe block (it masked a non-zero `cargo test` exit here) and the dangerous-command `@ack_*` gate. `usage.db` records only MCP calls, so `Bash` work is invisible to `/analyze-usage` and `docs/trackers/tool-usage-patterns.md`. Background and remedy for the env divergence that makes `cargo test` fail from `Bash` → memory `gotchas`.

Prefer codescout's MCP tools for source work regardless of what is permitted — `symbols`, `grep`, `edit_code`, `read_file`/`read_markdown` go through the LSP/AST index. That is a capability argument, not a permission one, and it survives whichever way the eval lands. Full hook inventory, cross-repo flow, and concurrent-multi-workspace rules → **`docs/architecture/companion-plugin.md`**.
## Language-Specific LSP Issues

See codescout memory `gotchas` (LSP section) for Kotlin multi-instance conflicts,
cold start behavior, circuit breaker, and LSP mux details.
## Docs

Files:

- **`docs/PROGRESSIVE_DISCOVERABILITY.md`** — Canonical guide for output sizing, overflow hints, and agent guidance patterns. **READ THIS before adding or modifying any tool.**
- `docs/manual/src/architecture.md` — Component details, tech stack, design principles.
- **[`docs/PROBES.md`](docs/PROBES.md)** — One-page index of every measurement instrument: standalone scripts, built-in `librarian` scans, skill-driven analyses. **Start here before answering a question with a number** — an instrument may already exist, and each row names the blind spot that would make you mis-trust its output.
- `docs/ROADMAP.md` — Quick status overview
- `CONTRIBUTING.md` — Contributor-facing setup + PR checklist
- `docs/RELEASE.md` — Release cycle, ship sequence, git-workflow safety
- **[`docs/conventions/cross-machine-catalog-resume.md`](docs/conventions/cross-machine-catalog-resume.md)** — **Run this after pulling onto a machine that has not been building codescout.** The catalog (`~/.local/share/librarian/catalog.db`) is machine-local and gitignored, so a clone always arrives missing three layers — semantic index, `cites` edges, and artifact augmentations — and **each is silent in a different way**: `artifact(get)` returns `augmentation: null` without comment; a missing edge is indistinguishable from an artifact that cites nothing. (The augmentation third is now partly automatic — since `f565504a` shape travels in a committed sidecar and `reindex` re-attaches a declared one, reporting `augmentations_restored`. What stays manual is any artifact whose shape was never exported; the page says which is which.) Nothing fails — you quietly get less. **And a fifth mode runs the other way: an artifact the other host archived arrives as a file rename, but `id = sha256(abs_path)` means your catalog keeps the pre-move row.** `doctor`'s `missing_file` is what names it — re-read that field *after* the pull, because the page's own triage calls `missing_file` mostly other-repo noise and that is true of the majority and wrong about you; `reindex` is the repair. Measured 2026-08-28 on a 437-commit pull: 21 of 23 memories invisible to `recall`, 697 of 1117 `cites` edges absent, 22 trackers' augmentations gone (including the three CLAUDE.md gives append/query recipes for). The page also carries the two measurement traps that pass produced, both of which return a plausible **number** rather than an error.
- `docs/architecture/companion-plugin.md` — codescout-companion hook inventory + cross-repo flow
- `src/prompts/README.md` — prompt-surface rules: surfaces, `ONBOARDING_VERSION`, 1900-**character** cap, style guide

Memories (Claude auto-loads these; listed for reference):

- `architecture` — 8-project workspace map, cross-project deps, CI/shared infra; per-project: module structure, key abstractions, data flows
- `conventions` — Commit style, branch strategy, error handling rules, pre-commit requirements; per-project patterns
- `development-commands` — Full command reference (cargo, scripts, release)
- `language-patterns` — Rust anti-patterns and idiomatic patterns
- `gotchas` — Cross-project path resolution pitfalls, symbols truncation, Kotlin LSP, embedding model restrictions, memory leak
- `domain-glossary`, `project-overview`, `system-prompt`, `onboarding` — project self-description
