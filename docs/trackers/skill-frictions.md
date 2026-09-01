---
kind: tracker
status: active
title: Skill Frictions Tracker
owners: []
tags:
  - skills
  - frictions
---

# Skill Frictions Tracker

Running log of rough edges found while using project skills. Feed into refactor passes.

---

## `/claude-traces`

### F-001 — lf.py env auto-discovery silently fails
**When:** `lf.py session <id>` on first run  
**Expected:** Auto-load keys from `~/agents/llm-proxy/.env` (documented behavior)  
**Got:** `ERROR: LANGFUSE_PUBLIC_KEY / LANGFUSE_SECRET_KEY not set`  
**Probable cause:** Script's `.env` walk doesn't reach `~/agents/llm-proxy/.env`, or path resolution breaks when CWD isn't home  
**Fix idea:** Make the search path explicit in skill docs; add a diagnostic mode (`lf.py check-env`) that prints where it looked  
**Status: FIXED 2026-05-03** — `get_client` error message now prints key location (`~/agents/llm-proxy/.env`) and explicit env-var invocation example

**Root cause found + FIXED 2026-07-07** — the 05-03 fix only improved the error message; the real bug was `load_env()` returning after the *first existing* `.env`. Any project with its own `.env` (e.g. codescout's, holding only `CARGO_REGISTRY_TOKEN`/`CODESCOUT_*`) shadowed `~/agents/llm-proxy/.env` so the `LANGFUSE_*` keys never loaded. Fix (in `~/agents/llm-proxy/.claude/skills/claude-traces/scripts/lf.py`, symlinked into codescout): load **all** candidate `.env` files with `os.environ.setdefault` merge semantics — earlier candidates still win per-key, real env vars win over files. Verified: `lf.py recent` now works from codescout cwd. Surfaced during Fable-tuning FT-8.

### F-002 — cc.py stats fails silently on cross-project sessions
**When:** `cc.py stats 64618681-de62-4bf7-abad-0e0d93de005a`  
**Expected:** Find session JSONL and return token/cost summary  
**Got:** `ERROR: session not found`  
**Probable cause:** `cc.py` only scans the current project dir by default; session may live in a different project  
**Fix idea:** Skill should instruct: run `cc.py sessions --all` first to locate which project owns the session before running `stats`/`trace`; or add auto-fallback to `--all` on not-found

### F-003 — No guidance on env setup prerequisite
**When:** Starting any Langfuse workflow  
**Observation:** Skill docs describe key locations but don't surface a "setup check" step. New users will hit F-001 first and not know why.  
**Fix idea:** Add a `## Prerequisites` section at the top: "Run `lf.py check-env` (or equivalent) to verify keys resolve before starting analysis."

### F-004 — `cc.py sessions --all` permission friction
**When:** Trying to locate session across all projects  
**Observation:** `--all` scans broadly (`~/.claude/projects/`); triggers user confirmation in some permission modes  
**Fix idea:** Skill should note this as a known prompt; suggest narrowing with `--project` when the project is known

---


### F-005 — `lf.py find` prints 12-char truncated IDs; `lf.py trace` needs full UUID
**When:** Running `lf.py trace <id>` with the ID copied from `lf.py find` output  
**Got:** `404 Client Error: Not Found for url: .../api/public/traces/12c504ad-d66`  
**Root cause:** `cmd_find` prints `t['id'][:12]` (line ~201 in lf.py), but `cmd_trace` passes the value directly to `GET /api/public/traces/{id}` which requires full UUID  
**The footer hint says:** `→ Drill down: lf.py trace <full_trace_id>` — but `full_trace_id` was never shown  
**Skill doc says:** "Use the short trace_id prefix shown to pick which call to inspect" — wrong; API doesn't support prefix matching  
**Fix options:** (a) print full UUIDs in `find` (widen column), (b) make `trace` do a prefix lookup via `GET /traces?sessionId=...` and filter client-side  
**Status: FIXED 2026-05-03** — `cmd_find` now prints full 36-char UUIDs; column widened to 36; footer updated to "copy as-is"

### F-006 — `lf.py trace` "Tools (N)" shows available schema, not actual calls
**When:** Reading `lf.py trace` output to understand what tools were used  
**Got:** `Tools (34): Agent, AskUserQuestion, ...` — lists all 34 tools in the schema  
**Expected:** Which tools were actually called in that API turn  
**Root cause:** Proxy logs `output: { text: "" }` — tool_use blocks are stripped from logged output. Tool names aren't recoverable from Langfuse observations alone.  
**Workaround:** Use `cc.py tool-calls <session_id> --project <path>` for actual sequence  
**Fix idea:** Proxy should log tool_use block names (not inputs) into observation metadata, e.g. `tools_called: ["mcp__codescout__symbols", "TaskCreate"]`

### F-007 — Session belongs to different project; cc.py stats/trace fails silently
**When:** `cc.py stats 64618681-...` from codescout project  
**Got:** `ERROR: session not found`  
**Root cause:** cc.py scans only the current project's JSONL by default; session was in `/home/marius/work/mirela/backend/kotlin`  
**No guidance** in skill on how to locate session → user must either know the project or run `--all`  
**Fix idea:** On not-found, auto-suggest `cc.py sessions --all | grep <session_prefix>` as next step; or add `cc.py locate <session_id>` that scans all projects and returns the owning path


### F-008 — cc.py path decoding ambiguous: `-` in dir names decoded as `/`
**When:** `cc.py sessions --all` shows `project: /home/marius/work/mirela/backend/kotlin`  
**Reality:** Actual path is `/home/marius/work/mirela/backend-kotlin`  
**Root cause:** JSONL encodes project paths as `~/.claude/projects/<path-with-slashes-as-dashes>/`. cc.py reverses this by replacing `-` with `/`, but directory names that contain `-` (e.g. `backend-kotlin`) are indistinguishable from path separators.  
**Impact:** Any `--project` flag or `cc.py stats/trace` call built from `--all` output will have the wrong path and fail silently.  
**Fix idea:** cc.py should verify the reconstructed path exists; if not, try heuristics (longest existing prefix). Also document the ambiguity in skill docs so users know to verify before using a `--project` path from `--all` output.  
**Status: FIXED 2026-05-03** — `project_key_to_path` now uses filesystem-guided bitmask decode: tries all `-`-vs-`/` splits ordered by most separators first, picks first path that exists on disk

### F-010 — artifact(update, rel_path) updates metadata but doesn't rename file on disk (FIXED 2026-05-23, codescout:1cb123d1)
**When:** `artifact(action="update", patch={rel_path: "new/path.md"})` after manually moving a file  
**Got:** Artifact metadata updated (confirmed `"updated": true`), but file stays at old path  
**Impact:** Subsequent `edit_markdown(path="new/path.md")` fails with "No such file or directory"  
**Fix:** `update` now rejects `patch.rel_path` with a RecoverableError hinting at `artifact(action="move", id=..., new_rel_path=...)`. Two-call APIs must reject the wrong input shape explicitly, not accept silently — silent divergence is the worst failure mode here because `updated: true` reads as proof of action. Test: `update_rejects_rel_path_with_move_hint` in `src/librarian/tools/update.rs`. The `move` action (`src/librarian/tools/mv.rs`) covers the file-rename use case atomically.

### F-011 — cc.py hardcodes `~/.claude`; sessions from other profiles invisible
**When:** 2026-07-10, inspecting session fc0e9019 (a `~/.claude-kat` session) with `cc.py stats/tool-calls`.
**Got:** `ERROR: session not found` — `CLAUDE_DIR = Path.home() / ".claude"` is hardcoded (cc.py:23), so `~/.claude-sdd` / `~/.claude-kat` sessions can't be inspected. This machine runs three profiles by design.
**Workaround:** sed-copied cc.py to scratchpad with the profile dir patched.
**Fix idea:** honor `$CLAUDE_CONFIG_DIR` (or add `--claude-dir`), and let `sessions`/`stats` fall back to globbing all three known profile roots when the session id isn't found in the default.
**RECURRED 2026-08-15** (2nd occurrence, ~5 weeks later) — restoring kat session `4ba7e23c` from a `~/.claude-sdd` session. Same hardcode, same workaround cost. Escalating: this is not a one-off, it is structural on a three-profile machine, and the skill's own description advertises profile identification ("which CC profile (~/.claude vs ~/.claude-sdd) made a request") — so the documented capability is *unreachable* for two of the three profiles via `cc.py`.
**Better workaround than sed-copying (use this one):** a wrapper importing cc.py as a module and repointing the globals — `PROJECTS_DIR` is read at call time, so no source patch is needed and the shared symlinked script stays untouched:
```python
import sys; sys.path.insert(0, "<skill>/scripts"); import cc
cc.CLAUDE_DIR = Path(profile); cc.PROJECTS_DIR = Path(profile) / "projects"
sys.argv = ["cc.py"] + sys.argv[1:]; cc.main()
```
**Second gap found on this pass:** `cc.py trace` truncates message bodies to ~200 chars, so the intent thread of a long session is not recoverable from it — the compaction-summary and multi-paragraph user prompts are exactly the high-value turns, and they are exactly the ones cut. Restoring a session needs an untruncated per-role message dump; there is no cc.py subcommand for it. **Fix idea:** add `cc.py messages <session_id> [--role user] [--since TS] [--full]`.
## `/analyze-usage`

### F-005 — `find ~/work` assumption not portable
**When:** Step 1 (Discover DBs)  
**Observation:** Skill hardcodes `~/work` as standard project root with a note to adjust — but doesn't tell the model HOW to detect the right root. If the user's projects live elsewhere, step 1 produces zero results with no actionable error.  
**Fix idea:** Add discovery fallback: check `git rev-parse --show-toplevel`, then `~/work`, then `~` — or ask user for root on zero results

### F-006 — No per-session filter mode
**When:** User wants to analyze a specific session (as in this conversation)  
**Observation:** Skill is report-only (all-time). There's no way to scope queries to a session_id even though `tool_calls.session_id` exists.  
**Fix idea:** Add `/analyze-usage session <id>` mode that runs the same queries with `WHERE session_id=?`

### F-007 — Skill doesn't coordinate with claude-traces
**When:** User asks for session-level efficiency analysis  
**Observation:** `/analyze-usage` covers usage.db (codescout-side), `/claude-traces` covers JSONL+Langfuse (Claude-side). Neither skill mentions the other or describes how to combine them for a full picture.  
**Fix idea:** Add a "See also" cross-reference in both skills; document the complementary data model (usage.db = tool call metrics, Langfuse = token/cost/tool sequence)


### F-009 — analyze-usage operates in isolation from claude-traces
**Observation:** `/analyze-usage` scans usage.db (codescout-side: tool call counts, latency, errors) but has no awareness of `/claude-traces` (Claude-side: token cost, stop reasons, actual tool sequences from JSONL/Langfuse). A full session audit requires both — usage.db tells you *what codescout saw*, JSONL/Langfuse tells you *what the model decided*.  
**Current state:** Neither skill references the other. A user wanting session-level efficiency analysis has to manually combine them.  
**Direction:** `/analyze-usage` should be the driver — it owns the audit workflow. It should:
1. Run its SQL queries as today
2. For sessions of interest (high error rate, high tool count), call into `/claude-traces` to pull the actual tool sequence and token cost
3. Synthesize both into a unified verdict (efficiency + correctness)  
**Fix idea:** Add a `## Cross-referencing with session traces` section to the analyze-usage skill that explains when and how to invoke `cc.py tool-calls` + `lf.py session` for drill-down, and what signals from usage.db should trigger the drill-down (e.g. sessions with >50 calls, error rate >10%, or overflows).

### F-008 — Skill doesn't mention librarian for tracker creation
**When:** User asked to create a tracker for grep usage patterns  
**Got:** File created manually with `create_file` instead of `artifact(action="create", kind="tracker")`  
**Prompt gap:** Neither `/claude-traces` nor `/analyze-usage` skill mentions that trackers should go through the librarian. A one-liner "create any tracker via `artifact(action=create, kind=tracker)` — call `librarian(tracker_design)` first" would prevent this.


### F-010 — Step-2 query-battery output overflows; natural `grep | sed` post-processing trips the companion IL3 gate
**When:** Step 2 (per-DB SQL queries), running under the `codescout-companion` PreToolUse hook (the normal dev environment for this repo).
**Observation:** The documented invoke pattern loops `sqlite3 -line "$db" "..."` across every DB. With ~10 active DBs the combined output exceeds the inline budget and lands in a `@cmd_*` buffer (440+ lines buffered). The obvious next step — `grep -E "..." @cmd_xxx | sed 's/^ *//'` to extract the per-DB error rows — trips the IL3 advisory ("piped `grep` to a log-trimmer"), re-buffers the result, and truncates it again, forcing a fallback to `cat @cmd` + multiple `sed -n 'A,Bp' @cmd` paging calls. Net: ~3 extra round-trips per analysis to read data the skill already produced.
**Got:** Skill Step 2 says nothing about (a) expecting overflow on multi-DB loops, or (b) that buffered output must be paged with a single bounded-LHS command (`sed -n`, `cat`, bare `grep @ref`) — never a chained pipe to `sed`/`head`/`tail`, which the companion gate blocks.
**Fix idea:** Add a note to Step 2: "Multi-DB loop output overflows into a `@cmd_*` buffer. Page it with `sed -n 'N,Mp' @cmd_id` or bare `grep PATTERN @cmd_id` — do NOT chain `| sed`/`| head`/`| tail`, which the codescout IL3 gate blocks. Or scope each query tighter (single DB, `LIMIT`, date filter) so results fit inline." Pairs with the existing buffer guidance in `get_guide("progressive-disclosure")`.
**Note:** The *cross-project* IL3 pipe-to-`head` recurrence seen in the usage data (deployment / claude-plugins / researcher piping `git log | head`, `find | head`) is a **tool-usage pattern**, not a skill friction — track that as a T-N in `docs/trackers/tool-usage-patterns.md`, not here.

## `superpowers:subagent-driven-development`

### F-012 — the finish step deletes every per-task report

**When:** Completing a ten-task SDD run (`get-guide-section-grain`, 2026-08-27).

**Got:** The skill's Finish step says *"delete this plan's workspace (`rm -rf <workspace>`) —
the git history is the record now."* That is true of the **code** and false of everything
else the run produced: ten task reports with their TDD evidence, twelve review reports, and
a progress ledger holding 31 rulings and 15 deferred minors. None of it is in git. The step
is unconditional, so following it destroys the run's entire reasoning trail.

The gap is sharpened by the skill's own instruction two paragraphs earlier — collect every
`Ruling:` line into the final message before deleting. That correctly identifies that the
ledger holds something git does not, then rescues one of the several things it holds.

**Counterfactual, and it is not hypothetical:** in this run the *final reviewer* flagged that
evidence kept only in the run ledger would be destroyed at finish, and asked for it to be
folded into a committed bug file. The controller did exactly that — then ran the deletion
step ten minutes later and destroyed the same class of evidence one directory over. A
reviewer identified the failure mode, the controller acted on the instance, and the pattern
still fired.

**Fix idea:** make the finish step conditional on preservation rather than unconditional.
Either (a) commit a distilled residue before deleting — rulings, deferred minors, and any
recommendation a report makes — or (b) move the workspace under a committed path for
finished runs. codescout now does (a) via `docs/trackers/sdd-ruling-log.md`, seeded from
this run; the upstream skill still says delete.

**Status:** open — local mitigation in place (`sdd-ruling-log.md` + a CLAUDE.md append
trigger); upstream skill unchanged.

### F-013 — `review-package` BASE is easy to get silently wrong when anything commits out-of-band

**When:** Same run. A bug file was committed between one task's head and the next task's
commit.

**Got:** Nothing warns you. Packaging the next review from the recorded BASE would have put
an unrelated 5 KB documentation commit inside the diff a reviewer was asked to judge —
spending a review seat partly on prose it has no business reviewing, and diluting the code
under review. The skill is right to insist on recording BASE explicitly rather than using
`HEAD~1`, but recording it once at dispatch is not enough: any out-of-band commit
invalidates it.

**Fix idea:** re-derive BASE at packaging time as "the head the previous review saw" rather
than trusting the value recorded at dispatch, or have `review-package` warn when the range
contains commits by an author/subject outside the task.

**Status:** open — worked around by hand this run (BASE moved deliberately, recorded as a
ruling).
## `/onboarding`

### F-001 — workspace onboarding silently over-reported per-project memory writes
**When:** Multi-project workspace with `force=true`. HARD-GATE only verified `project-overview` per project, allowing subagents to pass with 2 of 6 memories.
**Got:** Final summary claimed 6/6 coverage; in reality some projects had 2–3 memories.
**Fix idea (FIXED 2026-05-07):** Phase 4 Coverage Verification reads back all 6 topics per project; subagent MANIFEST line is advisory only.

### F-002 — onboarding root-layer content not captured
**When:** Monorepo with real root-layer code (dev scripts, docker-compose, top-level scripts).
**Got:** Workspace prompt explicitly forbade a root subagent and had no fallback to capture root content.
**Fix idea (FIXED 2026-05-07):** workspace `architecture` template grew Top-Level Code Map + Generic Navigation subsections; the no-root-subagent rule now states the reason.


## `/superpowers:writing-plans` + `/codescout-companion:reconnaissance`

### F-001 — writing-plans writes test assertions naming types without scouting them; recon's triggers don't catch this

**When:** Plan `docs/superpowers/plans/2026-05-18-jsonpath-negative-slice.md`
written via the writing-plans skill, then re-scouted via `/codescout-companion:reconnaissance`
before subagent dispatch. Recon caught a defect that the plan-writing phase
had baked in.

**Got:** Plan's Task 2 + Task 3 tests asserted on `err.hint.as_deref()` — a
field that doesn't exist. The actual `RecoverableError` (at
`src/tools/core/types.rs:169`) has `pub message` + `pub guidance:
Option<Guidance>` + a `.hint()` *method* on the impl block. The Display
impl's own comment documents `to_string().contains(...)` as the canonical
test-assertion form. The plan-writing skill never read the type — it
inferred the shape from the design spec, which itself didn't pin the
assertion form.

**Two-skill tension:**

1. **writing-plans** has no "scout types named in test assertions" step.
   Its self-review checklist covers placeholders / type consistency /
   spec coverage — but `type consistency` only catches inconsistencies
   *within* the plan, not between the plan and the codebase.
2. **reconnaissance** lists triggers like "before subagent dispatch" and
   "before editing code that changes a struct, function signature, or API
   contract" — but does NOT list "before writing a plan that asserts on a
   type the planner has not read". If the user hadn't re-invoked recon
   between writing-plans and subagent dispatch, F-3 (in
   `docs/trackers/bug-fix-session-log.md`, 2026-05-18) would have surfaced
   as the first subagent's compile error.

**Fix ideas (both surfaces are candidates):**

- **In writing-plans:** add a pre-write step "for every type T whose
  accessors appear in plan test code, `symbols(name=T, include_body=true)`
  once; cite the file path + exposed accessors in a footnote of the task
  that names T". Tightens Section 9 (Self-Review) "type consistency" to
  *external* consistency.
- **In reconnaissance:** add a `When to Use` bullet — "before writing
  test code in a plan, scout every type named in an assertion". Recon's
  current model is "at the seam"; this expands it to "before the seam
  becomes a plan token".
- **Composition fix:** writing-plans could declare reconnaissance as a
  REQUIRED SUB-SKILL for any plan whose tasks include test code that
  asserts on a type's accessors. Currently the cross-skill linkage is
  user-mediated.

**Severity:** med — caught this round, cost ~6 tool calls. Without the
user's mid-plan recon invoke, would have cost a failed subagent task +
controller drift mid-dispatch.

**Confirming data:** F-3 + W-2 in `docs/trackers/bug-fix-session-log.md`
(2026-05-18). Both surface the same gap from different angles — F-3 is
the drift, W-2 is the win-from-catching-it. The fact that both could be
*one* friction in the right place (writing-plans pre-write scout) and
not just a recon-saves-the-day story is the substantive complaint.

## `/codescout-companion:reaching-peer-sessions`

### F-001 — the trigger condition was observed, said out loud, and the skill still went uninvoked

**When:** 2026-09-01, a full session of cross-session coordination on a shared checkout
(codescout-17). `ListAgents` was called three times and reported 2–3 peers. A peer's
`file-provenance.py` run named two writing sessions that were **not in that list**. I stated
that discrepancy explicitly, in writing, to two peers — *"neither of those appears in my
`ListAgents` … either they have ended, or this is `BL-58` under-reporting again"* — and then
reasoned onward from the short list anyway.

**Got:** the skill's own `description` names this exact state as a trigger: *"…or when
`ListAgents` returns fewer peers than expected."* It was in my available-skills list from the
first turn of the session. It went uninvoked through:

- three peer messages routed on the incomplete set (a broadcast that reached **two of the five
  peers** in this checkout);
- a fifth instance filed in `docs/issues/2026-09-01-un-wired-function-reds-the-shared-build-with-no-author.md`
  concluding *"positive identification is unavailable"*;
- a paragraph promoted into `CLAUDE.md` § *Observer Blindness* stating that as a rule.

All three were wrong in the same way, and the operator caught it by asking whether a skill for
this existed. Running Step 1 took one call and returned **16 live sessions across 3 profiles**,
with **six** whose `cwd` is this checkout — five peers plus this session — against `ListAgents`'
2. Discovery is per-profile (`$CLAUDE_CONFIG_DIR/sessions/*.json`); delivery is per-user
(`/run/user/<uid>/cc-socks/`). The three sessions I could not see were reachable the whole time.

**Root cause — not a discoverability gap.** The skill is well-named, its description is precise,
and its trigger is a condition I *observed and articulated*. Nothing was missing except the act
of invoking it. So this is not "the skill needs a better description": it is a trigger that
requires the model to notice a state and then choose to act, with nothing in the environment
tying the two together. **A skill whose trigger is a condition the model must notice is a policy,
not a mechanism** — precisely the distinction `CLAUDE.md` § *Observer Blindness* draws, and I
committed this while editing that section.

**Impact:** high for correctness, low for cost. Three published claims required retraction, one
of them from `CLAUDE.md`. No peer was harmed — the broadcast recipients each spent a turn
establishing a negative — and the underlying issue was closed by its own author unprompted.

**Fix idea (mechanism, not exhortation):** the only place the precondition is observable at the
moment it matters is the `ListAgents` response itself. Have the companion's post-tool hook
annotate that response with the socket-scoped profile count whenever it exceeds the returned row
count — e.g. `[cs-hint] ListAgents is per-profile: 2 shown, 16 live across 3 profiles; run
/codescout-companion:reaching-peer-sessions`. That converts a trigger the model must remember
into a fact the response carries. Tracked as a candidate `H-N` in
`docs/trackers/codescout-usage-hookify.md`.

**Secondary fix idea (cheap, partial):** the skill's own § *Two readings to get right* already
says *"report the scope you actually searched … say which profile it covered rather than
presenting it as the population."* That is the right rule and it is inside the skill — i.e.
reachable only after invoking it. Consider hoisting one line of it into the `description`, so the
per-profile caveat is visible in the skill *list* rather than only in the body: a reader who
never invokes the skill currently never learns that `ListAgents` is a subset.

## `/buddy:summon`

### F-001 — Summon protocol assumes native Bash/Read; codescout-companion hard-denies both
**When:** 2026-06-11, `/buddy:summon hamsa` in the codescout project. The SKILL protocol steps say to use the Bash tool (discover-specialists.sh, track_specialist.py, summons.log append) and the Read tool (SKILL.md, lens addenda, memories, memory-protocol.md, gates.md).
**Got:** codescout-companion hard-denies native `Bash` and native `Read` on this project (and `read_file` on `.md` via IL4). Every step had to be translated: `run_command` for the scripts, `cat` for the foreign-repo markdown (SKILL.md / memory-protocol.md / gates.md), `printf >> summons.log`. It worked, but a less-adapted agent would stall on the first blocked Bash/Read call.
**Fix idea:** Have the buddy:summon protocol name codescout-aware fallbacks (if native Bash/Read are gated, use run_command / cat / read_markdown), or detect a codescout-companion project and emit the adapted tool list up front. Low urgency — the adaptation is mechanical once known.
