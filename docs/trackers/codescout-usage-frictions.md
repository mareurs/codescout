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

**Status:** open — promoted to H-1.

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

**Status:** open — promotion candidate to H-2.

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

**Status:** open — pattern recurs across sessions despite Pika warnings.

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

**Status:** open. Promotion candidate to **H-4** (drop companion compression-reminder once main-session server-instructions are proven to survive compaction).

**Notes:** the buddy `gates.md` copy is the easier kill — it's pure prose narration with no compaction-survival role. The companion copy *does* serve a real purpose (subagent inheritance, compaction-survival), so it stays as the most-likely-to-survive copy if any is dropped.



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

**Status:** open. Fix is a two-line edit to `hooks/session-start.sh`: restore the bounded-LHS exception text.



### U-6 — Compression-reminder cites stale codescout tool names

**When:** 2026-05-23, comparing companion SessionStart text to the live MCP tool registry.

**Iron Law / pattern:** project prompt-surface consistency rule (CLAUDE.md § "Prompt Surface Consistency"). Direct repeat of the "distance-from-change" failure mode documented in that section.

**Tool called (surface):** companion `hooks/session-start.sh` line:
> *"Code edits: replace_symbol/insert_code/remove_symbol, NOT edit_file/Edit for structural changes"*

**Should have called:** `edit_code` (single consolidated tool with `action="replace"|"insert"|"remove"|"rename"`). The three named handles (`replace_symbol`, `insert_code`, `remove_symbol`) do **not** exist as MCP tool handles in the current binary. Confirmed against the tool registry available in this session — only `mcp__codescout__edit_code` is registered.

**Whistle delivered:** yes (chat U-3 → this tracker entry).

**Recurrence:** 1st observed in this surface; pattern-wise it's the second documented instance of "distance-from-change" tool-name drift (the first lived in repo-side surfaces and was caught by `server::tests::prompt_surfaces_reference_only_real_tools`, prompting the lint).

**Severity:** **high** — the model will attempt to call non-existent tools. Each call hits "unknown tool", forcing recovery and round-trip waste. Worst-failure variety of prompt drift; exactly what the project's lint exists to prevent — except the lint does not cover companion-plugin hooks (companion lives in a sibling repo).

**Status:** open. Two fixes needed:
1. Edit `hooks/session-start.sh` to cite `edit_code` (single name).
2. Promote **H-3** to extend the lint to cover companion-hook output.



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

**Status:** open. Fix: edit CLAUDE.md § Prompt Surface Consistency to cite `source.md` plus the surface-marker mechanism. Run `librarian(action="audit_doc_refs", paths=["CLAUDE.md", "docs/**/*.md"], fail_on="med")` in the same pass to surface any other drift. Cross-reference **H-5** for CI promotion.



### U-8 — "Available shared memories" line truncates mid-name

**When:** 2026-05-23, scanning the codescout MCP `## Project Status` block delivered at session start.

**Iron Law / pattern:** progressive-disclosure design — overflow hints must be informative.

**Tool called (surface):** codescout's own `## Project Status` injection:
> *"Available shared memories: architecture, cargo-test-lib-skips-integration, conventions, development-commands, domain-glossary, gotchas, language-patterns, on… [truncated]"*

**Should have called:** either (a) full list — only ~10 memories exist, well within any reasonable budget; or (b) truncate at a comma boundary and emit `… +N more` so the model knows total count + that something remains. Mid-name `on…` discards information without naming it (the next memory is presumably `onboarding`).

**Whistle delivered:** yes (chat U-5 → this tracker entry).

**Recurrence:** 1st observed in tracker; visible at every session start.

**Severity:** low — model can recover with `memory(action="list")`, but only if it notices the truncation.

**Status:** open. Locate the emitter by `grep "Available shared memories"` in `src/`; likely in `src/server/...` Project Status assembly. Two-line fix.



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

**Status:** open. Two-line edit to `~/.claude-kat/CLAUDE.md`. Cross-reference U-9 (same root cause: cross-profile config drift).



### U-11 — Buddy `gates.md` re-narrates Iron Laws in prose

**When:** 2026-05-23, Pika summon loaded `claude-plugins/buddy/data/gates.md` per the summon protocol.

**Iron Law / pattern:** redundancy with canonical surfaces (see U-4).

**Tool called (surface):** `claude-plugins/buddy/data/gates.md` § "Tool gates — codescout Iron Laws" — ~20 lines of prose narration of the same five laws already canonical in `source.md::server_instructions`.

**Should have called:** be a *pointer* — "see canonical Iron Laws in MCP server instructions" — and add only what canonical doesn't cover: workspace gate semantics, hooks behavior, role-gate context. Prose narration of rules that already exist in tabular form a few hundred tokens away is pure cost.

**Whistle delivered:** yes (chat U-8 → this tracker entry).

**Recurrence:** 1st.

**Severity:** low — bloat only; no contradiction with canonical.

**Status:** open. Drift-prevention edit to buddy plugin (`buddy/data/gates.md`). Easier kill than U-4's companion copy because buddy's gates have no compaction-survival role.



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

**Status:** open. Fix requires:
1. Edit `hooks/hooks.json` matcher (line 25).
2. Edit `hooks/worktree-write-guard.sh` case statement (line 19).
3. Add or update worktree test coverage (the repo already has `worktree-write-guard.test.sh` per the earlier `tree` listing — verify it asserts firing on `edit_code`).
4. Re-confirm via a deliberate trigger scenario (set `.cs-worktree-pending`, attempt an `edit_code` call, confirm hook fires + blocks).

Deferred from the U-5/U-6 commit because the risk shape differs: text fixes are non-functional; matcher fixes are runtime-behavioral and warrant their own commit + test verification.

**Cross-reference:** root cause is the same gap as **H-3** (companion-side tool-name lint missing). Filing U-14 reinforces H-3's promotion criterion — this is the second confirmed instance of companion-side stale-tool-name drift, which trips H-3's promote-when threshold.
