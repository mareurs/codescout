---
kind: tracker
status: active
title: Codescout Usage Frictions — U-N Log
tags:
- pika
- iron-law
- usage
entry_high_water_U: 50
entry_prefix: U
expects_augmentation: true
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

> **Citation note (2026-08-09):** `strip_project_root_from_result` (cited below at
> old `src/server.rs:351`, later `:1662`) was deleted in the field-aware-path-strip
> rework (Task 3, commit `93565509`). The mechanism it describes is historical: it
> was a blanket textual rewrite; current stripping lives in
> `src/tools/core/path_strip.rs`, an allowlist-driven walk over the typed `Value`
> (`PATH_KEYS`/`ROOT_KEYS`), invoked inside `Tool::call_content` before any
> rendering. `post_process` now only appends the once-per-activation banner —
> on the `activate_project` response itself, not "every tool response except
> `run_command`" as described below. See
> `docs/superpowers/specs/2026-08-09-field-aware-path-strip-design.md` and
> `docs/issues/2026-08-09-path-strip-corrupts-file-content-and-root-fields.md`.

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

> **Citation note (2026-08-09):** `strip_project_root_from_result` and
> `strip_prefix_from_text` (cited below at old `src/server.rs:1311-1313`/`:352`,
> later `:1662`/`:1702`) were both deleted in the field-aware-path-strip rework
> (Task 3, commit `93565509`) — the docstring this entry corrects no longer exists
> to be wrong. Current stripping is field-aware and allowlist-driven, in
> `src/tools/core/path_strip.rs`; buffer re-reads (`read_file(@tool_xxx, ...)`,
> `grep ... @tool_xxx`) already see already-relativized values because the walk
> runs once, inside `Tool::call_content`, before the buffer payload is built —
> not because a second pass re-strips them on read. See
> `docs/superpowers/specs/2026-08-09-field-aware-path-strip-design.md`.

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
   `docs/issues/archive/2026-05-21-run-command-strips-project-root-from-path-literals.md`).

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
`docs/issues/archive/2026-05-28-path-annotation-spam.md`.

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


### U-29 — Guards that reject AFTER accepting the payload cost a full re-transmission (×3 in one session)

**When:** 2026-08-04, provenance-measurement session (13 rounds, heavy file
authoring). Three instances of the same shape, two tools.

**Iron Law / pattern:** Not an Iron Law violation — every guard fired correctly.
The friction is that the guard's cost scales with the size of the content it
rejects, because validation happens after the argument payload is already in the
request.

**Tool called / instances:**
1. `create_file` → `file already exists` on a ~6 KB whole-file rewrite of a Python
   module. Correct refusal, good hint (`pass overwrite: true`), but the composed
   payload is discarded and complying costs a second full transmission. ~6k
   tokens.
2. `create_file` → `outside the project root` for a commit-message file in the
   job tmp dir. Returns `@ack_*`; re-invoking with the handle requires re-sending
   the entire content.
3. `create_file` → same, for a script written to the relocated artifact
   directory. Again a full re-send.

**Should have called / fix:** The agent-side fix is anticipation (pass
`overwrite: true` when the path may exist; write scratch inside the project or
ack first), and it is genuinely partial — existence and scope are not always
known ahead of the call.

The codescout-side fix is the one that generalises: on a rejection that the
caller can resolve by re-invoking, **buffer the submitted content server-side and
return a handle**, so the retry references it instead of re-sending. The `@ack_*`
pattern already does exactly this for the *approval token* — it just does not
carry the payload. Extending it would make the guard's cost constant rather than
proportional to content size, and the machinery is the same output-buffer system
that already backs `@cmd_*` / `@tool_*` / `@file_*`.

**Whistle delivered:** yes — this entry. Also logged as F-6 in
`docs/trackers/provenance-probe-session-log.md` (instance 1, before the pattern
was visible as a class).

**Recurrence:** 3 in a single session, all in file-authoring work. Expect it to
track how much whole-file writing a session does rather than being uniform.

**Severity:** med — pure token cost, no correctness impact, but it recurs for
every large rewrite and the cost is highest exactly when the content is most
expensive to have composed.

**Status:** open. No hookify rule proposed — a hook cannot see the payload cost;
the fix belongs in the tool's rejection path.


### U-30 — IL3 slips ×4 in one session; companion hook is warn-only and its deny twin was orphaned by the .sh→.mjs port

**When:** 2026-08-06, `experiments` → `master` merge-prep session. Four separate
`run_command` calls rejected server-side for IL3.

**Got:** each rejection came in two parts — a companion `PreToolUse` hook emitting
`additionalContext: "IL3 warning — piped … to a log-trimmer"` (advisory, allows the
call), then codescout's own gate returning
`IL3 violation — … BLOCKED`. The four shapes:

1. `git log … --pretty=%s | sed … | sort | uniq -c` (commit-type census)
2. `grep -m3 ^version Cargo.toml; …; git tag --sort=-v:refname | head -6` — the
   *offending* pipe was on `git tag`, but the compound started with a bounded
   `grep`, which is what made it feel safe
3. `cargo fmt --check; …; cargo check … | tail -5; cargo clippy … | tail -5`
4. `find docs -iname '*handoff*' | head` — bare `find` (no `-maxdepth`), so
   correctly unbounded

All four were the same instinct: trim a long output *at the source* rather than run
bare and query `@cmd_*`. Cases 2 and 3 are the interesting ones — a compound command
whose first clause is bounded reads as compliant, and the unbounded pipe hides in a
later clause.

**Resolves the ⚠️ flag on H-1** (open since the 2026-06-11 pika audit, which asked
whether the deny hook was reverted or never registered). Verified at the source
2026-08-06:

- `hooks/hooks.json:94` registers `il3-warn-hook.mjs`.
- `hooks/il3-warn-hook.mjs:2` says *"Port of il3-warn-hook.sh. Advisory only:
  allows the call, injects a context…"*.
- `hooks/il3-deny-hook.sh` exists on disk, is **not** in `hooks.json`, and is still
  `.sh` while every registered hook has migrated to `.mjs`.

So the answer is neither reverted nor never-registered: **the deny hook was orphaned
by the shell→mjs migration.** The warn variant was ported forward, the deny variant
was left behind in the old language. Companion IL3 has been warn-only since that
port, while H-1 recorded `shipped (deny)`.

**Cost:** 4 wasted round-trips. Also, because the companion warns rather than denies,
the warning arrives as *context on an allowed call* — the model reads it after having
already committed to the shape, which is precisely the position where a warning
changes least.

**Status:** open — the recurrence is behavioral; the wiring finding is actionable and
recorded on H-1.

**Promotes to:** H-1 (evidence added; the fix is to register a deny hook in `.mjs`,
or to delete `il3-deny-hook.sh` and stop claiming deny).

---

### U-31 — The shell-on-source guard makes the `tool-docs-sync` CI gate impossible to reproduce locally

**When:** 2026-08-06, diagnosing why the `Tool Docs Sync` CI job was red.

**Expected:** run the job's own command locally to see the diff before fixing.

**Got:** the job body is

```bash
grep -rhA1 'fn name(&self)' src/tools/ --include='*.rs' --exclude-dir=tests … \
  | grep -E '^\s*"[a-z_]+"' | sed 's/.*"\(.*\)".*/\1/' | sort -u > /tmp/code-tools.txt
grep -rohE '^#{1,2} `[a-z_]+`$' docs/manual/src/tools/ | … > /tmp/doc-tools.txt
diff -u /tmp/code-tools.txt /tmp/doc-tools.txt
```

`run_command` refused: `shell access to source files is blocked`, hint pointing at
`symbols` / `references` / `grep`. Correct per Iron Law 3 — but the CI gate *is* a
recursive shell grep over `src/`, so the gate and the guard are mutually exclusive.
Reproducing it meant re-deriving both sides with codescout `grep` and diffing 34
names by eye.

**Why it matters:** this is a class, not a one-off. Any CI gate implemented as
`grep -r` over sources is unrunnable from inside a codescout session, so the agent
asked to fix a red gate cannot see the gate's own output. The friction is not the
guard's correctness — it is that no escape hatch is advertised in the refusal for
"I am deliberately reproducing a build gate". `acknowledge_risk: true` exists and is
mentioned in the hint, but reads as a danger override rather than the right tool for
this job.

**Fix idea:** either (a) reimplement the CI gate against `codescout symbols` output
so one command works in both places, or (b) have the refusal hint name
`acknowledge_risk: true` as the sanctioned path for reproducing a CI gate
specifically, rather than only as a risk bypass.

**Status:** open.

---

### U-32 — `.buddy/` is read-exempt but not write-exempt, and the buddy summon flow tells you to use native tools

**When:** 2026-08-06, updating a buddy project memory after `/buddy:summon`.

**Expected:** `.buddy/` is guard-exempt — the summon command says so explicitly:
*"Read that one file first with native `Read`… the `.buddy/` path is guard-exempt,
and `read_markdown` would fragment a persona-sized file into a heading map."*
Native `Read` on the spilled payload worked exactly as documented.

**Got:** native `Write` to `.buddy/memory/docs-lotus-frog/<slug>.md` was **denied** —
*"codescout's create_file is the tracked path for new source files… The native Write
tool bypasses codescout's safety gates and file tracking."* Succeeded via
`create_file(overwrite=true)`.

The exemption is read-only, which is defensible — writes should be tracked. But the
asymmetry is undocumented on both sides: the buddy summon flow advertises native-tool
access for `.buddy/` without qualifying it as reads-only, and the memory protocol's
write steps (`git add .buddy/memory/<rel-path>`) don't name the write tool at all.

**Cost:** one round-trip. Trivial in isolation; recorded because it is a
prompt-surface disagreement between two active plugins, which is the class that
quietly wastes calls in every session that touches memory.

**Fix idea:** one clause in the summon command — *"native `Read` for the payload;
writes to `.buddy/` still go through `create_file` / `edit_file`"* — and the same
note in `data/memory-protocol.md` § Staging.

**Status:** open.

---


### U-33 — IL3 recurred repeatedly in the same session after U-30 was written; the shape is "trim for my own reading"

**When:** 2026-08-06, same session as U-30, *after* U-30 had already been written up.
Three further server-side IL3 rejections:

1. `cargo check --lib --all-features 2>&1 | tail -40`
2. `cargo check --no-default-features …; cargo test … | grep -E 'test result|FAILED'; …`
3. `git add -A && git commit … ; git push origin experiments 2>&1 | tail -2`

**Why this is not just "U-30 again":** writing U-30 — including its own analysis that
all four slips were "the same instinct" — did not reduce the rate. Three more followed
within the same session, giving **7 in one session**. That is evidence about the
intervention, not about the operator: a friction log entry is a *record*, and records
do not change in-flight behaviour. Only the gate did, every time.

**Sharper characterisation of the shape than U-30 had.** All seven share one property:
the trimmer was added to make the output **short enough for me to read**, not to select
information. `| tail -40`, `| tail -2`, `| tail -5` are pure length limits with no
predicate. That distinguishes them from a genuine filter (`grep FAILED`), and it is
exactly the case the `@cmd_*` buffer already solves better — the buffer *is* the
length limit, and it preserves the rest.

Case 3 is the most instructive: the pipe sat on `git push`, whose output is two lines
regardless. The trimmer bought nothing at all; it was reflex, not intent.

**Cost:** 3 more wasted round-trips (7 total this session). Each is a full
re-transmission of a long compound command — see U-29 for that family.

**Status:** open. Escalates U-30's priority rather than duplicating it: the recurrence
now spans a within-session write-up, so "the operator will remember" is falsified as a
mitigation.

**Promotes to:** H-1. The evidence now supports the stronger reading — a **deny** hook
is the only intervention that has ever changed this behaviour, and the companion's is
warn-only because its deny twin was orphaned by the `.sh`→`.mjs` port (U-30). A cheap
partial fix worth considering: deny only the *predicate-free* trimmers (`head`/`tail`
with no pattern), which is the whole observed population and carries no false-positive
risk against genuine filters.

---

**Update 2026-08-06 (round 4) — ×3 more, 10 total. The count keeps rising and the shape
has never varied.**

The three new ones, in order:

| # | Command | Trimmer |
|---|---|---|
| 8 | `find src crates -name '*.rs' -newer target/release/codescout` | `\| head -20` |
| 9 | `cargo build --release --bin codescout` | `\| tail -2` |
| 10 | `git ls-files crates/librarian-mcp` | `\| head` |

All three are **predicate-free length trimmers** — no `grep`, no filter, nothing but
"show me less". That is now **10 for 10**: not one violation across the whole session
was a genuine content filter. The denyable subset is exact, and a rule limited to
`head`/`tail` with no accompanying predicate would have caught every one while never
touching a legitimate `\| grep FAILED`.

**What this datapoint adds beyond U-30 and the first U-33:** the intervention itself is
now falsified twice over. U-30 was written to stop the slips; three more followed in the
same session. U-33 was written to characterise them; three more followed after *that*.
A friction log is a record, and a record does not change in-flight behaviour — only the
gate does. The advisory PreToolUse echo fires *after* the violation is already composed,
so it teaches nothing at the moment of writing.

**Promote-when: fired.** At 10 datapoints with zero counter-examples, this is no longer
an observation. The fix is a hook-side deny on `(head|tail)` with no sibling predicate in
the pipeline, which is a strictly smaller rule than the current advisory matcher.

**Refinement after 13 (same session, later):** #11–#13 were `grep -rln … | head -20`,
`git diff … | grep -E … | head -12`, and `git status --porcelain … | grep -v "^ M"`. The
last one is a **genuine content filter**, not a length trimmer — so the "not one was a real
filter" claim holds for the first ten and is **not** universal. Stating that plainly because
it is the counter-example the proposed rule has to survive, and it does: a
predicate-free-trimmer deny would have **allowed** #13, while the current unbounded-LHS gate
blocked it. That is the proposal working as intended — strictly less restrictive than
today's gate on exactly the case where the pipe was legitimate. It also means the current
gate has a real false-positive rate of its own, which the narrower rule would reduce rather
than add to.
**Further refinement after 15 (same session, later still).** #14 was a chained
`git log -S … -- .gitignore` whose command substitution carried `| tail -1` and whose outer
clause carried `| head -30`; #15 was `cargo fmt && cargo test --lib audit_doc_refs 2>&1 |
tail -40`. Both are predicate-free length trimmers, so the tally is **13 of 15 pure "show
me less"**, with #13 still the only genuine content filter.

**What is new at 15 is *when* they fire.** Both landed immediately after a stretch of
deliberate, careful work — a mutation-verified regression test, a scouted seam — rather
than during a careless patch. The rule was also quoted correctly, unprompted, elsewhere in
the same session. So this is not a knowledge gap: it is a reflex that fires when attention
is on the command's *content* rather than its *shape*. Compose-time is the only moment that
can reach a reflex, which is precisely where a record cannot act and a deny hook can.

That makes three interventions now falsified by recurrence within one session: U-30 (write
it down), U-33 (characterise it), and the advisory PreToolUse echo (warn after composing).
**Refinement at 16 (2026-08-07, WIN-30 session).** #16 was
`grep -oE '^## (F|W)-[0-9]+' <tracker> | sort -t- -k2 -n | tail -4` — reading the highest
allocated F-N/W-N id before appending a tracker entry. Predicate-free again, so the tally is
**14 of 16 pure "show me less"** and #13 remains the only genuine content filter. Two details
sharpen the *when*: the LHS was already narrow (one file, one anchored pattern), and the trimmer
existed only to spare me the middle of a sorted list whose tail was the entire point — the
cheapest imaginable case, which is precisely where the habit survives enforcement. It also fired
during tracker bookkeeping rather than build-log reading, so the pattern is not specific to
compiler output.

The deny-hook conclusion is unchanged and the evidence for it is now 16 deep.

### U-34 — `edit_code action=insert` takes the ANCHOR in `symbol`, while an `anchor` param also exists

**When:** 2026-08-06. Inserting a new function after an existing one.

**Tried:** `edit_code(action="insert", path=…, anchor="is_module_path", position="after", body=…)`
→ `missing 'symbol' parameter`. Then
`edit_code(action="insert", symbol="is_path_segment", anchor="is_module_path", position="after", …)`
— reading `symbol` as "the symbol I am creating" — → `symbol not found: is_path_segment`.
Third attempt, `symbol="is_module_path", position="after"` with no `anchor`, succeeded.

**Got:** two failed calls to learn that for `insert`, `symbol` names the **existing
anchor**, and `anchor` is either an alias or inert. The error messages are individually
correct and jointly misleading: the first says a required param is missing without
saying what it should contain, and the second confirms `symbol` is looked up as an
existing symbol — which only makes sense once you already know the answer.

**Cost:** 2 round-trips, one of them re-transmitting a ~16-line body.

**Fix idea:** either drop `anchor` from the schema, or make `insert`'s `symbol`
description read "the EXISTING symbol to insert relative to". A one-line hint on the
`missing 'symbol'` error ("for action=insert, `symbol` is the anchor") would have cost
zero round-trips. Kin U-26 (action grammar learned via sequential errors).

**Status:** open.

---

### U-35 — `artifact(find, filter={rel_path:{eq: "<exact path>"}})` returns 0 for a path that `contains` matches

**When:** 2026-08-06, looking up `docs/research/README.md` after `read_markdown`
refused it as librarian-managed.

**Tried:** `artifact(action="find", filter={"rel_path": {"eq": "docs/research/README.md"}})`
→ `count: 0`. Same call with `{"contains": "research/README"}` → the artifact, whose
`abs_path` is reported as `docs/research/README.md`.

**Got:** an exact-match filter missing a row whose displayed path is exactly the
argument. Most likely cause: `rel_path` is stored (or compared) as an absolute path
while `find` *displays* it project-relative via the path-strip layer, so `eq` compares
against a different string than the one shown. `get_guide("progressive-disclosure")`
does warn that "the catalog stores absolute paths; the strip layer is a display-time
transform" — but it frames that as a *verification* concern, not as a filter-semantics
one, and nothing on the `find` surface says `eq` needs the absolute form.

**Cost:** 1 wasted round-trip, and the failure mode is silent: `count: 0` from an exact
filter reads as "no such artifact", which would be a wrong conclusion for anyone not
suspicious enough to retry with `contains`.

**Fix idea:** either normalise `rel_path` comparisons to the displayed form, or have
`find` emit a hint when a `rel_path` `eq` filter yields 0 but a `contains` on the same
value would not. Document the absolute-vs-displayed asymmetry on the filter surface in
`get_guide("librarian")` § Filter Syntax.

**Status:** open.

---

### U-36 — The harness's batch-independent-calls rule directly contradicts codescout's serialize-writes rule

**Observed:** 2026-08-06, working the doc-drift backlog. Issued two independent
`edit_markdown` calls in one block — different files, no shared state.

**Got:** both succeeded, and a `PostToolUse` hook fired:

```
[cs-hint] Parallel writes risk inconsistent state (BUG-021) — serialize write tool calls.
```

**The conflict is textual, not a judgement call.** The harness system prompt ends with:

> If you intend to call multiple tools and there are no dependencies between the calls,
> make all of the independent calls in the same block, otherwise you MUST wait for
> previous calls to finish first

That is an unconditional instruction with an emphatic MUST on its converse. Two
independent edits to two different markdown files satisfy its antecedent exactly. So the
harness instructs the batch, and codescout's hook then flags the batch as a hazard.

**Why it matters more than the warning suggests:** the hook is `PostToolUse` — it fires
*after* both writes have already landed. If BUG-021 is real, the advisory arrives too late
to prevent the inconsistent state it names; if it is not real, the hint is training an
agent away from a batching rule the harness demands. Either way the current arrangement
cannot be correct.

**Severity:** med — no damage observed (both edits verified applied), but the agent is
left choosing which of two directives to violate on every multi-file doc pass, and doc
passes are exactly where independent writes are most natural.

**Fix idea:** state the carve-out where the batching rule is read, not only after it is
broken. codescout's server instructions already carry Iron Laws 1–6; a seventh line —
*"write tools (`edit_code`, `edit_file`, `edit_markdown`, `create_file`, `artifact`) are
serialized; batch reads freely"* — would resolve it before the call is composed, the same
reasoning as U-33's promote-when. If BUG-021 is genuinely a correctness hazard, the hook
should be `PreToolUse` and deny, not `PostToolUse` and hint.

**Status:** RESOLVED 2026-08-27 — BUG-021 is a **stale precaution as stated**. The decision this
entry was waiting on, with evidence:

BUG-021 bundles three modes. Two are dead; the third is alive but is not what the hook names.

1. **Crash mode** (rmcp cancellation race). Fixed upstream in rmcp 1.2.0; this repo is on **rmcp
   1.3** (`Cargo.toml:53`). `panic = "abort"` (`Cargo.toml:237`) was added during the same
   2026-03-03 investigation. Dead.
2. **Concurrency corruption** (torn writes, lost updates). **Prevented in code, not by rule.**
   `WriteGuard` (`src/agent/write_guard.rs`) takes an in-process async mutex *and* a cross-process
   `flock` on `<root>/.codescout/write.lock`, on a shared total timeout budget. It is wired at
   `server.rs:1077` → `acquire_write_guard_if_writing`, which fires for every `is_write_call` — so
   it covers `edit_file`, `edit_code`, `edit_markdown`, `create_file`, `artifact`, `memory`. It is
   also pinned to the `workspace=` target rather than the session default, explicitly so "a
   concurrent subagent's activate() [cannot] steal the lock target" (`server.rs:652-655`). The
   lock files exist on disk in both this checkout and its worktree. Dead as a hazard.
3. **Partial state on permission denial.** Batch N writes, deny one: the approved ones apply and
   nothing rolls back. **Still live, by design, `wontfix`** —
   `docs/issues/archive/2026-03-21-parallel-edit-file-partial-state.md`. Serialization does not
   address it, because it is approval asymmetry, not a race.

And a fourth cost nobody had named: **lock contention**. Batched writes serialize on the guard
with a **5 s** default (`write_lock_timeout_secs`, `src/config/project.rs:180`); a slow write can
push its siblings into a `RecoverableError` timeout. That is a live, current reason to prefer
sequencing — unrelated to corruption.

**So this entry's diagnosis was right and its framing was too generous to the hook.** The hook is
not merely mistimed, it names a mechanism that codescout closed in April. `PostToolUse` is the
correct timing for what survives, because neither surviving cost is prevented by refusing the
batch — denial asymmetry needs the human, and contention resolves itself in 5 s.

**Recommended action, unblocked:** correct the hook's *text* rather than its timing or severity.
It should stop citing BUG-021 and stop claiming inconsistent state; the honest line is that writes
are serialized automatically and batching them buys nothing while risking a 5 s timeout and
partial application under denial. That hook lives in the **companion plugin repo**
(`claude-plugins:codescout-companion`), not here, so it is a cross-repo edit and is left as a
decision rather than made silently.

The seventh-Iron-Law idea in *Fix idea* above should be narrowed if taken: not "write tools are
serialized" as a caution, but as a *permission* — the server serializes them for you, so the
harness's batching rule is safe to follow for writes to **different** files.

**Caveat found while verifying, filed separately:** the sole end-to-end proof of mode 2,
`write_lock_contention_produces_recoverable_error` (`tests/cross_process_write_lock.rs`), `return`s
green when `target/debug/codescout` is absent. It did run in this session's `cargo test`, so the
evidence above stands — but the guarantee is one missing binary away from being unverified.

### U-37 — `edit_file(replace_all=true)` right after an insert rewrote the line inside the helper being introduced, making it call itself

**Observed:** 2026-08-06, de-saturating `scan_meta.degraded` in
`src/librarian/tools/audit_doc_refs/resolver.rs`.

**Sequence, and the order is the whole bug:**

1. `edit_code(action="insert")` added a helper whose body was
   `ctx.degraded_languages.borrow_mut().push(lang.to_string());` guarded by an
   `if lang != "unknown"`.
2. `edit_file(old_string="ctx.degraded_languages.borrow_mut().push(lang.to_string());",
   new_string="note_degraded(ctx, lang);", replace_all=true)` to convert the two real
   call sites.

Step 2 matched **three** occurrences, not two — the third being the line inside the helper
from step 1. Result:

```rust
fn note_degraded(ctx: &ResolveCtx<'_>, lang: &str) {
    if lang != "unknown" {
        note_degraded(ctx, lang);   // <- unbounded recursion
    }
}
```

**Got:** `cargo check` **passed** — infinite recursion is not a compile error, and clippy's
`only_used_in_recursion` did not fire on this shape either. It surfaced as
`has overflowed its stack / fatal runtime error: stack overflow, aborting` (SIGABRT) in
`resolver_unknown_when_lsp_offline`, i.e. only because a test happened to exercise that
path.

**Severity:** med — no wrong code shipped, but the failure mode is a hard process abort and
the only thing that caught it was test coverage on the affected branch. A helper introduced
on a path with no test would have compiled, passed clippy, and recursed in production.

**Root cause:** `replace_all` is scoped to the file *as it is at that moment*, which
includes anything the previous call inserted. "Replace every call site" and "the helper
contains a copy of the call site" are the same string, and nothing distinguishes them.

**Fix idea, in order of preference:**

1. **Do the `replace_all` FIRST, then insert the helper.** The helper cannot be caught by a
   replace that already ran. Zero tooling changes; purely ordering.
2. Write the helper body differently from the call sites it replaces — here, using a local
   binding or a different receiver expression would have made the strings non-identical.
3. Tool-side: `edit_file` could report the **match count** on `replace_all` ("replaced 3
   occurrences"), which would have made this visible immediately. It currently returns a
   bare `"ok"`, so the caller never learns the arity of what they just did. That is the
   cheap generalisable fix, and it helps every `replace_all` caller, not just this shape.

**Status:** open — (1) and (2) are discipline and cost nothing; (3) is a small tool change
worth doing because a silent arity is what turned a two-site edit into a three-site one.


### U-38 — `edit_file(replace_all=true)` matched 1 of 2 intended sites (different indentation) and reported plain `"ok"`

**Observed:** 2026-08-07, mutation-testing the new walk-error guard in
`src/tools/symbol/symbols.rs`.

**Tried:** neutralise both `Err`-arm counters at once —
`edit_file(old_string="                    audit.errors += 1;", replace_all=true)`.

**Got:** `"ok"`. One of the two sites changed. The two statements are byte-identical apart from
leading whitespace: the accepted-files walk sits at 16 spaces, the tree-sitter fallback walk at
20, because it is nested inside `if matches.is_empty()`. My `old_string` carried 20, so
`replace_all` correctly replaced *all* occurrences of that string — one — and said so in the
only way it can: not at all.

**Iron Law / pattern:** not an Iron Law breach; a **silent partial application**. The response
is a bare `"ok"` with no match count, so "replaced 2" and "replaced 1" are indistinguishable
from the caller's side. `replace_all` reads as an intent ("every occurrence") while it is
actually a predicate ("every occurrence *of this exact string*"), and in Rust the same statement
at two nesting depths is two different strings.

**Cost, and it is the interesting part.** The partial mutation left one counter live, so the
test under examination still passed — which read as *"the test is vacuous"*. The next step I
nearly took was rewriting a correct regression test for a real defect. Two cheap checks
separated the explanations instead: `id -u` plus a `chmod 000` probe confirmed the test's
precondition genuinely held, and grepping for the mutation marker showed it had landed at only
one site. Recorded as F-10 in `docs/trackers/release-promotion-session-log.md`.

**Fix idea:** have `edit_file` return the match count (`{"ok": true, "replaced": 2}`) rather than
a bare `"ok"`. That single number would have made the partial application self-evident, and it
is the same class of gap as U-37 — both are cases where `edit_file` did exactly what it was told
and told the caller nothing about what that was. Two datapoints now, one session apart, same
tool, same missing field.

**Recurrence:** 1st observed for the indentation shape; 2nd for the "`edit_file` reports no
match information" family (see U-37).

**Status:** open — the workaround (grep for the marker after mutating) is a habit, not a
mechanism. A `replaced` count in the response would retire both halves.


### U-39 — `grep`'s zero-match result is silent about the hidden-path skip, so `.github/` reads as absent

**Observed:** 2026-08-07, checking whether WIN-30's bug file was right that `ci.yml` skip-lists a
test by name.

**Tried:** `grep(pattern="background_command_with_quotes_captures_output",
glob="**/*.{yml,yaml,rs,md}")`, then three narrowing retries — `glob="**/*.yml"`,
`glob=".github/**/*.yml"`, and `glob=".github/workflows/ci.yml"`, the file's exact path.

**Got:** 15 matches in 5 files with `ci.yml` absent, then `0 matches` three times. The name is on
`.github/workflows/ci.yml:117`. No response mentioned that hidden paths had been excluded.

**Iron Law / pattern:** neither an Iron Law breach nor a tool defect — `include_hidden` is a
documented parameter (`src/tools/grep.rs:41`, default `false`), the walk honours it at
`src/tools/grep.rs:106` (`wb.hidden(!include_hidden)`), and `include_hidden_searches_dotfiles`
pins the default. The gap is on the **reporting** side: `0 matches` is the same string for "does
not occur anywhere" and "occurs only where I did not look", so the default answers *absence*
questions wrongly and silently. Sharpest form — a glob spelling out a hidden file's full path
still returns empty, because `overrides` are applied inside a walk that has already pruned the
parent directory.

**Cost.** Four probes and a wrong direction: I had started writing this up as a walk-level tool
defect before finding the flag. Worse counterfactual — stopping at the first result would have
meant "correcting" the bug file's accurate `ci.yml:117` claim into a false one and then acting on
the correction, since this session's skip-list decision turned on exactly that line. In this repo
the blind spot covers all of `.github/workflows/`, `.pre-commit-config.yaml`, and `.codescout/`:
the CI configuration is invisible to every default grep.

**Fix idea:** the `WalkAudit` treatment `symbols` received in `3bfa4025` — build with
`hidden(false)`, apply the dot-prefix rule in a `filter_entry` closure that counts what it prunes,
and attach a `completeness_warning` naming `include_hidden=true` when a zero-match result had
entries pruned. Warn only when that count is non-zero, by the same reasoning that deliberately
left a trustworthy zero bare in `symbols`. Filed as
`docs/issues/2026-08-07-grep-zero-match-silent-about-hidden-skip.md`.

**Recurrence, 2nd instance same day — and it was me, in the shell, hours after fixing the tool.**
Hunting the source of a `RERANK_BASE_URL` value, I ran
`grep -rlE 'RERANK_BASE_URL' /home/marius/.claude-sdd/*.json …` across four candidate configs and
got zero hits, then reported the provenance as unresolved with eight ruled-out sources. The value
was in `/home/marius/.claude-sdd/.claude.json` all along: **`*.json` does not match a leading
dot.** Same defect class, different surface — the shell glob prunes dotfiles exactly as
`ignore`'s `hidden(true)` prunes dot-directories, and both report the absence as a plain zero.

The generalised rule is therefore not about codescout's `grep` at all: **any glob-scoped search for
absence must be assumed blind to dot-prefixed names until proven otherwise** — `include_hidden=true`
for codescout's `grep`, an explicit `.*` term or `-r` on the directory for shell globs. Knowing the
rule did not prevent the second instance; the tell that should have fired is *"I am concluding
absence from a glob."*

**Recurrence:** 2nd observed for the hidden/dotfile shape. 3rd in the *silent false negative from a
discarded-or-pruned walk result* family — after `symbols`'s `walker.flatten()` (`3bfa4025`) and
WIN-30's poll discarding its `Err` arm (F-11), both the same day. Three instances, three
different surfaces, one shape: the code knew something was missing and the response did not say
so.

**Status:** fixed-verified, same session. A `WalkAudit` in `src/tools/grep.rs` now names the
pruned entries in a `completeness_warning` on zero-match results, and the four dropped-error
sites found while implementing it (two `walker.flatten()`, two `std::fs::read`) are counted
rather than discarded. Seven tests, two mutations each killing exactly one test, gate green at
3522. `.git` and `.codescout` are excluded from the warning — `rooted_ctx` creating
`.codescout/` in every test root is what made the noise problem concrete: both exist by
construction everywhere, so counting them would have warned on nearly every zero-match and
taught readers to skip the warning. Still inert in a live MCP session until `cargo rb` +
reconnect.

### U-40 — A multi-line `old_string` failed as "not found" on text that was verbatim present, and the error cannot tell a bad needle from a bad haystack

**Observed:** 2026-08-17, adding a paragraph to `src/prompts/guides/iron-laws-detail.md`
after the "Bounded LHS is allowed" block.

**Call:** `edit_markdown(action="edit", heading="## Iron Law 3: …", old_string=<three
lines copied out of the file>, new_string=<the same three lines plus a new paragraph>)`.

**Got:**

```
old_string not found in section '## Iron Law 3: `run_command` output → buffer, not pipe'.
The text must match exactly (whitespace-sensitive).
scoped_miss_tier: "no_close"
```

Those three lines had just been read twice — once by `grep` against the file, once by
`get_guide("iron-laws-detail")` off the wire. They were verbatim present.

**Cause — mine, not the tool's, and that is the point.** The `old_string` reached the
server carrying literal `\n` two-character sequences instead of newlines, so it genuinely
was not in the file. A **single-line** anchor succeeded on the next call.

**Why it is still a friction.** The message and the remedy point in opposite directions.
*"not found … must match exactly (whitespace-sensitive)"* describes a haystack that moved,
so the reflex is to re-read the file — which I did, twice, and both reads *confirmed* the
text was there, which made the tool look wrong rather than the query. Nothing in the
response separates "your needle is malformed" from "the file changed".
`scoped_miss_tier: "no_close"` is the one field that does separate them, and it is
undocumented: a near-miss means the file drifted; `no_close` on text you just read means
your string is corrupt. The signal exists and says nothing to the reader.

**Verified, not assumed.** Two rival hypotheses: (H1) `edit_markdown` rejects multi-line
`old_string`; (H2) the escaping was mine. Probe — a two-line `old_string` with real
newlines against a scratch `.md` outside the project. It succeeded and replaced both
lines, refuting H1. Filing H1 as a tool bug would have been a false report against a tool
that behaved correctly; the probe cost two calls and one scratch file, deleted after. See
R-101 for the general form — name what the rival hypothesis predicts before recording a
verdict.

**Fix idea.** Make `no_close` self-describing instead of a raw tier name: when the miss is
`no_close` **and** the `old_string` spans more than one line, say which of the two causes
is likelier and name both. Exactly the shape of U-39 one tool over — the zero was right,
the silence about *why* was the defect.

**Also observed in the same act.** `append_entry`'s `next_step` said the heading must be
`## U-40 — <title>` (H2). This ledger's own augmentation prompt mandates
`### U-N — <title>` (H3), all 36 existing entries use H3, and `docs/TAXONOMY.md` says H3.
The resolver's `def_re` does not anchor on `#`, so both levels resolve — but an agent that
follows the response boilerplate writes a heading inconsistent with every sibling. The
generic hint should defer to the ledger's own convention, or say "match the surrounding
entries".

**The heading half is fixed — `bf485a00` (experiments).** `allocate_entry_id` now returns
the level the ledger's own entries use — the *mode* of `^#{1,6} PREFIX-N` over the body it
already scans for `body_max`, so a stray heading at another depth cannot decide the level
for every future entry — and `append_entry` phrases the hint with it. Where the body heads
nothing there is no level to observe, and the hint now says its suggestion is a DEFAULT
instead of sounding certain. Promoted to a standing rule the same day: Anti-Pattern 5,
*Asserting a Convention the Tool Never Read*, in `docs/PROGRESSIVE_DISCOVERABILITY.md` —
the file `CLAUDE.md` requires reading before adding or modifying any tool.

**Status:** open — the heading half is fixed; the `no_close` half, which is the primary
friction above, is not. `edit_markdown` still reports a malformed needle and a moved
haystack in identical words, and `scoped_miss_tier` remains the undocumented field that
separates them.

### U-41 — `snapshot_stale` asserts the row "still shows the PREVIOUS field values" without knowing which fields the row renders

**Observed:** 2026-08-17, updating `BL-37`'s `next` field in the open-issue work queue via
`artifact(action="update_entry")`.

**Got:**

```
snapshot_stale: This tracker renders a snapshot in its body, and its `BL-37` row still
shows the PREVIOUS field values — params changed, the file did not.
```

The body's snapshot table renders `id | phase | task | status | bug`. `next` is not one of
them, so the row showed nothing stale at all. Acting on the advice would have meant editing
a row that was already correct.

**Mechanism — read, not inferred.** `snapshot_stale_note`
(`src/librarian/catalog/augmentation.rs`) takes `artifact_id`, `entry_id` and the set of
claimed indices. It does **not** take `changed_fields`, so it cannot know whether any
changed field appears in the rendered row. What it establishes is "the body keeps a
snapshot, and this row is in it"; what it says is "the row still shows the PREVIOUS field
values". The gap between those two sentences is the whole friction.

The function's own doc comment is honest about the limit — *"no id comparison can see
that, so the signal is 'you changed fields the body still renders the old way'"* — which
makes this a **wording** defect rather than a logic one. The check is deliberately
imprecise and documented as such; the message is not.

**Why it matters more than a cosmetic quibble.** It is the same shape as Anti-Pattern 5 in
`docs/PROGRESSIVE_DISCOVERABILITY.md`, added earlier the same day: a tool stating something
specific it never verified. An advisory that cries wolf on every field change trains the
reader to skip it — and the case it exists for (a *rendered* column going stale, which no
id comparison can catch) is genuinely valuable and genuinely invisible otherwise.

**Fix idea.** Either pass `changed_fields` through and fire only when one of them appears
in the rendered row, or soften the message to what was actually checked: *"you changed
fields on a row this body renders — check whether the rendered columns are affected."* The
first is better and costs one parameter; the second costs nothing and is honest today.

**Not filed as a bug** deliberately: nothing is wrong, corrupt, or lost, and the advisory
is load-bearing in its true case. This is friction with the wording of a correct-but-broad
check. Same reasoning that kept U-40 out of `docs/issues/` once the probe showed the tool
had behaved correctly.

**Status:** open — friction recorded, no fix shipped.

---

### U-42 — `artifact(find)` silently drops a top-level `rel_path` and answers with page 1 of the whole catalog

**Observed:** 2026-08-17, resuming after compaction. Wanted the catalog row for one tracker file:

```
artifact(action="find", rel_path="docs/trackers/open-issue-work-queue.md")
```

**Got:** `count: 50` and a list of 50 real artifacts — a heading-swallow bug, the hygiene log, TAXONOMY, an onboarding doc — none of them the file asked for. No error, no warning, no `corrections` block. The same question written as a filter returned exactly one row:

```
artifact(action="find", filter={"rel_path": {"contains": "open-issue-work-queue"}})
→ count: 1
```

**Mechanism — read, not inferred** (HEAD `637b9d37`). `find::Args` (`src/librarian/tools/find.rs:16-45`) has no `rel_path` field and no `#[serde(deny_unknown_fields)]`, so serde drops the key and the call runs at defaults in both dimensions that mattered: `filter: None`, and `limit: 50` from `default_limit()` (`src/librarian/tools/find.rs:47-49`). **`count: 50` was never a match count — it was the page size.** The number that reads as a result *is* the default.

**Why the schema invites it.** `rel_path` is an advertised top-level param (`src/librarian/tools/artifact.rs:105`). Its description opens `create: relative path for new file` and then spends two more sentences on a different action — *"In find results: path relative to repo root…"* and *"When filtering by path use contains/prefix…"*. An agent looking for how to find by path finds `rel_path`, described in find terms, at top level. Of the 37 action-labelled properties in that schema, this is the one whose prose crosses actions.

That second half then teaches the **inverted** leaf shape — `{"contains": {"field": "rel_path", "value": …}}` — which `repair_node` (`src/librarian/filter.rs:243-250`) exists to correct, and whose comment calls it *"the most common filter error"* per usage.db. Verified on the wire: the inverted form runs, and returns `corrections.filter` with the rewrite plus a hint teaching the canonical shape. So the schema propagates the error its own repair path absorbs.

**The asymmetry is the friction.** `find` already has a Repair-and-Continue surface: hand it a *malformed* filter and it repairs, reports, and teaches. Hand it *no* filter because the key was dropped, and it says nothing at all. The louder mistake gets the help; the silent one gets a well-formed, plausible, wrong answer.

**Cost:** 1 wasted round-trip and a near-miss on a wrong conclusion — 50 plausible rows read as "the query matched 50 things" rather than "the query never ran." Same shape as R-104 (`docs/trackers/reconnaissance-patterns.md`), one step further: a **non**-zero from a report is also a claim about your query, and a count equal to the default limit is the tell.

**Fix idea:** lift a top-level `rel_path` into `filter={"rel_path":{"contains": v}}` and report it under `corrections`, exactly as the inverted-leaf repair already does — the `lift_top_level_param!` precedent (`src/librarian/tools/update.rs:289-303`), which exists because this same class shipped twice on `update`. `deny_unknown_fields` is **not** available here: the dispatcher passes `action` down into the sub-tool's `Args`, and adding it once broke every `artifact(update)` call (`src/librarian/tools/artifact.rs:262-269`). Separately, split `rel_path`'s description so find guidance lives on `filter`, and delete the inverted example.

**Class note:** this is a third variant of a family already archived here — `2026-07-20-artifact-update-toplevel-status-param-silently-dropped.md`, `2026-07-13-artifact-create-drops-topic.md`, `2026-07-13-artifact-update-phantom-schema-fields.md`. The first two were params honored *nowhere*; the third, phantom keys backed nowhere, fixed by deleting them plus the test `input_schema_has_no_phantom_update_fields`. `rel_path` is the variant that test cannot catch: it **is** legitimately backed — by `create`. Nothing asserts a key is honored by the action whose description claims it.

**Filed:** `docs/issues/archive/2026-08-17-find-silently-drops-top-level-rel-path.md` (archived 2026-08-17, id `7d9e2dc48eb2b128`).

**Verified on the wire** after `cargo rb` + `/mcp`: `artifact(action="find", rel_path="docs/trackers/open-issue-work-queue.md")` returned `count: 1` — the correct row — with `corrections.filter` naming the lift. The same call returned 50 unrelated rows before the fix.

**Status:** fixed on `experiments`, commit `4fad1aa4` (cherry-picked from `0a955491`; a fast-forward was unavailable because the bookkeeping commit landed on `experiments` after the branch was cut). `find::Args` gains `rel_path` and lifts it into `filter={"rel_path":{"contains": v}}` with the lift reported under `corrections`; the schema description drops its inverted-leaf example. The durable half is the schema parity probe — which, note, **cannot reach `rel_path`**, because that key is labelled `create:` and a label-driven sweep misses it by construction. The gap between a key's label and its prose is the bug, and `rel_path_description_and_find_support_agree` is what closes that half.

**Both follow-ups are now done (`63dd620a`).**

1. The probe is generalised from `find` to **all 12 actions**, renamed `every_action_labelled_schema_key_is_honored_by_that_action`: **37 labelled keys, all honored**. The soundness problem the find-only version dodged was that most actions have required params, so "assert the probe call errors" is vacuous — it errors either way. The fix is to compare *two* calls, required-params-only versus the same plus one ill-typed key, and require the outcome to **differ**; a discarded key leaves both identical. Required params are dummies chosen to fail *resolution*, after deserialisation, so the baseline is stable. Mutation-verified on a `get`-labelled bogus key — deliberately not on `find`, so the new machinery is what gets exercised. `ACCEPTS_ANY_JSON` declares the probe's one blind spot (fields taking arbitrary JSON) and is empty today.
2. **`repo` is clean** — checked, not assumed. Its description is purely `create:`-facing and its only cross-reference is to `rel_path` as a create-time companion, so there is no label-versus-prose mismatch there.

---

### U-43 — The shell-on-source gate counts every relative path as in-project, so a `cd` out of the repo is refused with a hint that cannot be followed

**Observed:** 2026-08-17, extracting the artifact schema to a scratch file under `$CLAUDE_JOB_DIR/tmp` — outside the project — and running `awk` over it:

```
run_command("cd /home/marius/.claude-kat/jobs/44c01c0f/tmp && awk '…' artifact_head.rs")
→ shell access to source files is blocked
  hint: use read_file(path, start_line, end_line), symbols(path), … instead
```

**Got:** a refusal whose remedy is unusable — `read_file`/`symbols` resolve against the active project, and the file is not in it. Copying the byte-identical file to `artifact_head.txt` and re-running made it pass, which first read as *"the gate discriminates on extension, not location."* **That reading was wrong**, and only reading the guard showed it.

**Mechanism — read, not inferred.** `segment_reads_project_source` (`src/util/path_security.rs:1295-1299`) does check membership, through `path_is_within_project` (`src/util/path_security.rs:1307-1320`) — which states its own assumption:

> *"Relative paths are inside by construction — `run_command` executes with the project root as its cwd."*

`run_command` does start at the project root, but the command may `cd` elsewhere first and nothing tracks that. So the bare token `artifact_head.rs` takes the `is_relative() → true` branch and is classified as project source. The extension was necessary, not sufficient — the rename passed because it failed the extension half, not because location was ever re-evaluated.

**Why it matters: it is the residue of a fix shipped the day before.** The membership check landed 2026-08-16 in `433100bd` ("fix(il3): stop blocking source reads outside the project", GF-3 in `docs/trackers/2026-08-16-iron-law-gate-firing-audit.md`) precisely to stop refusals whose hint could not be followed — measured then at **25 of 111** `il3_shell_on_source` refusals naming a path outside the project. The `cd`-then-relative slice survives that fix, and the fix's own justification applies to it verbatim.

**Cost:** 1 wasted round-trip, plus one wrong mechanism that would have shipped as this entry's explanation had the guard source not been opened. The workaround — rename to a non-source extension — is discoverable and teaches the wrong lesson.

**Fix idea:** resolve a relative token against the segment's *effective* cwd rather than the project root. `check_source_file_access` already splits on `&&`/`;`/`|`, so a leading `cd <path>` in the segment is available at the point the decision is made. Keep the conservative bias: no `cd` seen, or an unresolvable `cd` target, still means inside.

**Filed:** `docs/issues/archive/2026-08-17-source-gate-treats-relative-paths-after-cd-as-in-project.md` — **FIXED 2026-08-18**, `be2d7781`, wire-verified after rebuild. Two causes behind one predicate: the shell can move (`cd` is now tracked per run, never across a pipe, and only when fully resolved), and an option can carry a source extension without naming a file (`--include='*.mjs'` is a filter pattern, and its relativeness forced the in-project verdict on its own). The second carve-out is keyed on **positive evidence** of an out-of-project operand, not on "options are not paths" — the tidier rule silently stops guarding `grep -rn x src/ --include='*.rs'`, where the glob is the only token naming the extension because `src/` carries none.

**Status:** FIXED 2026-08-18 (`be2d7781`, archived `92a88ff0405b47bc`) — zombie-open caught 2026-08-18, the entry's own "Filed:" line already said FIXED while this field still said open. Verified rather than trusted: `git branch --contains be2d7781` shows `experiments`; `path_is_within_project` (`src/util/path_security.rs:1438`) carries the `Cwd::At`/`Cwd::Unknown` split the entry describes; regression test `a_cd_out_of_the_project_makes_a_relative_source_read_reachable_again` exists.

---

### U-44 — The IL3 warn-hook's unbounded-LHS list contains the commands its own message calls bounded, so it contradicts itself in a single response

**Observed:** 2026-08-17, while committing U-42/U-43. `git log -3 --format='%s' | tail -30` — a pipe the IL3 rule explicitly permits, because `-3` is an output limiter — drew an IL3 warning. The server ran it: `exit_code: 0`.

**Got:** the advisory and its own counter-example in the same message.

```
run_command("ls docs | head -2")
→ exit_code: 0   stdout: adrs\nagents      ← server allowed it

PreToolUse hook: IL3 warning — piped `ls docs | head -2` to a log-trimmer.
  … bounded-LHS pipes (ls/cat/awk/sed/find -maxdepth N) pass through.
```

The hook flags `ls | head` and then, four lines later, names `ls` as a command that passes through.

**Mechanism — read, not inferred.** `claude-plugins/codescout-companion/hooks/il3-warn-hook.mjs:23`:

```js
const LHS = '(cargo|npm|pnpm|yarn|python|pytest|go|mvn|gradle|git|find|ls|grep|cat|diff|du|stat|rg|fd)';
```

One flat alternation, no limiter check and no bounded set. It lists `ls`, `cat`, `diff`, `du`, `stat`, `find` and `grep` — which the server's gate treats as **bounded** — alongside `cargo` and `pytest`, which are genuinely unbounded. `git` is in the list unconditionally, where the server's rule is *"`git` is unbounded ONLY without an output limiter: `git log -3`, `git status --short`, `git show --stat` are bounded and may be piped."* Nothing in the hook can see a limiter.

So the false-positive surface is not one command — it is every bounded-LHS pipe the Iron Law deliberately carves out. Measured this session: 3 warnings on 3 legal pipes, 0 true positives.

**Why it matters more than noise.** It is the same failure U-41 names for `snapshot_stale`: an advisory that fires on correct behavior trains the reader to skip it, and IL3's true positives (a piped `cargo test`) are the ones worth catching. It is also the shape of U-22 — an IL3 detector false positive — recurring in the sibling implementation rather than the same one, which is the tell that the rule is duplicated rather than shared: the server has the predicate with its carve-outs, the hook has a hand-copied approximation, and only one of them was updated.

**Cost:** noise on every legal bounded pipe, plus the second-order cost of a self-contradicting message — a reader who trusts the warning over the adjacent text will rewrite working commands into two-call buffer round-trips for no reason.

**Fix idea:** split the alternation into `UNBOUNDED` (cargo, npm, pnpm, yarn, python, pytest, go, mvn, gradle, rg, fd, recursive grep, `find` without `-maxdepth`) and treat everything else as bounded; add the git-limiter carve-out (`-N`, `--max-count`, `--porcelain`, `--short`, `--stat`, `-n`, and *not* `--oneline`, which bounds width rather than line count). The server's own predicate is the specification — port it rather than re-approximating it, or better, have the hook defer to the server entirely, since the message already admits the server is the enforcer.

**Version-bump trap for whoever fixes this:** each of the three profiles resolves plugins from its own version-keyed cache, so editing `il3-warn-hook.mjs` in the source repo changes behavior in **no** profile until `.claude-plugin/plugin.json` `version` is bumped (currently `1.16.8`) and the install records are refreshed in all three of `~/.claude`, `~/.claude-sdd`, `~/.claude-kat`. A content-only edit verifies as "no change" and reads as a failed fix.

**Filed:** `docs/issues/archive/2026-08-17-il3-warn-hook-flags-bounded-lhs-pipes.md` — **FIXED 2026-08-17**, `claude-plugins:a989d73` (shipped in companion `1.16.9`). The hook was deleted rather than corrected: it is `contextPreToolUse` and can never block, so it was redundant when the server refused and wrong when the server allowed, and correcting the regex would have rebuilt the duplicated predicate behind this and U-22. `path_security.rs` is now the sole implementation. Its own test suite asserted the false positives as intended behaviour and was deleted with it.

**Status:** fixed-verified 2026-08-18. Zombie-open until then — the fix landed 2026-08-17 and this line was never flipped, which is exactly the pattern CLAUDE.md's verify-open cadence exists to catch: the *Filed* paragraph above already said FIXED while the field a sweep reads still said open.

Re-verified independently, at the bytes and on the wire, not from the record:

- **All three profiles** carry the 1.16.9 cache with `hooks/il3-warn-hook.mjs` **absent**, `il3-deny-hook.sh` present-and-unwired, and **zero** `run_command` matchers in `hooks.json`. Install records read 1.16.9 in each, and every `installPath` starts with its own profile root (the cross-profile drift check from the global CLAUDE.md).
- **Live discriminating pair, re-run in this session** (`~/.claude-kat`), both halves needed: `ls docs/issues | head -3` — bounded LHS, legal — ran `exit_code: 0` **with no advisory**, the same command shape that produced the self-contradicting warning quoted in *Observed*; and `cargo clippy --all-targets -- -D warnings 2>&1 | tail -5` — unbounded LHS — was **refused by the server** with the full `@cmd_*` recovery text. The first alone would equally describe a fix that merely disabled IL3; the second proves enforcement is untouched and still emits the better message the deletion argument rested on.
- **The commits are pushed**, contrary to the bug file's Resume: `git ls-remote origin refs/heads/main` returns `478bc7d`, matching local `main`. That Resume was written under `NO_PUSH=1` and someone pushed afterwards; it has been corrected.

**The version-bump trap above is also now stale in a way worth reading carefully:** it names `1.16.8` as the current version, meaning the version to bump *from*. It was bumped, to `1.16.9`. A later reader skimming that paragraph for pending work will mistake the from-version for an unfinished to-version — which is precisely the error a compaction of this session made, reporting U-44 as "needs the three-profile 1.16.8 bump" when the bump was three commits old.

**Unswept call sites, found while closing this (2026-08-18).** Deleting the hook did not delete the surfaces that announce it. `docs/architecture/companion-plugin.md` — the inventory CLAUDE.md names as authoritative — still listed the matcher as carrying `il3-warn-hook.sh` and IL3 as "warn-only"; corrected in the same commit as this flip. Three machine-local surfaces in `claude-plugins` said the same: `.codescout/memories/architecture.md`, `.codescout/memories/gotchas.md`, and `.codescout/system-prompt.md`, the last of which is injected verbatim into every subagent in that repo. All three are gitignored (`claude-plugins/.gitignore:8`), so they are per-machine state a commit cannot repair — the memories were fixed through `memory()`; `system-prompt.md` needs `onboarding(action="refresh_prompt")` and is left open, because its staleness predates this fix (it still lists the pre-1.14.0 `.sh` hook names) and `claude-plugins/docs/trackers/version-bump-checklist.md:118` already scoped it out of a release. Same shape as U-22 and the buddy lesson `platform-law-leaks-at-call-sites`: the law changed in one place and the call sites nobody swept keep asserting the old one.

---


### U-48 — I tripped IL3 three times in the session whose subject matter was IL3

**Observed:** 2026-08-18, in a session that fixed the IL3 warn-hook (U-44), rewrote the IL3 row in `docs/architecture/companion-plugin.md`, and put the rule into two guides. Three of my own calls were refused by that same gate: `cargo clippy ... 2>&1 | tail -5`, `git -C <repo> check-ignore -v <paths> 2>&1 | head`, and `cargo rb 2>&1 | tail -3`.

**Got:** all three are **true positives**. The gate was right every time, the rule was in context every time, and I had just finished writing prose about it.

**Why this is a friction entry and not a confession.** It is a field replication of A-24 on a sample of one agent: A-24 measured immediate compliance at 96% against **per-session repeat of 47% for `il3_pipe` and 71% for `il1`** — agents obey every refusal and still cannot predict the next one, because the message teaches the CALL and not the PREDICATE. My three slips are that shape exactly. Each refusal was obeyed correctly and immediately; none made the next pipe predictable.

The three share a form worth naming: **each was a convenience trim appended to a long-output build or git command** (`| tail -3`, `| head`) reached for to keep the response small — the exact habit `@cmd_*` buffers exist to replace. Knowing the rule as prose is evidently a different representation from applying it while composing a command.

**Cost:** three wasted round-trips and no wrong output, since IL3 denies rather than warns. The evidential cost is larger: an agent that has just authored the rule is not a clean subject for judging whether the rule is learnable, so my own compliance is not evidence about the surface.

**Fix idea:** none for the gate — it works. This belongs with A-24's finding that the refusal should name the predicate, not just the call. Worth citing there as the n=1 in-the-wild replication.

**Status:** open — evidence for A-24, no independent fix.

---

### U-49 — `entry_filter={}` is rejected instead of meaning "every entry"

**Observed:** 2026-08-18, trying to read all entries of a params ledger to backfill headings.

```
artifact(action="get", id="52451519052d207c", entry_filter={})
→ leaf must have exactly one field, got 0
  hint: Each leaf has shape `{field: {op: value}}`. Wrap multiple fields with `and`/`or`.
```

**Got:** an error whose hint is about leaf *shape*, which is not the mistake. The intent — "no constraint, return everything" — is the obvious reading of an empty filter and the natural way to ask for a whole collection.

**Workaround found:** `entry_filter={"id": {"contains": "<PREFIX>-"}}`, which matches every row because every id carries its prefix. It works, and it is a tautology dressed as a filter — the sign of a missing affordance rather than a real constraint.

**Cost:** small per instance, paid four times in one session (once per ledger backfilled), each time re-deriving the same tautology. The `get` response already carries `entry_total` alongside `entries`, so "all rows" is clearly an expected shape.

**Fix idea:** treat `{}` as the identity filter and return the whole collection, or refuse it with a hint that names the tautology workaround. The first is better: an empty AST node has one sensible meaning, and the filter engine already reports `entry_total` for the unfiltered set.

**Status:** open.

---
### U-45 — The heredoc carve-out in the source gate covers only the segment holding `<<`, so one `|` anywhere in the body re-exposes it — and this one blocks, it does not warn

**Observed:** 2026-08-17, committing U-44. `git commit -F -` with a heredoc message was **refused** — not warned — because the message body quotes a regex alternation full of `|` characters. The gate split the *prose* on those pipes, found a segment beginning `head`, found a filename elsewhere in the same span, and read it as `head <source-file>`.

**Got:** `shell access to source files is blocked`, on a `git commit`. No source file was read; none was named as an argument.

**Minimal reproduction — one token apart.** Control, allowed:

```
run_command("true <<'EOF'\nhead -1 foo.rs\nEOF")   → exit_code: 0
```

Add `x | ` to the body and nothing else:

```
run_command("true <<'EOF'\nx | head -1 foo.rs\nEOF")
→ shell access to source files is blocked
```

The heredoc carve-out works in the first case and is gone in the second.

**Mechanism — read, not inferred.** `check_source_file_access` (`src/util/path_security.rs:1211-1277`) splits the command on `&&`, `||`, `;` and `|` **first**, then tests each segment:

```rust
let blocked = segments.iter().find(|seg| {
    // Heredoc: the command reads from stdin, not a source file.
    if seg.contains("<<") {
        return false;
    }
```

The skip is per-segment, so it protects only the segment that literally contains `<<`. A heredoc body is opaque data, but the split has already chopped it into segments, and every segment after the first `|` is scanned as if it were a command. Its own doc comment states the broader intent it does not implement:

> *"Heredocs (`cat <<'EOF'`) read stdin, not a file; **any source extension appearing inside the heredoc body is not a filename argument.** Segments containing `<<` are skipped — the operator unambiguously means stdin redirection."*

Sentence one is the correct rule; sentence two is a strictly narrower mechanism, and the gap between them is the bug. Ordering is the fix: a heredoc is a *region*, so it has to be excised before the split, not tested after it.

**Why this is not U-22.** U-22 is the same *shape* — shell syntax read out of opaque string content — and is `fixed-verified` at `codescout-companion:d64749e`. But that fix strips **quoted** substrings in the **hook**, and this is unquoted heredoc body hitting the **server's source gate**, a different gate on the enforcing side. U-22 also scoped out compound decomposition explicitly, which is precisely the mechanism here. So the de-quoting pass cannot reach it, and the third instance of this shape in this ledger (with U-44) is the argument that the shape, not any single regex, is what needs fixing.

**Cost:** one hard-blocked commit and a rewrite of the message to drop the alternation, then a second block, then the file route. Severity is a step above U-22 and U-44: this one refuses work rather than adding noise, and the diagnostic names source-file access for a command that reads no file, so the message actively misdirects.

**Fix idea:** excise heredoc regions before `split_outside_quotes` — on seeing `<<[-]?['\"]?DELIM`, drop everything through the line matching `DELIM`, then split what remains. `split_outside_quotes` already exists and already respects quotes; heredocs need the same treatment one level up. Keep the conservative bias: an unterminated heredoc drops to end-of-command.

**Workaround that works today:** write the message to a file and use `git commit -F <path>`. The command string then contains neither the pipes nor the filenames. This is the same workaround U-22 documented, still needed for the case its fix did not cover.

**Filed:** `docs/issues/archive/2026-08-17-heredoc-carve-out-defeated-by-a-pipe-in-the-body.md` (archived 2026-08-17, id `f4784780d5413db1`).

**Verified on the wire** after `cargo rb` + `/mcp`, all four cases:

| command | before | after |
|---|---|---|
| heredoc body containing a pipe | blocked | **runs** (exit 0) |
| `cat src/main.rs <<< x` | **allowed** | blocked |
| real pipe after a closed heredoc | blocked | blocked |

The second row is the bypass this fix closed, and it is the one worth re-reading: it was **permitted** before. The third is the guard — a careless heredoc-stripping fix hides a genuine read behind a heredoc, and it holds.

**Status:** fixed on `experiments`, commit `4fad1aa4` (cherry-picked from `0a955491`). The fix idea above was wrong in a useful way: it proposed writing a `strip_heredoc_bodies` helper, and that helper **already existed** (`src/util/path_security.rs:747`), already called by `detect_il3_violation`. Only `check_source_file_access` kept the older approximation — so this was one call site that never adopted an existing contract, and the fix is a two-line ordering change. Removing the per-segment skip also closed a bypass nobody had noticed: a here-string puts `<<` in the same segment as a real read, so `cat src/main.rs <<< x` was skipped entirely. Mutation-verified.

A third gate defect surfaced from these tests and was filed separately, then **fixed** at `308014b5` and archived (`docs/issues/archive/2026-08-17-source-gate-does-not-split-on-newlines.md`, id `2f686e4b0ccd4fc6`): the segment splitter never broke on a newline, so `echo hi\ncat src/main.rs` read project source unchecked. Reviewing the sibling pipe gate for the same gap — which that bug's plan said to do — found it there too, with the opposite sign: `pipeline_segments` had the same omission as a false **negative**, so a piped `cargo test` on line two escaped IL-3 entirely.

---

### U-46 — `audit_doc_refs` reads `e.g` and `1.16.8` as module paths and `contains/prefix` as a file path — two FP mechanisms, 12 of 50 findings

**Observed:** 2026-08-17, running a scoped `audit_doc_refs` over the four bug files written for U-42–U-45 as the doc-side gate before committing.

**Got:** `exit_code: 0`, `n_refs_found: 109`, `n_refs_resolved: 52`, `n_refs_broken: 29`, `n_refs_unknown: 28`. Reading the 50 shown findings rather than trusting the zero — R-104 — **none** was a real broken reference. The breakdown by kind:

```
36  file_path
12  module_path
 2  file_line
```

Two distinct false-positive mechanisms, both triggered by punctuation in prose:

| `raw_ref` | Classified | Actually |
|---|---|---|
| `contains/prefix` | `file_path` | two op names joined by a slash, quoted from a schema description |
| `ls/cat/awk/sed/find` | `file_path` | a command list from the gate's own hint text |
| `e.g` | `module_path` | an abbreviation |
| `1.16.8` | `module_path` | a plugin version number |
| `ext_re.is_match`, `expanded.is_relative`, `expanded.starts_with` | `module_path` | Rust method calls on locals, inside quoted code |
| `corrections.filter` | `module_path` | a JSON response key path |
| `usage.db` | `file_path` | a filename with no directory |

The remainder are legitimate non-resolutions and correctly reported: deliberate repro fixtures (`foo.rs`, `x.rs` — `foo.rs` *not* existing is the point of U-45), cross-repo `claude-plugins/…` paths, and ephemeral session-scratch paths in evidence sections.

**Mechanism — partly inferred, and said so.** Not traced to a line this pass. The pattern in the data is that a slash makes a token a `file_path` candidate and a dot makes it a `module_path` candidate, with no test that the shape is *plausible* for the kind — `e.g` has no path-like structure beyond the dot, and `1.16.8` is three integers. The `module_path` half is the same family as U-15 ("mis-parses Rust `::` separator + classifies git refs as paths") and U-17 (39 FPs from instructional placeholders), which is why this is one entry rather than three: the classifier has a recall-first bias and no cheap plausibility filter.

**Severity is lower than it first looks, and the correction is the point.** These never gate. CI runs `--fail-on high` (`.github/workflows/ci.yml:370`), and every one of these is `Med` after `issues_drop` capping — `fail_on=high` ignores Med (`src/librarian/tools/audit_doc_refs/mod.rs:1168`, pinned by `fail_on_high_ignores_med_severity`). An earlier read of this same data claimed they "would gate at med in an ADR"; that was wrong in the direction that matters, because it inflates a precision issue into a CI risk. The real cost is **report noise**: 29 broken + 28 unknown, all benign, is a haystack a reader has to walk before finding a real drift — and the tool is documented as *manual*, run when drift is suspected, which is exactly when a 57-item benign list is most expensive.

It would gate for anyone running `--fail-on med`, which is the setting a team tightening doc hygiene reaches for first.

**Fix idea:** add a plausibility filter before emitting, not after — reject `module_path` candidates whose segments are all-numeric (`1.16.8`), whose final segment is a known English abbreviation (`e.g`, `i.e`, `etc`), or that resolve to a local-variable method call already inside a fenced code block; reject `file_path` candidates with no path-ish segment and a known-prose separator. Cheaper alternative, and probably the right first move: extend the existing `cap_code_comment` treatment so a ref extracted from **inside a fenced block** in a bug file drops to `Low` — most of the 12 `module_path` FPs here are quoted code, and quoted code is a citation of nothing.

**Not filed as a bug** — same reasoning as U-40 and U-41. Nothing is wrong, lost, or gated; the classifier's recall-first bias is a deliberate design stance and the capping layer is already absorbing the consequence. This is friction with the precision of a correct-by-design check, and it belongs with U-15 and U-17 as evidence for a future precision pass rather than as a fifth open bug.

**Status:** partially fixed 2026-08-18 — the `module_path` half of the 12 sampled FPs. Verified before fixing: static trace of `has_known_ext` disagreed with this entry's own table for `usage.db` (claimed `file_path`; the extension list has no `db`, so it cannot classify that way). A live repro fixture confirmed the trace, not the table — `usage.db` classifies `module_path`, and this entry's mechanism table is stale on that one row.

`is_module_path` gained two rejections: no dot-separated segment set where every segment lacks an alphabetic character (kills `1.16.8`, a version number, and generalises — any all-numeric dotted token is rejected, not just this one), and a small denylist for `e.g`/`i.e`. TDD: two failing tests first (`a_bare_version_number_is_not_a_module_path`, `common_latin_abbreviations_are_not_module_paths`), a positive-control test guarding against over-rejection (`the_new_filters_do_not_touch_real_module_paths`, pinning `os.path` and `commits.git_root`), then the minimal fix. Gate: fmt 0, clippy 0, 4198 passed (4 new) / 0 failed.

**Deliberately left open:** the `ext_re.is_match`/`expanded.is_relative`/`corrections.filter` shape and the `contains/prefix`/`ls/cat/awk/sed/find` shape. Both are syntactically identical to real citations — the first to a genuine Python-style module path (`os.path`, `mrv.chat_app`), the second to a genuine extensionless directory path (`crates/librarian-mcp`) — and neither has a discriminator that doesn't also reject the real case. The method-chain shape is actually the *documented* markdown default (`PathSyntax::for_language(None) == DottedModules`, deliberately unchanged since SD-1's sweep, per this file's own doc comment) rather than a bug: an inline code span carries no language, so the classifier cannot know `ext_re.is_match` is Rust field access rather than a Python-style name. Forcing either heuristic risks trading false positives for false negatives on real content, which is the same mistake this entry's own analysis warned against ("no test that the shape is plausible for the kind").

---

### U-47 — Between `EnterWorktree` and `workspace(activate)`, writes are blocked but reads are not — so git reconnaissance answers about the wrong checkout

**Observed:** 2026-08-17, twice in one session, opening a worktree to fix the newline gap.

**Got:** `EnterWorktree` moves the *session* cwd into the worktree, and its post-hook says:

```
WORKTREE DETECTED: codescout must switch to the worktree.
MCP write tools (edit_code, edit_file, edit_markdown, create_file) are BLOCKED
until workspace is called — they would otherwise silently write to the wrong repo.
```

Writes are blocked. **Reads are not.** In that window `run_command` still resolves against
the previously-active project, so this — run from inside the worktree — described the main
checkout:

```
run_command("git log -1 --format='%h %s'; git rev-list --left-right --count experiments...HEAD")
→ 3ecb8730 docs(prompts): correct the … claim
  0   0
```

I read that as "the worktree is based on `experiments`, no reset needed" and moved on. The
worktree was actually at `eca9902e` — **origin/master, 1091 commits behind** — because
`worktree.baseRef: fresh` branches from the default branch. The real state only surfaced
when `edit_code` said `symbol not found` and `wc -l` gave 2199 lines against the 3403 the
file actually has.

**The tool did warn me, in that same response.** The `_workspace_notice` field said it
exactly:

> *Reads are resolving against "/home/marius/work/claude/codescout". This repo also has
> linked git worktrees […] and no project has been explicitly activated, so results
> describe the main checkout even if you are working in a worktree.*

So this is not a missing signal. It is a **correct signal in a sibling field, next to a
confident-looking answer** — and the answer wins attention. The asymmetry is what makes it
dangerous: a blocked write fails loudly and is unmissable; a read that quietly answers
about another tree looks like a finished reconnaissance step.

**Second, smaller defect in the same hook:** it names the wrong path. It says to call
`workspace(action="activate", path="<MAIN root>")` when the session has just moved *into* a
worktree — activating main would leave every subsequent path resolving to the checkout you
deliberately left. The right call is the worktree root, plus `read_only=false`, since a
foreign activation defaults to read-only (`get_guide("workspace-state")` § home/foreign).
I overrode the hook both times and it was right to.

**Cost:** two wrong conclusions about which tree I was on. The first time (earlier session)
I caught it from a suspicious line count; the second time only from a failed `edit_code`.
Either could have ended with tests written against a 1091-commit-old file and a "why does
this symbol not exist" investigation.

**Fix idea, in order of preference:**

1. **Have `EnterWorktree` activate the worktree itself.** It knows the path — it just
   created it. The manual step exists only to open this window.
2. **Block reads too, not just writes,** until a project is chosen when linked worktrees are
   present. The precedent is `guard_worktree_write`, which already refuses *writes* on
   exactly this ambiguity; the same argument applies to a read whose answer names a tree.
   The bug that armed that guard
   (`docs/issues/archive/2026-08-15-worktree-guard-covers-writes-but-not-reads.md`) made
   this same case and was closed on the write half only.
3. **At minimum, promote the notice from a sibling field into the answer** — prefix the
   `stdout` rather than adding a key beside it, so it cannot be read past.

**Verify-first rule this yields, cheap and general:** the first command after entering a
worktree should be `pwd`, and its output compared to where you think you are. One token of
output, and it is the only thing in the response that cannot be about the wrong tree.

**Filed:** `docs/issues/archive/2026-08-17-worktree-reads-resolve-against-the-old-project.md`.

**Status:** mitigated — fix idea 3 (prefix the notice into `stdout`, not just a sibling field) landed in `src/tools/core/types.rs`'s `inject_notice`. Fix idea 2 (guard reads outright) was deliberately rejected: `worktree_read_notice`'s own doc comment already argues against refusing reads mid-orientation, and that reasoning holds. Fix idea 1 (`EnterWorktree` self-activates) remains open and lives in `codescout-companion`, a different repo — not done here.

### U-50 — Nothing surfaces concurrent writers in a shared checkout; a peer's uncommitted refactor appeared in my working tree mid-task

**Valid:** conditional — a concurrent-writer signal ships (session-start or `workspace(action="status")`), or worktree-per-session becomes the default for agent sessions

**Rests on:** `docs/trackers/archive/multi-agent-concurrent-coordination.md` — the fault line, its two prior concretes, and Option C

**When:** 2026-08-27, executing `docs/superpowers/plans/2026-08-27-operator-rules-phase-1.md` under `subagent-driven-development` in `/home/marius/work/claude/codescout`.

**Got:** I ruled against an isolated worktree — `experiments` is the repo's designated experimental branch, and a worktree costs `merge_worktree` catalog reconciliation — and dispatched a Task 1 implementer into the main checkout. Three minutes later `git status` showed two files modified that I had not touched: `src/tools/file_summary/file_summary.rs` (+147/−30) and `src/tools/markdown/edit_markdown.rs` (+12), a `HeadingQuery` occurrence-selector refactor. `ListAgents` showed **two other codescout sessions live and busy**, started 9h and 8h earlier. Nothing in the session-start surface, the activation bootstrap, the companion hooks, or `workspace(action="status")` had mentioned them.

**Cost:** the implementer's `cargo test` would have run against a tree holding a peer's half-finished refactor, so a red suite would have been unattributable — mine or theirs. Any `git add -A` would have swept their work into an operator-rules commit. The tell surfaced only because the operator overrode my ruling and instructed a worktree, which prompted a `git status` I had no other reason to run. Absent that instruction the first signal would have been a confusing test failure inside a subagent.

**Why this is a missing projection, not missing data:** `ListAgents` already reports every live session on the machine. `workspace(action="status")` reports the active project, index state and memories — not peers. `librarian(action="doctor")` reports worktree-scoped catalog rows but nothing about live sessions. The data exists one tool call away, and no surface an agent reads by default carries it.

**This is a third concrete on a fault line already scoped — and then archived while still scoping.** `docs/trackers/archive/multi-agent-concurrent-coordination.md` names the fault line, *"shared resource accessed by ≥2 concurrent agents with no transaction between observation and action"*, records two concretes (a `git reset --soft HEAD~1` race on shared HEAD; an F-N allocator collision), and offers three options — of which **Option C is worktree-per-session**, the only one whose stated scope is the fault line rather than the symptoms. Its Promote-when fires on *"a third concrete instance of the same fault line (whatever the shared resource is)"* and on *"concurrent-session rate exceeds 1 per day, sustained"*. Today supplies both: the shared **working tree** is a third resource its conventions never covered, and three codescout sessions were live at once. Its decision table's bottom row reads *"Daily, multiple collisions / week → Option C"*, against a recorded *"Today's state: 1 concurrent session pair this calendar quarter"* — a figure that is now false.

Its own Stale-when says archive when Option A, B or C ships, or when concurrent work stops for ≥3 months. Neither happened. The criteria that should have fired today stopped being swept the moment the file moved into `archive/`.

**Fix idea (cheap, independent of the structural one):** have `workspace(action="status")` and the session-start surface name other live sessions sharing this `git_root`. One line — *"2 other codescout sessions are active in this checkout"* — turns a silent race into a visible precondition, and it is a projection of data `ListAgents` already returns.

**Fix idea (structural):** restore the coordination tracker to `active` and decide Option C against its own recorded criteria, rather than re-deriving them from scratch next time.

**Leverage claim — recorded as a hypothesis, NOT as a finding.** The operator's reason for filing this is that worktree-per-session would close several partially-done problems at once. A grep for `merge_worktree|worktree-scoped|reseat_worktree|concurrent session|ListAgents` returns 254 matches across 69 files; these are the candidates that look like the same fault line:

- `docs/issues/archive/2026-08-17-prose-ledger-worktree-id-collision.md`
- `docs/issues/archive/2026-08-16-usage-db-attributes-calls-to-a-shared-session-id-file.md`
- `docs/issues/archive/2026-08-26-zombie-servers-on-deleted-binaries-stamp-stale-config-into-shared-state.md` (fixed 2026-08-30 — all four directions shipped)
- `append_entry`'s refusal to allocate ids from a worktree, documented in `get_guide("tracker-conventions")` — itself a per-resource mitigation on this line

**Nobody has checked whether these actually collapse into one fix.** Counting and classifying that set is the first task of any promotion, not an assumption to build on: a fix that names a population asserts the population is non-empty and homogeneous, and this one is so far only eyeballed.

**Status:** open
