---
kind: tracker
status: active
title: Codescout Usage Hookify Candidates — H-N Log
owners: []
tags:
  - pika
  - hookify
  - promotion-candidates
---

# Codescout Usage Hookify Candidates — H-N Log

Patterns observed across U-N entries that earn substrate enforcement.
Format from `~/.claude/buddy/skills/codescout-pika/SKILL.md` § Tracker
Format. Each H-N is gated by `Promote-when` before graduating to a real
`/hookify` rule.

---

### H-1 — Deny piped `run_command` (warn first)

**Pattern:** `run_command` invoked with a shell command whose body matches
`\| (head|tail|wc|grep)\b` (and likely also `awk|sed`). The pipe filters
MCP `run_command` output instead of using the `@cmd_*` buffer system.

**Confirming data:**
- **U-1** — 45 slips in one session (`753e9a4a`), single-shape predicate.
  Backing rows: `pika_observations.cc_session_id='753e9a4a-a81f-4cf2-aeaa-a3877d35d1ce'`
  AND `subkind='iron_law_3'` (45 rows; originally 50, 5 self-matches
  retroactively deleted 2026-05-17 — see U-1 *Post-cleanup note*).
- **Smoke-scan observational** — 3090 historical pipe-shaped `run_command`
  calls in `.codescout/usage.db` across the whole project, recorded in
  `docs/trackers/pika-phase-1-validation.md`. This is observational
  (no per-call judgment), not verdict-bearing. Used only as
  cross-session shape confirmation, not as the sole basis for promotion.

**Proposed hookify rule:**

- **Predicate:** tool name `run_command`, command body regex
  `\|\s*(head|tail|wc|grep|awk|sed)\b`.
- **Decision:** `warn` (not `deny` at first ship — pipes are legitimate
  inside `bash -c "…|…"` script bodies; deny would punish script-internal
  pipelines that have nothing to do with Iron Law 3).
- **Reason text:** *"Iron Law 3: `run_command` output piped to a filter.
  Run the command bare and query the returned `@cmd_*` buffer in a
  follow-up call (e.g. `grep FAILED @cmd_abc`). The buffer system exists
  to save context — use it."*

**Promote-when:**
- A second user-asked scan (different session) writes ≥10 IL3 slip rows
  with the same predicate shape, AND
- The `warn` rule has shipped and run for ≥1 session without false-positive
  complaints on script-internal pipes; then promote `warn` → `deny`.

**Status:** **shipped (deny) — 2026-05-18.**

**Promotion evidence:**
- U-1: 45 strikes in one session (session `753e9a4a`), warn-mode caught all.
- U-3: 9 strikes across this session (2026-05-18) despite explicit Pika
  warnings on each. Warn-mode failed to change behavior within a single
  long session — the buffer-query habit did not stick.
- Cumulative ≥50 slip rows across 3 sessions matches the strict
  ≥10-cross-session-rows criterion (52 > 10). FP rate under warn:
  zero documented complaints over multiple weeks of shipping.
- Deny hook tested locally before swap: positive case emits
  `permissionDecision: "deny"`, jq/yq pipes silently allowed,
  no-pipe commands silently allowed.

**Hook details:**
- File: `claude-plugins/codescout-companion/hooks/il3-deny-hook.sh`
  (copy of the warn variant with `additionalContext` → `permissionDecision:
  "deny" + permissionDecisionReason`).
- `hooks.json` PreToolUse matcher `mcp__.*__run_command` now points at
  the deny script.
- Warn variant (`il3-warn-hook.sh`) preserved in git history for
  emergency revert; not registered in `hooks.json`.

**Notes:**
- The 45-row evidence covers 8 command families (`git`, `find`, `cargo`,
  `ls`, `grep`, `cat @<buffer>`, `diff`, other) — the predicate is
  command-family-agnostic, which means a single regex catches all of them
  without per-family tuning. (`sqlite3` was a 9th family pre-cleanup but
  all 5 of its rows were Pika self-matches and were deleted.)
- 2 of the 45 (cat-buffer family) already use a `@file_*` reference but
  then pipe its content through `jq | wc -c` or `jq | head`. The hookify
  rule still applies — the violation is the trailing pipe, not the input.


---

### H-2 — Deny `read_file` on `.md` (direct deny, no warn stage)

**Pattern:** `read_file` invoked with a path ending in `.md`. Already
hard-rejected by the in-server tool gate (`"Use read_markdown for
markdown files"`), but the rejection costs a tool round-trip + leaves
a row in `tool_calls`. Hookify catches it pre-call.

**Confirming data:**
- **U-2** — 3 same-turn slips in session `42874b1a`, all blocked by
  the in-server gate. Backing rows:
  `pika_observations.subkind='read_file_markdown'` (3 rows).
- **Cross-session shape confirmation (deferred):** no second-session
  data yet. H-2 stays `proposed` until a second session writes ≥1
  more `read_file_markdown` slip.

**Proposed hookify rule:**

- **Predicate:** tool name `read_file`, `path` matches regex `\.md$`.
- **Decision:** `deny` straight off (skip the `warn` stage that H-1
  used). Justification: the in-server tool gate *already* hard-rejects
  this — there is no legitimate `read_file(*.md)` call. `warn` is
  redundant; `deny` saves the round-trip.
- **Reason text:** *"Markdown files must use `read_markdown(path)` —
  heading-addressed, size-adaptive, slice-able. `read_file` on `.md`
  is hard-rejected by the in-server gate; calling it costs a wasted
  round-trip and a `tool_calls` row. Use `read_markdown` first try."*

**Promote-when:**
- A second user-asked scan (different `cc_session_id`) writes ≥1 more
  `read_file_markdown` slip row, confirming the pattern is not
  session-local quirk. (Lower bar than H-1's ≥10 because the in-server
  gate already certifies the predicate is universally invalid.)

**Status:** proposed.

**Notes:**
- Asymmetry with H-1: H-1 started `warn` because pipes are legitimate
  inside `bash -c "…|…"` script bodies. H-2 has no analogous
  false-positive — `.md` is `.md`. Direct-deny is correct first ship.
- Same-turn recurrence (3 slips in one turn) is the dominant signal,
  not cross-session count. The model did not learn from the first
  in-server rejection within the turn — memory route too slow;
  substrate route required.



---

### H-3 — Lint must cover companion plugin surfaces for stale tool names

**Pattern:** any token in `claude-plugins/codescout-companion/hooks/*.sh` (or other companion text surfaces) that *looks like* a codescout tool name must resolve to a real tool in the current binary. The project's existing `prompt_surfaces_reference_only_real_tools` lint covers `source.md` + `builders.rs` but **not** the companion-plugin surfaces — companion lives in a sibling repo and is rendered into context via hook output at session start.

**Confirming data:**
- **U-6** — companion `hooks/session-start.sh` cites `replace_symbol` / `insert_code` / `remove_symbol`, none of which are registered tool handles. Real handle is `edit_code` (consolidated). Direct drift caused by the gap in lint coverage. **Fixed in claude-plugins:bd20a8a (2026-05-23)** for the text-drift surface only; the lint extension that would have prevented this remains unbuilt.
- **U-14** — same root cause, second surface: `hooks/hooks.json:25` matcher + `hooks/worktree-write-guard.sh:19` case statement alternate over four nonexistent tool handles. Runtime safety failure (modern write tools slip past the worktree-write-guard silently). **Open** — pending matcher-fix commit + worktree test coverage.
- **Cross-reference:** project CLAUDE.md § "Prompt Surface Consistency" already documents the "distance-from-change" problem this lint exists to prevent. The lint just hasn't followed the surface to the companion repo yet.

**Promote-when criterion now satisfied (2026-05-23):** two confirmed instances of companion-side stale-tool-name drift in two different surface types (text + matcher). The lint extension should be drafted and landed.

**Proposed hookify rule:**

- **Predicate:** post-build CI step that captures the rendered output of companion hooks (`session-start.sh`, `subagent-guidance.sh`, `semantic-tool-router.sh`) and lints any token matching the regex `\b[a-z_]+(_symbol|_code|_file|_markdown)\b` against the live MCP tool registry.
- **Decision:** `deny` (CI fails on unknown handle).
- **Reason text:** *"companion hook references nonexistent codescout tool `<name>` — confirm against the live MCP tool registry (`cargo run -- list-tools` or equivalent) or update the hook to cite a real handle."*
- **Implementation paths:**
  1. *In codescout repo*: extend `server::tests::prompt_surfaces_reference_only_real_tools` to ALSO read companion hook scripts from a configured path (env var `COMPANION_PATH` or workspace sibling lookup). Best place to run: pre-publish.
  2. *In companion repo*: add a CI step that clones codescout, builds it, dumps tool names, and lints `hooks/*.sh` against the dump. Best place to run: pre-merge in companion.

  Both are valid; (2) is more decoupled (companion owns its own lint) but requires companion CI to build codescout. (1) is more centralized but couples the two repos.

**Promote-when:** lint extension is drafted and ready to land; OR a second instance of companion-side stale-tool-name drift surfaces (whichever comes first). Current threshold: 1 confirmed (U-6); a second instance would force the issue.

**Status:** proposed.

**Notes:** the existing repo-side lint (`prompt_surfaces_reference_only_real_tools`) has a per-token allowlist for non-tool identifiers (param names, etc.). Companion-side lint must mirror that allowlist or expose it as configuration to prevent false positives on legitimate non-tool tokens.



### H-4 — Drop companion compression-reminder once server-instructions survive compaction

**Pattern:** the companion `SessionStart` hook duplicates Iron Laws content already canonical in `src/prompts/source.md::server_instructions`. Multiple copies in context — three by U-4's count — produce drift (U-5, U-6), token bloat, and an inversion of "canonical is the source of truth" (the *weakest* derived copy is the most compaction-resilient).

**Confirming data:**
- **U-4** (triplication: canonical + companion + buddy).
- **U-5** (compression-reminder drops the bounded-LHS carve-out for Law 3 — derived surface loses precision).
- **U-6** (compression-reminder cites stale tool names — derived surface drifts faster than canonical).

The three failures stem from the same root cause: maintaining a derived copy by hand. Drop the copy, drop the drift.

**Proposed hookify rule:**

- **Predicate:** if `server_instructions` is rebroadcast at every CC MCP session start *and* survives `/compact` events (i.e. is re-injected when the user resumes a compacted session), then the companion compression-reminder is duplicate-by-design and should be removed from main-session `SessionStart`.
- **Decision:** deferred until measurement.
- **Reason text:** *"compression-reminder duplicates canonical server-instructions; codescout MCP injects them at every session-start. Companion's main-session copy is redundant if injection happens on resume too."*
- **Counterpoint to test:** subagents may NOT inherit MCP server-instructions automatically (they only see the parent's context). If subagents miss the canonical Iron Laws, the compression-reminder still earns its keep — but **only on `SubagentStart` hooks**, not on main-session `SessionStart`. Keep one, drop the other.

**Promote-when:** one targeted experiment confirms two facts:
1. `server_instructions` is re-injected at session resume after `/compact` (measure by triggering compaction and inspecting the next assistant turn's system prompt).
2. Subagents do NOT inherit MCP server-instructions on `SubagentStart` (measure by reading a subagent's system prompt).

Outcomes:
- Both confirmed → drop from `SessionStart`, keep on `SubagentStart`.
- Only (1) → drop from `SessionStart`, also drop from `SubagentStart`.
- Only (2) → keep both; Pika lives with the triplication.

**Status:** proposed (blocked on the two measurements).

**Notes:** the buddy `gates.md` prose narration (U-11) is the easier first kill — it has no compaction-survival role. Drop that first regardless of how this experiment resolves; H-4 is the harder, evidence-gated promotion.



### H-5 — Wire `audit_doc_refs` into CI for CLAUDE.md and docs/**/*.md

**Pattern:** doc surfaces (CLAUDE.md, trackers, READMEs) cite code paths that have since been renamed, moved, or removed. The project already has a tool — `librarian(action="audit_doc_refs")` — built specifically to detect this; it's just not wired into automated enforcement.

**Confirming data:**
- **U-7** — CLAUDE.md cites `src/prompts/server_instructions.md` and `src/prompts/onboarding_prompt.md`, both renamed into `src/prompts/source.md`. The project's own self-referential surface ("Prompt Surface Consistency") drifted; nothing automatic caught it.
- **Cross-reference:** the project's `## Standard Ship Sequence` in CLAUDE.md (step 5) already documents running `audit_doc_refs` *post-cherry-pick*. CI promotion just makes that automatic per-PR rather than per-release.

**Proposed hookify rule:**

- **Predicate:** CI step `cargo run -- librarian audit_doc_refs --paths CLAUDE.md docs/**/*.md README.md --fail-on med` on every pre-merge build.
- **Decision:** `warn` initially (start lenient — existing drift may produce noise); escalate to `deny` once a one-time cleanup of existing drift is complete.
- **Reason text:** *"doc references a path / symbol / link target that no longer exists in the codebase. Either fix the doc or update the reference. See `librarian audit_doc_refs` output for the finding details and severity."*
- **Implementation note:** the audit tool already supports `--fail-on` thresholds and emits a tracker artifact when `emit_tracker=true`. For CI, run with `--fail-on med` and skip the tracker emit (tracker mode is for manual investigation sessions, not CI).

**Promote-when:** one more doc-vs-code drift incident lands on `master` (current count: 1 confirmed via U-7; possibly 2+ if prior unfixed instances exist in the audit history). Concrete threshold for promotion from `proposed` to `active`:
- **warn ship:** 3 documented `audit_doc_refs` findings of severity≥med in repo doc surfaces across two months.
- **warn → deny promotion:** zero warn-stage CI false positives across one month.

**Status:** proposed.

**Notes:**
- The audit tool already classifies findings as `verdict ∈ {missing, ambiguous_basename, resolved_basename}`. CI should only fail on `verdict=missing severity≥med`; `ambiguous_basename` is informational (could be a basename collision; not necessarily wrong); `resolved_basename` is OK.
- Once active, this hook closes the loop on U-7 by making the failure mode loud at PR time instead of session time. The companion to H-3 (which catches tool-name drift in companion surfaces): H-5 catches path/link/symbol drift in doc surfaces.
