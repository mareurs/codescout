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

### U-4 — Iron Laws triplicated in context (canonical + companion + buddy)

**When:** 2026-05-23, user-requested prompt-surface self-reflection during a `/buddy:summon pika` session. Discovered by reading `src/prompts/source.md`, `claude-plugins/codescout-companion/hooks/session-start.sh` output, and `claude-plugins/buddy/data/gates.md` side-by-side.

**Iron Law / pattern:** surface design — single-source principle. The same five Iron Laws appear in three places in the loaded context:
1. `src/prompts/source.md::server_instructions` (canonical, build-sliced — 44 lines, terse 5-bullet table).
2. `claude-plugins/codescout-companion/hooks/session-start.sh` "CODESCOUT RULES (compression-resilient reminder)" (~10 lines, bulleted; *intentionally* designed to survive compaction).
3. `claude-plugins/buddy/data/gates.md` `## Tool gates — codescout Iron Laws` (~20 lines, prose narration).

**Tool called (surface):** all three surfaces re-state the same five rules.

**Should have called:** one canonical copy. The two derived surfaces should be *pointers* ("see Iron Laws in MCP server instructions") unless they add information canonical doesn't have. Whichever copy is most likely to survive `/compact` should be the only one — currently the weakest (compression-reminder) is most compaction-resilient because SessionStart rebroadcasts on resume, which inverts the design intent of "canonical is the source of truth."

**Whistle delivered:** yes (chat U-1 in this session; promoted to this tracker entry).

**Recurrence:** 1st observed and recorded.

**Severity:** low — current copies are *consistent in content*; the cost is token bloat (~30 redundant lines in every session prefix) plus drift risk for future edits. Drift already realized in U-5, U-6.

**Status:** **by-design, not drift (revised 2026-05-23).** The three copies serve three lifecycle stages:
1. Canonical `source.md::server_instructions` — primary at MCP session init; cut at ~2 KB by Claude Code's instructions channel.
2. Companion compression-reminder (SessionStart hook) — post-`/compact` safety net; refires on session resume.
3. Buddy `gates.md` — per-specialist defense-in-depth (U-11 reduced this from full-prose to a pointer + at-a-glance cheat sheet, which is what the layer actually needs).

The triplication is correctly layered; the failure mode I worried about (drift between copies) is now substrate-prevented by **H-3** (companion-surface lint, shipped 2026-05-23). The buddy copy was simplified by **U-11** (gates.md rewrite). The remaining cost is bloat, not contradiction, and the bloat is paid in exchange for compaction-survival.



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

**Status:** fixed-shipped (code-explorer:c37bcea7, 2026-05-23). All 11 stale references updated to cite `src/prompts/source.md` plus surface names (`server_instructions`, `onboarding_prompt`). `audit_doc_refs` re-run on CLAUDE.md confirms zero matches for the old filenames. Same commit also retired the pre-archive `docs/TODO-tool-misbehaviors.md` reference in the Bug Tracking trigger rules. Audit also surfaced 20 false-positive findings (globs, template placeholders, home-paths, comma-trailing snippets) — noted as input to H-5's promotion plan (CI needs extractor FP filters before `--fail-on med`).



### U-8 — "Available shared memories" line truncates mid-name

**When:** 2026-05-23, scanning the codescout MCP `## Project Status` block delivered at session start.

**Iron Law / pattern:** progressive-disclosure design — overflow hints must be informative.

**Tool called (surface):** codescout's own `## Project Status` injection:
> *"Available shared memories: architecture, cargo-test-lib-skips-integration, conventions, development-commands, domain-glossary, gotchas, language-patterns, on… [truncated]"*

**Should have called:** either (a) full list — only ~10 memories exist, well within any reasonable budget; or (b) truncate at a comma boundary and emit `… +N more` so the model knows total count + that something remains. Mid-name `on…` discards information without naming it (the next memory is presumably `onboarding`).

**Whistle delivered:** yes (chat U-5 → this tracker entry).

**Recurrence:** 1st observed in tracker; visible at every session start.

**Severity:** low — model can recover with `memory(action="list")`, but only if it notices the truncation.

**Status:** fixed-shipped (code-explorer:2c4be270, 2026-05-23). Root cause confirmed via ADR `docs/architecture/mcp-channel-caps.md`: Claude Code's MCP client caps `initialize.instructions` at ~2 KB and appends `… [truncated]`. The line landed in the cut zone because (a) it followed the static `SERVER_INSTRUCTIONS` constant (~1.8 KB) and (b) the line itself was ~350 chars due to a wordy action-hint suffix. Fix in `src/prompts/mod.rs::build_server_instructions`: label shortened to `Memories`, action-hint suffix dropped (the memory tool's own description already documents how to call it). Bare list now fits within cap for typical projects. 2443/2443 tests still pass.

**Note for U-4 / future work:** the broader architectural issue is that the entire Project Status block lives in the cut zone. Workspace tables, custom instructions, and language warnings currently land in the dead 95% of the channel. That's Snow-Lion-class — see the ADR Open Decision for the structural recommendation.



### U-9 — Caveman SessionStart payload injected twice

**When:** 2026-05-23, session start of this conversation.

**Iron Law / pattern:** hook coalescing / harness dedup.

**Tool called (surface):** caveman plugin's SessionStart payload appears as two consecutive `<system-reminder>` blocks at session start, content near-identical (level: full both times).

**Should have called:** one copy. Either the hook runs twice (likely two SessionStart hooks registered in different profile dirs — see U-10 cross-CC-profile config drift) or the harness fails to dedupe identical SessionStart payloads.

**Whistle delivered:** yes (chat U-6 → this tracker entry).

**Recurrence:** 1st observed this session; needs cross-session confirmation.

**Severity:** low — bloat only, no semantic harm.

**Status:** open. Out of scope for codescout repo; file against caveman plugin or CC harness. Note: same root cause may underlie U-10's contradictory CLAUDE.md content (config drift between `~/.claude/`, `~/.claude-kat/`, `~/.claude-sdd/`).



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

**Status:** fixed-shipped (code-explorer:61bc678b, 2026-05-23). Two new tests added (`parser_rejects_git_refs`, `parser_handles_rust_double_colon_symbol_separator`).

**Measurement** (CLAUDE.md audit, hi-sev finding counts):
| State | Hi-sev count |
|---|---|
| Pre-FP-filter (initial discovery) | 21 |
| Post-FP-filter (0425b8ef) | 4 |
| Post-this-fix (61bc678b) | **1** (the cross-repo `claude-plugins/` ref) |

The 1 remaining hi-sev finding is a cross-repo reference to the sibling `claude-plugins/` directory. Resolving it would require either an "external roots" config on the audit, or recognizing that paths ending in `/` are dir-intent and tolerating not-locally-present. Design call for a future audit improvement, not drift to fix.


### U-16 — Pika invoker hit IL3 on first survey move (substrate caught)

**When:** 2026-05-23, post-/compact reload of the codescout-pika specialist. First exploratory git log of the new session was piped: `git log --oneline master..experiments | head -20`. The `pre-tool-guard.sh` (or its codescout-server counterpart) blocked it with the standard IL3 message; required a re-run as `git log --oneline master..experiments` (bare) followed by `grep`/`tail` on the returned `@cmd_*` buffer.

**Iron Law / pattern:** Iron Law 3 — `run_command` output piped to a log-trimmer (`| head -20`). `git log` is an unbounded-LHS command.

**Tool called (surface):** `run_command(command="git log --oneline master..experiments | head -20")` — invoked from the main Claude Code agent operating as Pika.

**Should have called:**
1. `run_command("git log --oneline master..experiments")` — bare, full output stored in `@cmd_*`.
2. `grep PATTERN @cmd_*` or `read_file(@cmd_*, start_line=..., end_line=...)` for trimming.

**Whistle delivered:** yes — self-whistle, recorded inline ("→ pika whistles: own IL3 slip on first move"). Self-correction acknowledged in the same turn.

**Recurrence:** 3rd observed IL3 slip in this tracker (after U-1, U-3). Notable that the slip came from the Pika operator itself — the agent watching for IL3 violations was the one that committed the violation. **Update: 2nd self-slip same session** — operator subsequently piped `cargo build --release 2>&1 | tail -5` later in the same exploration loop. Substrate held both times. Pattern: even after the first whistle, the watcher slipped again ≤30 turns later on a structurally identical shape (unbounded-LHS command + log-trimmer pipe). Reinforces severity assessment: discipline alone is insufficient.

**Severity:** low — substrate held. The gate caught the slip; cost was one wasted tool call + a "BLOCKED" reply turn. No code change reached disk.

**Status:** substrate-caught — no code fix required. The IL3 gate (companion `pre-tool-guard.sh` + codescout server-side check) worked as designed.

**Lesson / counterfactual (W-N material):** the IL3 substrate is now demonstrably robust against the actor that should know it best. This is evidence that gate-by-substrate beats gate-by-discipline — the rule "always whistle on IL3 violations" doesn't prevent the watcher from violating IL3; the substrate does. Reinforces the rationale for H-3 (companion-surface lint) and the worktree-write-guard test pattern: durable behavior comes from gates, not prompts.

**Note on observability:** the slip would have been invisible to the user without the BLOCKED error message in the tool result. There is no separate "Iron Law violation log" surface — the gate's deny path doubles as the observability mechanism. If the deny message were silent (e.g. swallowed by an auto-retry), this U-16 entry would not exist.


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

**Status:** partially-shipped (code-explorer:faa77dd7, 2026-05-23) — placeholder `path/to/` reject landed in `looks_like_path`, catches ~6 of 39 FPs. Reader-side path scoping (`.github/skills/`, `.vscode/mcp.json` etc., ~33 residual FPs) deferred to design call between H-6 options (B) per-doc frontmatter opt-out and (C) default scope exclusion of `docs/agents/**`.

**Measurement** (audit on docs/**/*.md, hi-sev counts):
| File | Pre-fix (61bc678b) | Post-fix (faa77dd7) | Notes |
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
