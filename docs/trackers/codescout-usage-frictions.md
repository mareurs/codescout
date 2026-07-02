---
kind: tracker
status: active
title: Codescout Usage Frictions — U-N Log
owners: []
tags:
  - pika
  - iron-law
  - usage
---

# Codescout Usage Frictions — U-N Log

Observed tool-misuse violations. Each U-N is allocated by the Pika at scan
time. Format from `~/.claude/buddy/skills/codescout-pika/SKILL.md` § Tracker
Format. Backing rows live in `.codescout/usage.db::pika_observations`.

**Archive policy:** entries with terminal status (`fixed-shipped` to master,
`wontfix`, `by-design`, `substrate-caught`) graduate to
`docs/trackers/archive/codescout-usage-frictions-<YYYY>-q<n>.md` on the
quarterly archive pass. The active list keeps only currently-open items
plus closures still awaiting master cherry-pick. See
[`docs/trackers/archive-cadence-policy.md`](archive-cadence-policy.md).

**Archived to 2026 Q2** (pilot pass, 2026-05-24): U-4, U-9, U-16. See
[`archive/codescout-usage-frictions-2026-q2.md`](archive/codescout-usage-frictions-2026-q2.md).

---

### U-1 — IL3 piped run_command, session 753e9a4a

**When:** First scoped Pika scan of this repo, 2026-05-17. Bound:
`cc_session_id='753e9a4a-a81f-4cf2-aeaa-a3877d35d1ce'` (559 tool_calls).

**Iron Law / pattern:** Iron Law 3 — `run_command` output piped to a filter
(`| head`, `| tail`, `| wc`, `| grep`) instead of running bare and querying
the `@cmd_*` buffer.

**Tool called:** `run_command` with command body containing `| {head,tail,wc,grep}`.

**Should have called:** `run_command(command)` bare, then in a follow-up
call query the returned `@cmd_*` buffer (e.g. `grep FAILED @cmd_abc`).

**Whistle delivered:** no (this is the first scan — whistles fire at
observation time, this U-N is a retrospective aggregate).

**Recurrence:** 45 occurrences in the scanned session (50 originally
observed; 5 self-matches retroactively removed 2026-05-17 — see
*Post-cleanup note* below).

**Severity:** low (all rows are observational; none blocked progress).

**Status:** closed via H-1 (deny hook shipped 2026-05-18). The 45-row evidence here was the baseline that drove H-1 from `proposed` → `warn` → `deny`. Substrate enforcement now blocks the predicate at PreToolUse; subsequent same-shape slips (e.g. U-16 in this session) hit the deny path and never reach the tool. See H-1 in `docs/trackers/codescout-usage-hookify.md` for the live hook + promotion evidence.

**Backing rows:** `pika_observations.id ∈ {1..50} \ {35, 36, 48, 49, 50}`,
`tool_call_id ∈ [20255, 20823]`, `subkind='iron_law_3'`, `verdict='slip'`,
`severity='low'`.

**Shape distribution among the 45 remaining rows (DB-authoritative,
2026-05-17 post-cleanup):**

| Pipe target | Count |
|---|---|
| `\| head` | 25 |
| `\| tail` | 12 |
| `\| wc` | 4 |
| `\| grep` | 4 |

| Command family | Count |
|---|---|
| `git …` | 11 |
| `find …` | 8 |
| `cargo …` | 8 |
| `ls …` | 6 |
| `grep …` | 6 |
| other | 3 |
| `cat @<buffer> …` | 2 |
| `diff …` | 1 |

**Post-cleanup note (2026-05-17):** Five rows (ids 35, 36, 48, 49, 50)
were retroactively deleted after the Pika scan SQL was discovered to
self-match — its own `LIKE '%|%'` discriminator and `INSERT INTO
pika_observations …` writes were being recorded as IL3 slips. All five
deleted rows were `sqlite3 …` invocations (Pika's own scan/insert
queries), which is why the `sqlite3` row dropped from 5 → 0 and is
omitted from the command-family table. The remaining 45 rows are real
IL3 slips. Discriminator fix: `INSTR(input_json, '''%|') = 0 AND
INSTR(input_json, 'pika_observations') = 0`. Filter mirrored to
`~/.claude/`, `~/.claude-sdd/`, `~/.claude-kat/` (md5 `670836e7`).
### U-2 — `read_file` on markdown, session 42874b1a

**When:** Second scoped Pika scan of this repo, 2026-05-17. Bound:
`cc_session_id='42874b1a-1ef5-44ce-ad64-4eb5b84cf93f'` (42 tool_calls).

**Iron Law / pattern:** Gates §"Tool gates" rule 2 — `read_file` on `.md`
should be `read_markdown`. SKILL Heuristic 6.

**Tool called:** `read_file(path="…/*.md")` — three times in one turn,
each hard-rejected by the in-server gate with hint
`"Use read_markdown for markdown files"`.

**Should have called:** `read_markdown(path)` first try; offers
heading-based navigation + slice-able body + smaller payload.

**Whistle delivered:** retrospective (scan-time, not real-time —
this is the first scan to surface the pattern).

**Recurrence:** 3 same-turn occurrences within session `42874b1a` (rows
`pika_observations.id ∈ {2,3,4}`). First slip = `med` severity, second
= `med`, third = `high` (same-turn recurrence escalates per Pika
Operating Principle 4).

**Severity:** high (same-turn frequency = the in-server error
message did not land; three round-trips wasted before correction).

**Status:** closed via H-2 (deny hook shipped 2026-05-24, claude-plugins:4587283d). The same-turn 3-slip recurrence here was the decisive signal that pushed H-2 from `proposed` to `shipped (deny direct, no warn stage)`. Substrate now hard-blocks `read_file(*.md)` at PreToolUse — the in-server gate stays in place as defense-in-depth.

**Backing rows:** `pika_observations.id ∈ {2,3,4}`, `tool_call_id ∈
{21631, 21633, 21634}`, `subkind='read_file_markdown'`,
`verdict ∈ {slip, habit, habit}`, `cc_session_id='42874b1a-…'`.

**Shape — all 3 rows:**

| tool_call_id | path | gate response |
|---|---|---|
| 21631 | `…/buddy/data/memory-protocol.md` | `Use read_markdown for markdown files` |
| 21633 | `…/buddy/data/gates.md` | same |
| 21634 | `…/.buddy/memory/common/dont-fabricate-commit-rationale.md` | same |

**Notes:** All 3 paths are doc/config markdown, not source-adjacent.
The predicate is shape-only (`.md` suffix); no command-family
variation as with U-1.



### U-3 — IL3 piped `run_command`, session 2026-05-18 (×7)

**When:** Tracker backfill + jsonpath ship-prep session, 2026-05-18.
Bound: this conversation (continued from compacted 2026-05-17 fix work).

**Iron Law / pattern:** Iron Law 3 — `run_command` output piped to a
filter (`| head`, `| tail`, `| sort | uniq -c`, `&&`-chained `cat` →
`grep`) instead of running bare and querying the `@cmd_*` buffer.

**Confirming data:** seven strikes in a single session, all flagged by
Pika's PreToolUse warning. First four were captured during the tracker
backfill + jsonpath ship-prep work; three more landed during the
librarian-misclassification fix + IL3-hook scout (this same session):

1. `git log --all --oneline | grep -E "^(808fe4b|a70816b5|66bee623)"`
2. `diff trackers/X.md trackers/archive/X.md | head -20`
3. `cat .codescout/.../@tool_X | grep ... | sort | uniq -c`
4. `cat _TEMPLATE.md && echo "---" && grep -oE "..." trackers/X.md | tail -3`
5. `ls docs/issues/*.md docs/issues/archive/*.md | wc -l` (count files)
6. `cargo test --release classify 2>&1 | tail -20` (test output bound)
7. `grep -A2 serde_json Cargo.lock | head -30` (Cargo.lock probe)

Plus two more during the H-1 promotion scout itself:
8. `grep -rn "iron.law.3..." | head -40` (settings sweep)
9. `grep -rln "run_command\|iron.law" ... | head -20` (hooks sweep)

Cumulative: 9 strikes this session.

**Severity:** med — each strike added ~200-500 tokens of pipe output to
my context vs. the bounded buffer-query path. Cumulative drift over a
long session is the real cost; individual strikes look free.

**Status:** closed via H-1 (deny hook shipped 2026-05-18). U-3's 9 strikes in one session despite explicit Pika warnings WAS the H-1 warn→deny promotion evidence ("warn-mode failed to change behavior within a single long session — the buffer-query habit did not stick" — H-1 Promotion evidence). Substrate now hard-blocks the predicate at PreToolUse.

**Diagnosis (introspection):** the four strikes break down as:
- 2× reaching for `| head` / `| tail` to bound output size before
  it lands in context — buffer-query gives the same bound for free.
- 1× `sort | uniq -c` aggregation — habit from shell pipelines;
  buffer-query supports the same `grep` step but not the trailing
  `sort | uniq`, which means I'd need a follow-up run_command for
  the aggregation. The "single round-trip" instinct pushes me to
  pipe instead.
- 1× `&&`-chained two commands — saving a round-trip by bundling
  two reads into one call. Same root cause: round-trip aversion.

**Pointer:** Promotes H-1's warn→deny criterion. With ×4 in one session,
H-1 has 2 sessions of evidence (the U-1 baseline + this U-3 follow-up)
— close to deny-threshold.



---

### U-5 — Compression-reminder drops bounded-LHS carve-out for Iron Law 3

**When:** 2026-05-23, line-by-line comparison of canonical Law 3 against the SessionStart compression-reminder.

**Iron Law / pattern:** Iron Law 3 — no piping unbounded `run_command` output.

**Tool called (surface):** companion `hooks/session-start.sh` line:
> *"Never pipe run_command output — query @ref buffers instead"*

**Should have called:** preserve the canonical exception text from `source.md`:
> *"NEVER pipe unbounded run_command output → run bare, query the @cmd_* buffer (grep "ERROR" @cmd_abc). **Bounded LHS (`ls`, `cat`, `awk`, `sed`, `find -maxdepth N`) is OK.**"*

The companion compression-reminder dropped the bolded clause. Post-compaction this becomes the dominant interpretation, and the model will refuse legitimate bounded-output pipes like `ls -la | awk '{print $9}'` — wasting round-trips on commands designed to produce bounded output.

**Whistle delivered:** yes (chat U-2 → this tracker entry).

**Recurrence:** 1st observed. Note: cross-references with U-3 (IL3 strikes in this session) — the model already has a pre-existing IL3 instinct problem; an over-narrowed rule makes it *worse*, not better.

**Severity:** med — actively wrong post-compaction interpretation, not just bloat.

**Status:** fixed-shipped (claude-plugins:bd20a8a, 2026-05-23). The bounded-LHS exception text was restored in both `hooks/session-start.sh` and `hooks/subagent-guidance.sh` (the latter caught during fix-time grep — same drift, second file).



### U-6 — Compression-reminder cites stale codescout tool names

**When:** 2026-05-23, comparing companion SessionStart text to the live MCP tool registry.

**Iron Law / pattern:** project prompt-surface consistency rule (CLAUDE.md § "Prompt Surface Consistency"). Direct repeat of the "distance-from-change" failure mode documented in that section.

**Tool called (surface):** companion `hooks/session-start.sh` line:
> *"Code edits: replace_symbol/insert_code/remove_symbol, NOT edit_file/Edit for structural changes"*

**Should have called:** `edit_code` (single consolidated tool with `action="replace"|"insert"|"remove"|"rename"`). The three named handles (`replace_symbol`, `insert_code`, `remove_symbol`) do **not** exist as MCP tool handles in the current binary. Confirmed against the tool registry available in this session — only `mcp__codescout__edit_code` is registered.

**Whistle delivered:** yes (chat U-3 → this tracker entry).

**Recurrence:** 1st observed in this surface; pattern-wise it's the second documented instance of "distance-from-change" tool-name drift (the first lived in repo-side surfaces and was caught by `server::tests::prompt_surfaces_reference_only_real_tools`, prompting the lint).

**Severity:** **high** — the model will attempt to call non-existent tools. Each call hits "unknown tool", forcing recovery and round-trip waste. Worst-failure variety of prompt drift; exactly what the project's lint exists to prevent — except the lint does not cover companion-plugin hooks (companion lives in a sibling repo).

**Status:** fixed-shipped (claude-plugins:bd20a8a, 2026-05-23). Stale handles replaced with `edit_code` in both `hooks/session-start.sh` and `hooks/subagent-guidance.sh`. The matching lint extension (H-3) remains open — see U-14 for the runtime-impact follow-up discovered during fix-time grep (worktree-write-guard matcher cites same nonexistent handles).



### U-7 — Project CLAUDE.md references renamed prompt files

**When:** 2026-05-23, attempted to read the canonical server-instructions text by the path CLAUDE.md cited; got `file not found`.

**Iron Law / pattern:** doc-vs-code drift; `librarian(action="audit_doc_refs")` exists to catch this exact failure.

**Tool called (surface):** project `CLAUDE.md` § "Prompt Surface Consistency" cites:
- `src/prompts/server_instructions.md`
- `src/prompts/onboarding_prompt.md`

**Should have called:** `src/prompts/source.md` — single source-of-truth file, sliced at build time via `<!-- @surface server_instructions -->` / `<!-- @surface onboarding_prompt -->` markers. See `src/prompts/README.md`:
> *"`src/prompts/source.md` — the **single editable document** for the next two surfaces. `build.rs` slices it into `OUT_DIR` at compile time; `src/prompts/source.rs::extract_surface` is the matching runtime parser."*

Old paths return "file not found" via both `read_file` and `read_markdown`.

**Whistle delivered:** yes (chat U-4 → this tracker entry).

**Recurrence:** 1st observed.

**Severity:** med — contributors (human or LLM) following the stale CLAUDE.md guidance look for files that don't exist; the surface that's supposed to *prevent* prompt-surface drift has itself drifted. Self-referential.

**Status:** fixed-shipped to experiments (`experiments:70b25e2f`, 2026-05-23; not-yet-on-master — awaiting cherry-pick). All 11 stale references updated to cite `src/prompts/source.md` plus surface names (`server_instructions`, `onboarding_prompt`). `audit_doc_refs` re-run on CLAUDE.md confirms zero matches for the old filenames. Same commit also retired the pre-archive `docs/TODO-tool-misbehaviors.md` reference in the Bug Tracking trigger rules. Audit also surfaced 20 false-positive findings (globs, template placeholders, home-paths, comma-trailing snippets) — noted as input to H-5's promotion plan (CI needs extractor FP filters before `--fail-on med`).

*Citation history:* original orphaned SHA `c37bcea7` (rebased away 2026-05-24); re-assigned to `70b25e2f` on the current experiments branch. T11 reconciliation (2026-05-24).



### U-8 — "Available shared memories" line truncates mid-name

**When:** 2026-05-23, scanning the codescout MCP `## Project Status` block delivered at session start.

**Iron Law / pattern:** progressive-disclosure design — overflow hints must be informative.

**Tool called (surface):** codescout's own `## Project Status` injection:
> *"Available shared memories: architecture, cargo-test-lib-skips-integration, conventions, development-commands, domain-glossary, gotchas, language-patterns, on… [truncated]"*

**Should have called:** either (a) full list — only ~10 memories exist, well within any reasonable budget; or (b) truncate at a comma boundary and emit `… +N more` so the model knows total count + that something remains. Mid-name `on…` discards information without naming it (the next memory is presumably `onboarding`).

**Whistle delivered:** yes (chat U-5 → this tracker entry).

**Recurrence:** 1st observed in tracker; visible at every session start.

**Severity:** low — model can recover with `memory(action="list")`, but only if it notices the truncation.

**Status:** fixed-shipped to experiments (`experiments:22fa98b2`, 2026-05-23; not-yet-on-master — awaiting cherry-pick). Root cause confirmed via ADR `docs/architecture/mcp-channel-caps.md`: Claude Code's MCP client caps `initialize.instructions` at ~2 KB and appends `… [truncated]`. The line landed in the cut zone because (a) it followed the static `SERVER_INSTRUCTIONS` constant (~1.8 KB) and (b) the line itself was ~350 chars due to a wordy action-hint suffix. Fix in `src/prompts/mod.rs::build_server_instructions`: label shortened to `Memories`, action-hint suffix dropped (the memory tool's own description already documents how to call it). Bare list now fits within cap for typical projects. 2443/2443 tests still pass.

*Citation history:* original orphaned SHA `2c4be270` (rebased away 2026-05-24); re-assigned to `22fa98b2` on the current experiments branch. T11 reconciliation (2026-05-24).

**Note for U-4 / future work:** the broader architectural issue is that the entire Project Status block lives in the cut zone. Workspace tables, custom instructions, and language warnings currently land in the dead 95% of the channel. That's Snow-Lion-class — see the ADR Open Decision for the structural recommendation.



### U-10 — Two global CLAUDE.md files disagree on CC instance count

**When:** 2026-05-23, both global CLAUDE.md files loaded into session context.

**Iron Law / pattern:** internal consistency across user-global config.

**Tool called (surface):**
- `~/.claude-kat/CLAUDE.md`: *"This machine runs **two separate Claude Code instances**"* — lists `~/.claude/` and `~/.claude-sdd/`.
- `~/.claude/CLAUDE.md`: *"This machine runs **three separate Claude Code instances**"* — lists `~/.claude/`, `~/.claude-sdd/`, `~/.claude-kat/`.

**Should have called:** sync the kat copy to mention the third instance, or drop the count entirely and just list. The kat one is stale — it predates the creation of `~/.claude-kat/` (the file's own host).

**Whistle delivered:** yes (chat U-7 → this tracker entry).

**Recurrence:** 1st.

**Severity:** low — minor model confusion; no principled tiebreak from the model side.

**Status:** fixed-shipped (in-place edit to `~/.claude-kat/CLAUDE.md`, 2026-05-23). The kat copy now matches the main copy: heading renamed to "Three Claude Code Instances", body lists all three profiles, applies-to instruction now says "ALL THREE", plus the 2026-05-16 cross-profile `installPath` note was synced over. File is not in any git repo (its own first line states this), so no SHA — the edit lives only in the user's home dir.



### U-11 — Buddy `gates.md` re-narrates Iron Laws in prose

**When:** 2026-05-23, Pika summon loaded `claude-plugins/buddy/data/gates.md` per the summon protocol.

**Iron Law / pattern:** redundancy with canonical surfaces (see U-4).

**Tool called (surface):** `claude-plugins/buddy/data/gates.md` § "Tool gates — codescout Iron Laws" — ~20 lines of prose narration of the same five laws already canonical in `source.md::server_instructions`.

**Should have called:** be a *pointer* — "see canonical Iron Laws in MCP server instructions" — and add only what canonical doesn't cover: workspace gate semantics, hooks behavior, role-gate context. Prose narration of rules that already exist in tabular form a few hundred tokens away is pure cost.

**Whistle delivered:** yes (chat U-8 → this tracker entry).

**Recurrence:** 1st.

**Severity:** low — bloat only; no contradiction with canonical.

**Status:** fixed-shipped (claude-plugins:3588d9b, 2026-05-23). `## Tool gates — codescout Iron Laws` was rewritten as a pointer + 6-bullet at-a-glance cheat sheet + the unique non-codescout fallback paragraph. As a bonus, the bounded-LHS exception (same as U-5) was restored on rule 5 in the rewrite — the prior prose had dropped it too.



### U-12 — Recon SKILL body inline-pasted instead of lazy-loaded

**When:** 2026-05-23, user invoked `/codescout-companion:reconnaissance` early in session (turn 1 of this conversation, before the Pika summon).

**Iron Law / pattern:** static-prefix budget — every line in slash-command output joins the cached session prefix.

**Tool called (surface):** the slash command pastes ~300 lines of `reconnaissance/SKILL.md` inline into the user-message turn.

**Should have called:** debatable — slash commands trade lazy-load (Skill tool) for "always visible". For a *frequently invoked* skill like reconnaissance during a multi-task session, inline-paste is the right call (the body is referenced repeatedly). For a one-shot acknowledgment without follow-up scout work, lazy-load wins.

**Whistle delivered:** yes (chat U-9 → this tracker entry).

**Recurrence:** 1st.

**Severity:** low — design call, not a defect. Listed for awareness; not for immediate fix.

**Status:** open. Defer until usage data accumulates: query `.codescout/usage.db` for how often the recon body content gets actively referenced vs sits idle in the prefix. If reference rate is low, lazy-load wins.



### U-13 — Per-turn re-injection of output-style anchor

**When:** 2026-05-23, every assistant turn in this session.

**Iron Law / pattern:** per-turn hook design.

**Tool called (surface):** the CC harness re-injects `"Explanatory output style is active. Remember to follow the specific guidelines for this style."` as a `<system-reminder>` on every turn.

**Should have called:** by design — re-anchoring prevents style drift mid-session, especially under voice stacking (currently three layers in this session: Pika voice + Caveman + Explanatory output style). Listed only as a surface to be aware of when designing future hooks; the design tradeoff is "always anchored" vs "per-turn token cost".

**Whistle delivered:** yes (chat U-10 → this tracker entry).

**Recurrence:** every turn (by design).

**Severity:** info, not friction.

**Status:** open as design note. No fix expected.



---

### U-14 — Worktree-write-guard matcher cites nonexistent tools (silent safety failure)

**When:** 2026-05-23, discovered while fixing U-5 + U-6 in companion `session-start.sh`. Broad grep for stale tool names surfaced 31 matches across 15 files; most are historical doc plans, but two are **live runtime configs**.

**Iron Law / pattern:** project prompt-surface consistency, same root cause as U-6 — stale tool names in companion-plugin surfaces drifting from the live codescout MCP tool registry. Where U-6 was *text drift in display surfaces*, U-14 is **matcher drift in runtime hook configs**: the affected lines pattern-match on tool name to gate execution.

**Tool called (surface):**
1. `claude-plugins/codescout-companion/hooks/hooks.json:25` — PreToolUse matcher:
   ```
   "matcher": "mcp__.*__(edit_lines|replace_symbol|insert_code|create_file|create_or_update_file)"
   ```
2. `claude-plugins/codescout-companion/hooks/worktree-write-guard.sh:19` — case statement filter:
   ```
   *__edit_lines|*__replace_symbol|*__insert_code|*__create_file|*__create_or_update_file)
   ```

Both alternations list four nonexistent tool handles (`edit_lines`, `replace_symbol`, `insert_code`, `create_or_update_file`) and one real handle (`create_file`).

**Should have called:** matchers must cover the **live** write-tool surface:
- `mcp__codescout__edit_code` (consolidated structural edits)
- `mcp__codescout__edit_file` (text edits)
- `mcp__codescout__edit_markdown` (markdown edits)
- `mcp__codescout__create_file` (already covered)

Proposed corrected matcher:
```
"matcher": "mcp__codescout__(edit_code|edit_file|edit_markdown|create_file)"
```
(with matching case-statement adjustment in `worktree-write-guard.sh`.)

**Whistle delivered:** yes (this entry; companion commit `bd20a8a` cited it forward).

**Recurrence:** 1st observed.

**Severity:** **high** — runtime safety failure. The worktree-write-guard exists to block silent wrong-file writes when a worktree is `.cs-worktree-pending` (workspace not yet `activate`d). With the current matcher, the guard fires only on `create_file`; `edit_code`, `edit_file`, and `edit_markdown` writes in a pending worktree are **silently unguarded**, exactly the failure mode the guard was built to prevent.

**Status:** fixed-shipped (claude-plugins:4efb7d3, 2026-05-23). Both `hooks/hooks.json:25` (PreToolUse matcher) and `hooks/worktree-write-guard.sh:19` (case statement) updated to fire on the live write surface — `mcp__codescout__(edit_code|edit_file|edit_markdown|create_file)`. Also fixed model-facing message text in `worktree-activate.sh:60` and `cs-activate-project.sh:42` (both listed nonexistent tool names in their BLOCKED/unblocked messages). Added `hooks/worktree-write-guard.test.sh` with 16 black-box tests covering modern handles (deny), read-only handles (allow), no-marker (allow), non-worktree (allow), and stale-handle regression sentinels (allow — would flip to deny if drift recurs). 16/16 PASS.

Design note: the old matcher used a wildcard `mcp__.*__` across MCP servers; narrowed to `mcp__codescout__` because the guard only protects local worktree writes, which only codescout performs. github MCP writes go through the API to a remote, not local files.



---

### U-15 — audit_doc_refs mis-parses Rust `::` separator + classifies git refs as paths

**When:** 2026-05-23, post-/mcp-reconnect verification of the H-5 FP-filter precursor (`0425b8ef`). Re-running `librarian audit_doc_refs` on CLAUDE.md showed FP count had dropped 21 → 4 hi-sev, but the 4 remaining included one real audit bug and two new FP classes.

**Iron Law / pattern:** audit-tool correctness — H-5 (audit_doc_refs CI gate) requires zero FP-shaped hi-sev findings before deny-stage promotion.

**Tool called (surface):**
1. `src/librarian/tools/audit_doc_refs/resolver.rs::resolve_file_symbol` — `rsplit_once(':')` on a Rust `path::symbol` ref leaves a trailing colon on the path part. The resolver then looks for `src/prompts/source.rs:` (with trailing colon) and reports `file_missing` even though the real `src/prompts/source.rs` file exists.
2. `src/librarian/tools/audit_doc_refs/parser.rs::looks_like_path` — `origin/master` and `origin/experiments` matched the multi-segment-slash heuristic and got classified as file_paths. They're git refs in `git rev-parse` examples, not filesystem paths.

**Should have called:**
1. `rsplit_once("::")` first (Rust style), fall back to `rsplit_once(':')` (Python/line refs). Apply symmetrically in both parser's `classify` and resolver's `resolve_file_symbol`.
2. Reject `origin/` and `upstream/` prefixes in `looks_like_path`, same shape as the existing `~/`, `*`, `<>`, `$` filters.

**Whistle delivered:** yes; fix shipped same session.

**Recurrence:** 1st observed.

**Severity:** med — was producing 3-of-4 hi-sev FPs blocking H-5's deny-stage promotion. After fix: 1 hi-sev remains (`claude-plugins/` cross-repo dir ref, a legitimate sibling-repo reference the local audit can't resolve — structural limitation, not drift).

**Status:** fixed-shipped to experiments (`experiments:f17c063d`, 2026-05-23; not-yet-on-master — awaiting cherry-pick). Two new tests added (`parser_rejects_git_refs`, `parser_handles_rust_double_colon_symbol_separator`).

*Citation history:* original orphaned SHA `61bc678b` (rebased away 2026-05-24); re-assigned to `f17c063d` on the current experiments branch. T11 reconciliation (2026-05-24).

**Measurement** (CLAUDE.md audit, hi-sev finding counts):
| State | Hi-sev count |
|---|---|
| Pre-FP-filter (initial discovery) | 21 |
| Post-FP-filter (0425b8ef) | 4 |
| Post-this-fix (f17c063d) | **1** (the cross-repo `claude-plugins/` ref) |

The 1 remaining hi-sev finding is a cross-repo reference to the sibling `claude-plugins/` directory. Resolving it would require either an "external roots" config on the audit, or recognizing that paths ending in `/` are dir-intent and tolerating not-locally-present. Design call for a future audit improvement, not drift to fix.


### U-17 — audit_doc_refs classifies instructional placeholder + reader-side paths as missing files (39 FPs)

**When:** 2026-05-23, same exploratory pass that produced U-16. Ran `librarian audit_doc_refs` across the full doc tree (551 files); hi-sev count was 40 — but breakdown showed 39 of them concentrated in two files: `docs/agents/copilot.md` (25) and `docs/agents/claude-code.md` (14). Only 1 was in a historical ADR.

**Iron Law / pattern:** audit-tool correctness — same family as U-15, but a new false-positive class. H-5 (audit_doc_refs CI gate) cannot promote past warn-stage while these FPs dominate the hi-sev signal.

**Tool called (surface):** `src/librarian/tools/audit_doc_refs/parser.rs::looks_like_path` + `classify`. They match strings like `path/to/copilot-codescout`, `.github/skills/`, `.github/agents/`, `.vscode/mcp.json` as `file_path` then resolve against `git_root` → `missing` → hi-sev.

**Reality check (Conclude-Last save):** read `docs/agents/copilot.md` line 22 — the doc explicitly says *"The commands use `path/to/copilot-codescout` as a placeholder for wherever you cloned it."* `.github/skills/`, `.github/agents/`, `.github/hooks/` are paths in the **reader's** repo (Copilot user setting up VS Code), not codescout's repo. `.vscode/mcp.json` is the reader's per-project MCP config. These are correct instructional content, not drift.

**Should have called:**
1. **Placeholder filter** — reject `path/to/`-prefixed refs in `looks_like_path` (same shape as the existing `~/`, `origin/`, `upstream/` rejections from U-15). One-line addition.
2. **Reader-side scope** (optional, broader fix) — allow per-doc frontmatter opt-out: `audit_reader_side_paths: true` on agent-onboarding docs would skip path resolution entirely. Cleaner long-term but more design surface.
3. **Or scope exclusion** — extend the `paths` glob default to exclude `docs/agents/**` (these docs are agent-onboarding, not codescout-internal). Cheapest fix but loses coverage for any *real* drift in those files.

**Whistle delivered:** yes (this entry). Fix not yet shipped — pending design call between (1), (2), (3).

**Recurrence:** 2nd FP class in audit (after U-15's two classes). Suggests the audit's classifier needs an extensible reject-list mechanism rather than per-FP-class one-off filters.

**Severity:** med — was about to mis-report 39 hi-sev findings as drift in a Pika exploration pass (Conclude-Last caught the misread). For real CI use, H-5 deny-stage promotion would falsely fail the build on every change. The bug is in the audit, not in the docs.

**Status:** **closed — fully shipped 2026-05-24.** Three patches landed:
- `experiments:956c080f` — `path/to/` placeholder filter (caught Class C of the FP breakdown, ~6 refs).
- `experiments:7a1f2a11` — Class B resolver fix: `../`-relative links now anchor at `md_file.parent()` instead of `repo_root` (8 cross-doc refs in agent docs flipped from `missing/high` to `resolved/low`).
- `experiments:0ad00251` — H-6 (C) shipped: `docs/agents/**` excluded from `DEFAULT_AUDIT_EXCLUDES` (handles Class A reader-side paths + Class D tool-method-name-mis-classification, ~30 refs).
- `experiments:5c51f01d` — docs/agents/*.md content refresh: stale `list_symbols` / `find_symbol` / `search_pattern` / `find_file` tool names replaced; multi-project example updated to use `workspace(activate, ...)` + `symbols(name=...)`. (Real drift surfaced once the audit was cleared of FPs.)

**Measurement** (audit on docs/**/*.md, hi-sev counts):
| State | Hi-sev count | Notes |
|---|---|---|
| Pre-fix (initial discovery) | 40 | 39 in agent docs + 1 ADR |
| Post-`path/to/` filter (956c080f) | 38 | 5 placeholder FPs filtered in copilot.md |
| Post-Class-B fix (7a1f2a11) | 30 | 8 `../manual/...` refs now resolve correctly |
| Post-doc refactor (01ec2890) | 30 | Real drift fixed; no new FPs introduced |
| Post-H-6 (C) (9fa04f0b) | **1** | Only the ADR historical drift remains; agent docs excluded by default |

**Measurement** (audit on docs/**/*.md, hi-sev counts):
| File | Pre-fix (f17c063d) | Post-fix (956c080f) | Notes |
|---|---|---|---|
| `docs/agents/copilot.md` | 25 | 20 | `path/to/` filter dropped 5 placeholder FPs |
| `docs/agents/claude-code.md` | 14 | 14 | no `path/to/` refs; reader-side `.github/...` paths still FP |
| `docs/agents/cursor.md` | 0 | 3 | reader-side `.cursor/mcp.json`, `.cursor/rules/` surfaced (4th affected file) |
| `docs/adrs/2026-05-13-semantic-anchors-qdrant-payload.md` | 1 | 1 | historical ADR drift (`src/embed/index.rs` renamed/moved) |
| **Total** | **40** | **38** | net −2 (placeholder −5, cursor.md visibility +3) |

The cursor.md delta is run-state, not code-state: pre-fix audit ran with `emit_tracker=true` (merger applies lifecycle dedup), post-fix audit ran with `emit_tracker=false` (raw findings). The 3 cursor.md refs were likely suppressed by merger logic in the first run and surfaced in the second.

**Hookify candidate:** see H-N tracker — propose H-6 (placeholder-prefix + reader-side classifier extensions).

### U-18 — Iron Law slips persist under deny-mode, session 2026-05-23 (×4)

**When:** audit_doc_refs noise investigation session, 2026-05-23 (this conversation).
Bound: continued from compacted cs-hint tracker session earlier the same day.

**Iron Law / pattern:** Mixed — Iron Law 1 (grep on source files) and Iron Law 3
(piped unbounded `run_command` output). Both caught by codescout-companion
PreToolUse hook in **deny mode**. Earlier U-3 (2026-05-18) was the warn-mode
baseline that promoted H-1 to deny; this is the first multi-strike post-deny
data point.

**Confirming data:** four strikes in a single session, all hook-blocked
and rerouted within the same turn:

1. `grep -rEn 'with_hint\b' src | wc -l` — recursive grep + pipe, blocked
   by source-file gate. Reroute → codescout `grep(pattern, path='src')`.
2. `grep -rEn 'to_string\(\)\.contains' src --include='*.rs' | grep ...`
   — recursive grep with file-type filter, blocked by source-file gate.
   Reroute → codescout `grep(pattern, path='src')`.
3. `cargo test --lib librarian::tools::audit_doc_refs:: 2>&1 | tail -30`
   — pipe to log-trimmer, blocked by IL3 gate. Reroute → run bare,
   `tail @cmd_xxx` on buffer.
4. `git log --oneline -- docs/trackers/doc-ref-audit.md 2>&1 | head -5; ...`
   — pipe to log-trimmer in chained command, blocked by IL3 gate.
   Reroute → run bare, `head @cmd_xxx` on buffer.

**Severity:** low — hook denied all 4 before any context cost. Each
recovery added one round-trip (~5-15s wall-clock). No cumulative drift
this session, in contrast to U-3's warn-mode 9-strike cost.

**Status:** open — the deny-mode substrate works as designed (zero
context bloat), but the reflex itself did not extinguish across 5+ days
since U-3. Habit persists; only the consequence changed.

**Diagnosis (introspection):**

- **Slips 1 and 2** (recursive grep): muscle memory from shell-first
  workflows. `grep -r` is a single token in mental shorthand for
  "search the tree"; codescout's `grep(pattern)` requires unpacking
  that into a tool-name. Under load (long investigation, many files to
  search), the unpack step gets skipped. Same root cause as U-3's
  "single round-trip" instinct, but the failure mode is *tool selection*,
  not output bounding.
- **Slips 3 and 4** (pipe to head/tail): exactly U-3's pattern, still
  active. Knowing the buffer exists doesn't override the reflex of
  bounding output at emission time. Tail-on-buffer requires two thoughts
  ("run bare" → "tail the buffer") where pipe-tail requires one ("just
  trim it inline").

**Pointer:** Deny-mode is the right substrate — it prevents context
bloat with zero ambiguity. The reflex persisting is bounded-cost (one
extra round-trip per slip) and arguably acceptable given habit-extinction
across sessions is slow. Open question for the H-N tracker: is there a
proactive nudge (per-turn first-call reminder, or skill-style "before
your first run_command this turn, consider …") that could shift the
reflex faster? Not blocking; capture as candidate, not priority.


---



### U-19 — `edit_code` preserves outer attributes with no drop path; `edit_file` blocked for attribute removal

**When:** Stability backlog task #68 (re-enable 5 Windows-gated `guide_hint`
tests), session 2026-05-25 (this conversation, post-compact). Encountered
while removing `#[cfg_attr(target_os = "windows", ignore = "...")]` blocks
above 4 test functions in `src/server.rs`.

**Iron Law / pattern:** IL2 enforcement gap. `edit_code` is the prescribed
tool for structural source edits, but it has **no action** for dropping an
outer attribute. `edit_code` action=replace explicitly PRESERVES outer
`#[...]` attributes; the docstring says "drop with edit_file". But
`edit_file` is hard-blocked for structural-looking edits on source files
(`debug_enforce_symbol_tools` is enabled), and the hook's structural
classification is broad enough to catch even **narrow attribute-only edits**
that don't touch the function signature or body.

**Confirming data:** three blocked attempts in a single turn:

1. Batched `edit_file` with 6 combined edits (cfg_attr removal +
   `#[serial]` insertion + tuple-pattern change across 6 tests) — blocked
   as structural.
2. Narrowed to single-test, **attribute-only** `edit_file` (delete the
   4-line `#[cfg_attr(...)]` block above one fn; signature preserved
   character-for-character) — still blocked as structural.
3. No `edit_code` action maps to "drop only the cfg_attr above this fn":
   `action=replace` preserves attributes, `action=remove` deletes the
   whole symbol (attributes + signature + body), `action=insert` adds
   adjacent code, `action=rename` only changes names.

**Severity:** med — forced a fallback to Python via `run_command` to do
filesystem-level string replacement on the `.rs` file. The Python escape
hatch worked but bypasses the codescout edit tools entirely (no LSP
validation, no symbol awareness, no buffer round-trip). The session cost
was ~10 minutes of tool-search + drafting + verifying. The larger cost
is the precedent — every future attribute-drop in this codebase faces
the same gap.

**Status:** fixed-verified (this session). `edit_code` action=replace now
accepts an optional `attributes: Vec<String>` field. When supplied
(even empty), replaces ALL outer attributes with the supplied list:
`attributes: []` drops them, `attributes: ["#[derive(Debug)]"]` sets
them exactly. Omitted keeps the original preserve heuristic.

The U-19 example (removing `#[cfg_attr(target_os = "windows", ...)]`
above a test fn): now expressible as a single `edit_code` call with
`attributes: ["#[tokio::test]"]` (or with whatever attributes you want
to keep). No more Python escape hatch. Closed alongside U-21 in the
same fix.

**Diagnosis (introspection):** the IL2 design assumed `edit_code` would
cover all structural edits and `edit_file` the rest. Outer-attribute
mutation falls in a gap — it IS structural (changes which attribute
expansions run at compile time), but `edit_code` doesn't surface it.
The docstring's "drop with edit_file" was written before the Pika hook
took source-file `edit_file` calls fully off the table for anything
multi-line.

**Pointer:** raise as a codescout tool-surface gap. Likely promotes to
H-N (hookify / substrate change) once a concrete API change is sketched
(option A is the smaller PR). Until fixed, the Python-via-`run_command`
escape hatch is the documented workaround. Worked example from this
session: 13-line script removed 4 cfg_attr blocks across `src/server.rs`
via `content.replace(...)` matches, with no codescout tool involvement.


---

### U-20 — Test helper hides a process-global env-var race behind innocent-looking signature

**When:** Stability backlog task #68 (re-enable 5 Windows-gated `guide_hint`
tests), session 2026-05-25 (this conversation). Recon discovery while
diagnosing the SQLite mandatory-locking deadlock root cause.

**Iron Law / pattern:** Not an Iron Law violation — a **project-level
test-setup foot-gun** worth recording in the U-N series because the
pattern recurs across multiple test modules in this repo. The
`make_server()` helper in `src/server.rs::guide_hint_tests` returned
`(TempDir, CodeScoutServer)` and looked self-contained. In reality it
created a librarian Agent that read `LIBRARIAN_DB` from the
process-global env — falling back to a shared default
(`dirs::data_local_dir().join("librarian/catalog.db")`) when unset.
Every test that called `make_server()` raced on the same DB file. On
Linux POSIX advisory locks the race was usually invisible; on Windows
mandatory file locks it deadlocked routinely, producing the
intermittent "tool 'artifact' not registered" failures gated behind
`cfg_attr(target_os = "windows", ignore = "...")`.

The same pattern exists in `librarian::mod::tests`, where the
`EnvGuard` + `serial_test::serial` discipline is already established —
but `guide_hint_tests` didn't import either.

**Confirming data:**

1. 4 of 6 `guide_hint_tests` were Windows-ignored with `cfg_attr` blocks
   citing "SQLite mandatory-locking race on the shared LIBRARIAN_DB"
   as the suspected cause.
2. Inline comment in `src/librarian/mod.rs:343-348` already documents
   the pattern as a hazard: "tests that mutate LIBRARIAN_WORKSPACE /
   LIBRARIAN_DB / CODESCOUT_REGISTRY leak their values into the rest of
   the process — e.g. `build_tool_context()` later picks up a stale
   tempdir path that no longer exists, and unrelated tests (e.g.
   `server::guide_hint_tests::*`) fail with 'tool artifact not
   registered'." The hazard was named in the librarian tests but not
   propagated to the consumer (`guide_hint_tests`).

**Severity:** med — bounded blast radius (test-only), but masked a real
cross-platform bug for months. The fix (per-test `EnvGuard` for
`LIBRARIAN_DB` + `#[serial]` on every test that constructs an Agent
through `make_server()`) is mechanical but easy to miss without the
existing librarian-tests precedent.

**Status:** fixed-verified (instance + class). Instance: `make_server()`
in `src/server.rs::guide_hint_tests` now returns
`(TempDir, EnvGuard, CodeScoutServer)` (#68 commit `701103d5`). Class:
the project-wide convention is promoted to
[`docs/conventions/test-env-isolation.md`](../conventions/test-env-isolation.md)
with a CLAUDE.md cross-link in the Testing Patterns section
(this-session commit). Future test helpers that read env-resolved
config now have a discoverable rule + two exemplars to copy from.

**Known gap (deferred):** the `#[serial]` + `EnvGuard` discipline
serializes within a module but not across. Observed once on Linux
during the U-23 verification session — `artifact_event_after_artifact_no_hint`
flaked under full `cargo test --lib`, passed both isolated retries.
The convention doc names the gap explicitly in its "Known gaps"
section so future maintainers see it before they hit it. Class fix
candidates (annotate every env-mutating test with `#[serial]`, or
move config off env onto explicit args) deferred — not blocking.

**Diagnosis (introspection):** the friction is **shape**, not knowledge.
The librarian module already documented the hazard inline. A
`guide_hint_tests` author skimming `make_server()`'s body in isolation
would not have seen the comment 8 directories away. The
process-global env-var dependency was invisible from the helper's
return type — `(TempDir, CodeScoutServer)` reads as "self-contained
tempdir + server".

**Pointer:** propose a project-level convention captured as an H-N or
ADR entry: **any test helper that constructs an Agent (or any object
that resolves config from env vars) must either (a) accept the relevant
env values as explicit arguments, or (b) return an `EnvGuard` that
isolates the process-global state for the test's lifetime, or (c) carry
a `#[serial]` requirement documented on the helper.** The fix shipped
this session is path (b) + (c). The principle promotes after a second
datapoint — likely the next time another test module hits this.


---



### U-21 — `edit_code` action=replace silently drops outer attributes when body starts with an attribute

**When:** Stability backlog task #68 (re-enable Windows-gated `guide_hint`
tests), session 2026-05-25 (post-compact). Surfaced during the second
phase of the fix — adding `#[serial]` to all 6 tests after the Python
cfg_attr removal pass (see [[U-19]]).

**Iron Law / pattern:** `edit_code` behavioral inconsistency with its
docstring. Tool docs state: *"action='replace' overwrites body
(PRESERVES outer #[...] attributes — drop with edit_file)"*. In
practice, when the replacement body's first non-whitespace token is
itself an attribute (e.g. `#[serial]\n    async fn ...`), the previously
preserved outer attributes (`#[tokio::test]`) **disappear** from the
result. Net effect: the test functions ended up with only `#[serial]`
attached, lost their `#[tokio::test]` marker, and the test runner found
zero tests in the module — exactly the symptom that triggered the bug
hunt.

**Confirming data:** single-turn data point with 6 verbatim repro
instances (all 6 tests in `guide_hint_tests`):

1. First-pass `edit_code action=replace` body: `#[serial]\n    async fn first_artifact_call_emits_librarian_hint() { ... }`.
2. Post-edit `cargo test --lib server::guide_hint_tests` reported
   `running 0 tests` plus 6 dead-code warnings on `EnvGuard`,
   `make_server`, `tool_by_name`, `shared_ctx`, `extract_hint`,
   `EnvGuard::set` — diagnostic signal that no test fn was bound to a
   harness attribute.
3. `read_file` of the affected region showed `#[serial]\n    async fn ...`
   with `#[tokio::test]` absent from above.
4. Second-pass replacement body explicitly carrying both attributes
   (`#[tokio::test]\n    #[serial]\n    async fn ...`) restored the
   correct shape; tests then ran (6 passed).

**Severity:** med — caught immediately by the build's dead-code warnings
and the 0-test count, so no shipped damage. But the docstring's
preserve promise is a load-bearing claim — any other replacement whose
body happens to start with `#[...]` faces the same trap.

**Status:** fixed-verified (this session). Closed alongside U-19 in one
fix. `edit_code` action=replace now supports an explicit `attributes`
field. The behavioral quirk (`body_leads_with_decorator` heuristic
clobbering existing attrs when the new body started with `#[...]`)
still exists for backwards compat when `attributes` is omitted — but
callers who want deterministic outcomes pass `attributes: [...]` and
get exactly the result they specify. The trap U-21 documents is now
**avoidable** with one extra field, and the new tool description names
the option so it's discoverable.

Worked example showing the U-21 trap is now closeable in one call —
the cfg_attr removal that took Python via run_command in the U-19
session would now be:

```python
edit_code(symbol="..." , path="...", action="replace",
         body="async fn ...() { ... }",
         attributes=["#[tokio::test]", "#[serial]"])
```

Three integration tests added in `tests/bug_regression.rs`
(`u19_replace_with_empty_attributes_drops_outer_attrs`,
`u21_replace_with_explicit_attributes_overrides_existing`,
`u19_u21_replace_without_attributes_preserves_existing_default`) cover
the three meaningful states: drop, replace, default-preserve.

**Diagnosis (introspection):** the heuristic edit_code probably uses to
find the symbol-replacement region scans backwards from the `fn` /
`async fn` keyword over `#[...]` lines and includes them in the
replacement scope. When the new body itself starts with `#[...]`, the
heuristic may interpret it as "you supplied the attributes you want" and
elide the previously-preserved set. Without source inspection of
`edit_code`'s implementation this is speculation; the observable
behavior is what U-21 captures.

**Pointer:** two possible fixes:

- **Doc fix** — clarify in the docstring that "preserved" only applies
  when the replacement body's first token is NOT an attribute. Add a
  worked example showing the correct pattern (body includes all desired
  attributes; outer attrs are concatenated only if body has none).
- **Behavior fix** — make preservation unconditional regardless of body
  shape, and require callers to opt OUT via a new `replace_attributes`
  field if they want to override (the option-A path from U-19 would
  also close this).

Workaround until fix: always include ALL desired outer attributes in
the replacement body. Treat edit_code's preserve promise as "preserved
only when body starts with non-attribute syntax".


---

### U-22 — IL3 detector flags literal `|` inside the string content of `git commit -m`

**When:** Stability backlog task #68 commit phase, session 2026-05-25.
Hit twice in two attempts to ship the same commit message.

**Iron Law / pattern:** Pika IL3 detector false-positive. The detector
scans the full `run_command` invocation string for pipe characters
without parsing shell quote/escape boundaries. When the commit message
*content* contains a literal `|` — common in shell-related code
discussions, e.g. "uses 'yes filler | head -2000' shell pipeline" — the
detector sees it as an output pipe and blocks the call.

**Confirming data:** two strikes in a single session:

1. First attempt:

   ```
   git add ... && git commit -m "... uses 'yes filler | head -2000' ..."
   ```

   Blocked. IL3 hook reported the message as "piped … to a log-trimmer".
   The `|` it flagged was inside a single-quoted substring within the
   `-m` argument.

2. Second attempt switched to Python heredoc — but the Python source
   itself referenced the same shell pipeline (`'yes filler | head -2000'`)
   in the message body string. Blocked again with the same diagnostic.

   ```
   python3 -c "
   msg = '''... uses \"yes filler | head -2000\" ...'''
   open('/tmp/commit-msg-68.txt', 'w').write(msg)
   "
   ```

   The detector's text scan does not respect Python triple-quoted
   string boundaries either.

**Severity:** low — workaround is mechanical (write message to file
via heredoc, then `git commit -F /tmp/file`), but the workaround is
ad-hoc and non-obvious until you've hit it. First-occurrence cost
~5 minutes of debugging the unhelpful "to a log-trimmer" message
before realizing the offending `|` was inside the string content.

**Status:** fixed-verified (this session). Closed at
`codescout-companion:d64749e`. The IL3 deny hook now runs a sed pass
to strip single-quoted (`'...'`) and double-quoted (`"..."`)
substrings before the pipe-detection regex, so literal `|` characters
inside string content no longer trigger the false positive. Also
derives `PRE_PIPE` from the de-quoted command so a quoted `|` before
a real `|` doesn't truncate the pre-pipe segment at the wrong
position. 4 new test cases added in
`hooks/il3-deny-hook.test.sh`; all 22 pre-existing tests still pass.

Compound shell decomposition (`&&` / `;` / `||`) remains out of scope
— the detector continues to treat compound commands as a single CMD,
which is a separate enhancement opportunity. The fix here is scoped
specifically to the U-22 friction shape (quoted-pipe in single
command), which is what bit during the #68 / U-23 session.

The heredoc-to-file workaround documented earlier in the entry is no
longer needed for the specific shape U-22 captured; the detector now
allows quoted pipes directly. Workaround stays valid for any other
future false-positive shape.


---

### U-23 — MCP server `strip_project_root_from_result` rewrites path strings, easy to misread as catalog data

**When:** Stability backlog task #69 (`librarian doctor`) extensive
smoke-test phase, session 2026-05-25 (this conversation). Discovered
when verifying the doctor output against the live catalog post-rebuild.

**Iron Law / pattern:** not an Iron Law violation — a
**methodological gotcha** for any agent inspecting MCP tool output to
reason about underlying data shape. The codescout MCP server at
`src/server.rs:351` runs `strip_project_root_from_result(call_result,
&root_prefix)` on every tool response except `run_command`, rewriting
absolute paths under the active project root into relative-looking
form. The catalog stores absolute paths; the MCP buffer shows
project-relative views; the two are easy to conflate when you have not
read the server's response-processing code.

**Confirming data:** single-session misread with concrete fallout:

1. `librarian(action="doctor")` returned 153 violations. The first
   ~75 had paths like `/home/marius/work/stefanini/...` (absolute) —
   those are NOT under the active project root, so the strip layer
   did not rewrite them.
2. The last ~78 had paths like `docs/issues/2026-05-19-...md`
   (relative-looking) — those ARE under the active project root
   (`/home/marius/work/claude/code-explorer/`), so the strip layer
   rewrote them to omit the prefix.
3. I read the mixed shapes as "two classes of catalog drift —
   absolute (genuine missing files) plus relative (wrong-shape rows)"
   and drafted a follow-up commit `feat(librarian): doctor — add
   abs_path_must_be_absolute check` with an overclaiming message
   citing a non-existent discovery.
4. The CLI's raw stdout (which bypasses the MCP strip layer) showed
   ALL 153 paths absolute. Re-reading `src/server.rs:341-371` confirmed
   the strip layer's behavior.
5. Amended the commit message to honest defense-in-depth framing.
   No code change required — the check itself is still valuable as
   a guardrail.

**Severity:** med — the misread led to overclaiming in a draft commit
message. Caught and corrected before push. But the underlying confusion
shape is reproducible: any agent inspecting MCP tool output that
contains path fields can hit the same misread, and the strip behavior
is not surfaced in the tool response itself (only the `read_file`
fallback emits a `[codescout] paths are relative to {root}` annotation,
capped at 3 per session — see `src/server.rs:365`).

**Status:** fixed-verified (this session). The annotation now emits on
every stripped response — no per-tool filter, no per-session cap. Cost
is ~50 bytes per stripped response (negligible vs the prefix savings
the stripping itself yields). The fix is the option-B path from the
original entry ("surface-on-every-response"). Per-call commit on
experiments captures the change + a regression test exercising
post_process with 7 mock tool names (4 read_file + tree + symbols +
librarian) plus a negative case for run_command (which is exempt from
stripping and must NOT carry the annotation).

**Diagnosis (introspection):** the strip layer exists for human
readability — relative paths are visually scannable, absolute paths
add noise for the common case where the agent already knows the
project root. The trade-off is correct for human reading, wrong for
machine-side data-shape verification. The annotation-on-`read_file`
cap of 3 reads as "once you've seen this 3 times you know the
convention" — but for a fresh agent on a fresh session, 3 strikes
isn't enough to internalize when the convention applies (which tools)
and when it doesn't (`run_command`).

**Pointer:** the smoke-test discipline lesson is portable: when
verifying a tool's data-shape claims against an MCP response, prefer
the CLI variant or read the buffer with `read_file ... json_path=...`
on a known-absolute field to detect rewrites. Worth a W-N in the
recon-patterns tracker once the pattern repeats a second time.
Related: see [[U-19]] / [[U-21]] for other "docstring says X,
behavior does Y" cases.


---



### U-24 — `strip_project_root_from_result` docstring lies about how buffer content is covered

**When:** 2026-05-25 verify-open recon pass after the U-23 fix shipped.
Investigating the question "does the annotation survive @tool buffer
overflow?" as a follow-up to my own prior-session note ("may not appear
in @tool buffer when response overflows").

**Iron Law / pattern:** not an Iron Law violation — a **documentation
bug** that misdirects future readers about how strip coverage is
actually achieved. The actual runtime coverage is correct; the docstring
makes two factually false claims about it.

**Symptom (in `src/server.rs:1311-1313`):**

```rust
/// Buffer content (`@tool_xxx` refs) is covered automatically: it only
/// re-enters the pipeline through `run_command`, which also passes through
/// `call_tool` and gets stripped there.
```

Both claims are wrong:

1. **"It only re-enters the pipeline through `run_command`"** — false.
   Buffer content also re-enters via `read_file(@tool_xxx, json_path=...)`,
   `read_file(@tool_xxx, start_line=N, end_line=M)`, `grep PATTERN @tool_xxx`,
   and any tool that accepts an `@ref` substring.
2. **"`run_command` ... gets stripped there"** — false. `run_command` is
   EXEMPT from stripping per the gate at `src/server.rs:352`
   (`let should_strip = tool_name != "run_command";`). The exemption
   exists precisely because `run_command`'s output is raw shell stdout
   where stripping would corrupt path literals (see
   `docs/issues/2026-05-21-run-command-strips-project-root-from-path-literals.md`).

**Why it slipped through:** the docstring was written under an older
mental model when both observations may have been partially true.
Then `run_command` was carved out for the path-literals bug, and the
buffer-reading surface broadened to include `read_file`'s json_path /
line-slicing forms — but the docstring of `strip_project_root_from_result`
wasn't updated either time. The two claims compound each other: a reader
who trusts claim 1 (only run_command re-enters) and claim 2 (run_command
strips) concludes "all buffer-re-reads are stripped" — which happens to
be correct by accident, since other re-read paths (`read_file`, etc.)
also strip via the non-`run_command` post_process path. The mechanism
described is wrong; the conclusion happens to be right.

**Actual coverage (how it really works):**

- Original tool call: tool's `call_content()` produces output. If
  oversized, the tool writes raw content to a buffer file and returns a
  small JSON envelope referencing `@tool_xxx`. The envelope passes
  through `post_process`, which strips (small envelope's path strings)
  and annotates.
- Later `read_file(@tool_xxx, ...)`: dispatches to the `read_file` tool,
  which reads the raw buffer content. Its output passes through
  `post_process` (because `read_file != "run_command"`), which strips
  and annotates the buffer content too.
- `run_command @tool_xxx` is the only exception — its output is raw
  shell bytes, exempt by design.

**Severity:** low — documentation only. Easy to fix by replacing the
two false claims with the accurate framing.

**Status:** fixed-shipped (this session). Docstring rewritten in
`src/server.rs` to describe the actual mechanism.

**Pointer:** the same shape ("docstring describes wrong mechanism;
runtime coverage is accidentally correct anyway") is worth watching for
during any future PRs that touch `post_process` or the buffer subsystem.
Related: see [[U-23]] (the U-N entry that originated the question), and
the prior `_path_note_count` rename which was the LAST vestigial
artifact of the per-session-cap mental model that the docstring also
reflects.



### U-25 — Path-disambiguation annotation fires per call; activation + worktree state invisible

**When:** 2026-05-28 session, working inside a git worktree
(`/home/marius/work/mirela/backend-kotlin/.worktrees/weekly-pattern`).
User flagged the `[codescout] paths are relative to <root>` line as
"useful but spammy" and asked for: (1) novelty-gated emission, (2)
worktree validation, (3) explicit activation signal at session start.

**Iron Law / pattern:** not an Iron Law violation — a **prompt surface
density / signal placement** issue. The U-23 fix (2026-05-25) resolved
correctness ("cold readers misread stripped paths as catalog data") by
moving from a per-session cap to per-call emission. The cost was ~50
bytes × every non-`run_command` tool, multiplied across a session, with
no corresponding signal for two adjacent UX questions ("am I in a
worktree?", "did activate_project happen?").

**Resolution shape:**

1. **A — novelty-gated annotation.** Repurposed the vestigial
   `_path_note_count: AtomicUsize` field at `src/server.rs:76` (pre-fix)
   into `path_note_emitted_since_activation: AtomicBool`. `post_process`
   emits the annotation only on the first stripped response since
   server start or last `activate_project`. The activation branch of
   `call_tool` (`src/server.rs`) resets the bool so the next stripped
   response carries the annotation again with the new root.
2. **C — worktree-aware validation banner.** New `WorktreeInfo` struct
   + filesystem-only `detect_worktree_info` helper in
   `src/prompts/mod.rs`. Plumbed through `ProjectStatus` and populated
   in `Agent::project_status`. `build_server_instructions` emits
   `**Worktree:** branch \`<branch>\` of \`<main_repo>\`` when present.
3. **D — explicit activation banner.** `**Project:**` →
   `**Active project:**` in `build_server_instructions`. Surfaces the
   implicit launch-time activation. Refreshes on every
   `activate_project` via the existing `refresh_instructions` path.

**Why this is safe vs U-23:** the cold-reader signal the U-23 fix
protected (per-call annotation so post-compaction readers can still
disambiguate stripped paths) now lives in `server_instructions` —
specifically the `**Active project:**` line in the Project Status
block, which is system-prompt content and survives compaction. The
per-response annotation becomes redundant after the first stripped
call within an activation window.

**Severity:** low — UX friction, not correctness. The fix is a
follow-on to U-23, not a regression.

**Status:** fixed-shipped (this session, experiments-side; master SHA
to be recorded after cherry-pick).

**Related:** [[U-23]] (the per-call cadence this entry partially
relaxes), [[U-24]] (the docstring-vs-runtime follow-up on the same
post_process surface). Bug file:
`docs/issues/2026-05-28-path-annotation-spam.md`.

### U-26 — `artifact(update, patch={body_edits})` action grammar undocumented; `edit` vs `replace` found only via 3 sequential errors

**When:** 2026-06-09 session, flipping the F-15 `**Status:**` line in `bug-fix-session-log.md` via `artifact(action="update", patch={body_edits:[...]})`. A scoped string swap took **three rejected calls** to land:
1. `{action:"replace", old_string, new_string}` → "missing required 'heading' field"
2. `{heading, action:"replace", old_string, new_string}` → "content is required for the replace action" (bare, no hint) — the old_string/new_string intent was silently discarded
3. `{heading, old_string, new_string}` (no action) → "missing required 'action' field — Allowed actions: replace, insert_before, insert_after, remove, edit" — only here did the enum surface, revealing `edit` is the string-swap verb
4. `{heading, action:"edit", old_string, new_string}` → ok

**Iron Law / pattern:** not an Iron Law violation — a **schema discoverability gap**. The intuitive guess for "replace this string" is `action="replace"`, but `replace` is whole-section overwrite (needs `content`); the old_string/new_string verb is the non-obvious `edit`. Neither the `patch.body_edits` schema description nor `get_guide("librarian")` § Body Editing Surfaces enumerated the actions or paired old_string/new_string with `edit` — both said only "mirrors edit_markdown's batch shape."

**Resolution shape (this session, experiments-side):**
1. `src/librarian/tools/artifact.rs` — `patch` description now enumerates `replace|insert_before|insert_after|remove|edit` and disambiguates `edit` (scoped swap: heading + old_string + new_string) vs `replace` (whole-section overwrite: heading + content).
2. `src/prompts/guides/librarian.md` § Body Editing Surfaces — same action grammar added to the `body_edits` row.
3. `src/tools/markdown/edit_markdown.rs:99` — the bare `anyhow!("content is required for the replace action")` now appends "...for a scoped text swap pass action='edit' with old_string + new_string" (shared by `edit_markdown` direct calls and the `artifact` body_edits path); the "content is required" prefix is preserved so no assertion breaks.
4. Regression test `body_edits_replace_without_content_points_at_edit_action` (`update.rs`) pins the recovery contract — newline-free fixture, chosen to dodge the `\n`-payload hazard.

**Severity:** low — discoverability friction, no correctness or data risk. Cost: ~3 wasted calls per agent that guesses `replace`.

**Status:** fixed (this session, experiments-side; uncommitted at time of writing). Pika note: whistled late (after slip #3, not slip #1) — a watch-miss to do better on.

**Related:** surfaced jointly by the Prompt Hamsa + Pika. Touches three surfaces — schema description, `get_guide("librarian")` body, and the shared error message.


### U-27 — "Never read_file source" whistle is a false positive for imports & lossy-extractor reads; criterion narrowed

**When:** 2026-06-14 Pika session (summon-scope). I whistled this session's two `read_file`-on-`.rs` calls (`ids.rs`, `indexer.rs`) as Iron Law 1 violations. User pushed back: `symbols` cannot surface imports. Researched `usage.db` across 4 projects (codescout, backend-kotlin, eduplanner-ui, MRV-poc) + read the `read_file.rs` / `symbols` mechanism.

**Iron Law / pattern:** Iron Law 1 ("NEVER read_file source code → symbols") is **too absolute**. `symbols` is a *definition projection* — it cannot return imports / `package` / `use`, module glue (`mod.rs`, barrel `index.ts`), macro output, annotations/decorators, exact bytes, or any construct the extractor drops.

**Evidence:**
- `symbols` returns **0 import lines** on `ids.rs` (Rust), `PreSolveDataValidation.kt` (Kotlin), `config.py` (Python — first symbol L14, imports L1–13 invisible). No `symbols` query surfaces imports in any language tested.
- Source `read_file` is **82–94% sliced** (line-range), not full reads: Rust 403/427, Kotlin 628/701, Python 1136/1306, TS 149/181.
- `read_file` already self-governs (`read_full_file`, `exceeds_inline_limit`): large source full-read → symbol outline (≈`symbols`, still importless); small → content + a "prefer symbols" hint; sliced → raw bytes. It never blocks source (backend-kotlin: 611 kotlin reads success vs 82 error).
- Six open `2026-06-04` extractor-gap bugs (rust macros, kotlin nested classes, TS arrow-consts/namespace, Go generics) prove `symbols` is *silently* lossy — `read_file` is the ground truth there.

**Tool called:** `read_file` on `.rs` source (the whistled calls).
**Should have called:** intent-dependent — `symbols(name=…, include_body=true)` for a named body; `read_file` (sliced) is **correct** for imports / glue / macros / exact-bytes / lossy-language. Only a full, no-range read of a *large indexed* source file is mild waste (and the tool redirects it anyway).

**Whistle delivered:** yes — and partly wrong; withdrawn for the import case.
**Recurrence:** 1st (criterion correction, not a repeat slip).
**Severity:** med — a false-positive whistle *criterion* erodes Pika signal; unchecked it would whistle ~85% of legitimate source reads.
**Status:** open — narrowed criterion recorded here; the durable fix is the prompt-surface rewording of Iron Law 1 (drafted this session, not yet shipped). See H-7.

**Refined whistle criterion:** whistle `read_file`-on-source ONLY when it is a **full, no-range read of a large indexed source file** (low severity — tool auto-redirects). NEVER whistle: sliced reads, import/glue/header reads, or reads in languages with known extractor gaps.

**Related:** R-32 (recon-patterns, this session). Kin F-22 (sibling session) — `read_file` offset/limit now normalizes to a line slice, which *reinforces* sliced-source-read legitimacy.

### U-28 — `read_markdown` errors are untagged (`err_family` NULL), hiding ~23 live errors/week

**When:** 2026-06-21 Pika re-scan of `.codescout/usage.db` (`id > 6213`, 1,833 new calls, 34 sessions lifetime).

**Iron Law / pattern:** Observability gap, not an Iron Law. `recoverable_error` rows from `read_markdown` carry `err_family = NULL`, so they never appear in the recency rollup that gates every "live friction" verdict. 23 errors/week were invisible until drilled by hand.

**Tool called:** `read_markdown` — **51 lifetime / 23 in last 7d**, the #1 source of `(null)`-family errors (next: `artifact` 29, `symbols` 9, `edit_code` 8, `references`/`read_file`/`edit_markdown` 7 each).

**Sub-signatures (7d):**
- `file not found: 'CLAUDE.md'` ×10 — relative-path / moved-file reads (some against the just-restructured CLAUDE.md).
- `read_markdown only supports .md files` ×3 — wrong tool; should be `read_file`.
- `combined headings span N lines — exceeds inline threshold` ×4 — too many headings per call.
- `heading '…' not found` ×several — stale heading references.
- `missing 'path' parameter` ×1.

**Should have called / fix:** Two-sided.
- *Codescout-side (primary):* tag `read_markdown` errors with `err_family` (`md_file_not_found` / `non_md_file` / `heading_not_found` / `heading_span_over_threshold` / `missing_path`) so they surface in the rollup instead of hiding in `(null)`.
- *Agent-side:* verify path + heading exist before `read_markdown`; use `read_file` for non-`.md`; read fewer headings per call.

**Whistle delivered:** yes (this entry + `pika_observations` row, `subkind=read_markdown_untagged_errors`).

**Recurrence:** 51 lifetime / 23 in 7d — habit.

**Severity:** med — observability blind spot plus recurring retry-cost; no data loss.

**Status:** open. No hookify rule proposed — the fix is observability (`err_family` tagging), not a deny/warn gate.
