---
kind: tracker
status: active
title: Session Log — Bug-Fix Work Stream
owners: []
tags:
  - session-log
  - bug-fix
topic: Multi-session bug-fix work stream — frictions and wins from closing open buffer/markdown bugs in docs/issues/
time_scope: open-ended
---

# Session Log — Bug-Fix Work Stream

> **Scope:** Multi-session work to close the OPEN bugs in `docs/issues/`.
> Started 2026-05-17 when the controller began scouting the 4 open buffer/markdown
> bugs filed 2026-05-09. F-N entries capture drift between bug-file Resume hints
> and current code reality; W-N entries capture practices that prevented wasted
> fix work.


> **Purpose:** Two-sided observation log for a multi-session work stream.
> Captures frictions (F-N) and wins (W-N) that the session producing it
> wants to preserve so future sessions inherit the lesson.
>
> **How to use:** Copy this file to `docs/trackers/<topic>-session-log.md`
> in the active project on first reconnaissance pass. Append F-N / W-N
> entries via `edit_markdown(action="insert_before", heading="## Template
> for new entries", content=...)`. Add a row to the Index / Wins Index
> table for each new entry — the indexes are the eval surface, the
> sections are the evidence.
>
> **Lifecycle:**
> - Created at the start of a multi-session work stream.
> - Appended-to across every session that touches the work.
> - Entries with `Status: open` carry forward across sessions.
> - Promotion to permanent surfaces (CLAUDE.md, ADRs, formal bug
>   trackers) happens when the entry's `Promote-when` / `Fix idea`
>   criteria fire.
> - File archived (moved to `docs/trackers/archive/`) when the work
>   stream wraps.

---

## Index

| ID | Date | Severity | Category | Status | Title |
|----|------|---------:|----------|--------|-------|
| F-1 | 2026-05-17 | low | plan-prose | fixed-verified | Bug-file Resume paths cite non-existent layout |
| F-2 | 2026-05-17 | med | self-friction | fixed-verified | 2 of 3 buffer bugs likely stale — code reads correct |
| F-3 | 2026-05-18 | med | plan-prose | fixed-verified | Plan test assertions cited non-existent `RecoverableError.hint` field |
| F-4 | 2026-05-18 | med | codescout-tool | fixed-via-bug-tracker | `edit_markdown action="replace"` with a heading clobbers the whole section body |
| F-5 | 2026-05-18 | high | release-pipeline | open | HEAD detached from `experiments` without `git checkout` in this session |
| F-6 | 2026-05-20 | med | release-pipeline | fixed-verified | HEAD non-compiling + 11 dormant clippy-1.95 lints exposed by toolchain bump |
| F-7 | 2026-05-21 | high | codescout-tool | open | `references` undercounts vs `call_graph` (~18%); root is live-RA incompleteness, not position |
| F-8 | 2026-05-23 | med | codescout-tool | fixed-verified | `format_read_file` dispatches on `type`; json_path output collided → rendered `"0 lines"` |
| F-9 | 2026-05-24 | med | codescout-tool | mitigated | `audit_doc_refs` `fail_on` arg silently ignores `med`/`low` despite docs |
| F-10 | 2026-05-24 | low | release-pipeline | mitigated | RELEASE-TODO advertises "CI pipeline" as unchecked; workflow exists, push trigger pointed nowhere |
| F-11 | 2026-05-24 | med | release-pipeline | fixed-verified | CI runner missing `mold` linker required by `.cargo/config.toml` |
| F-12 | 2026-05-24 | med | codescout-tool-usage | fixed-verified | Dismissed `references`'s "use call_graph for authoritative callers" warning → shipped half-fix, missed `build.rs` duplicate |

## Wins Index


| ID | Date | Impact | Pattern | Counterfactual | Status |
|----|------|-------:|---------|----------------|--------|
| W-1 | 2026-05-17 | med | Scout helper-fn bodies before fixing reported bugs | Would have written instrumentation / "fix" for `extract_lines` and `extract_json_path` despite both being correct + having passing tests | promoted-to-permanent-docs |
| W-2 | 2026-05-18 | med | Pre-dispatch recon scouts type accessors named in plan assertions | Task 2's first subagent would have failed `cargo check` on `err.hint.as_deref()` (no such field); 1+ wasted round-trip per test, controller drift mid-dispatch | validated |
| W-3 | 2026-05-18 | high | `git merge --ff-only <sha>` for detached-HEAD recovery under concurrent work | Naive `git branch -f` would silently traverse a parallel session's commit (the F-13 failure mode); `--ff-only` errors loudly on stale tip instead | promotion-eligible |
| W-4 | 2026-05-18 | high | Pre-fix recon validates filed-bug claims against pinned regression tests | Would have implemented a "fix" that broke the BUG-037 regression test `editing_start_line_does_not_walk_back_to_outer_attribute_on_impl_block`; bug filing itself was inaccurate (claimed attrs not included; actually they ARE via BUG-031) | validated |
| W-5 | 2026-05-24 | med | Deserialize before asserting: test against semantic data, not serialized text | Round-2 Windows fix asserted on JSON-encoded `text` containing escaped backslashes; passed Linux but broke Windows. Saves ≥1 CI cycle (10-15 min) per cross-platform test fix | validated |
| W-6 | 2026-05-24 | high | Cross-platform representation choices apply at every read AND write seam | Round 5 normalized writes only; round 6 had to fix 6 separate read-side boundaries (LIKE patterns, scope filters, substring checks). Missed read-side normalization is silent until integration. One miss (delete_orphan_repos LIKE) was destructive — wiped every catalog row. | validated |
| W-7 | 2026-05-25 | med | Verify-open recon flips zombie-fixed entries from `open` to `fixed-verified` | Without scout, F-6 + F-11 would have continued to be counted as actionable backlog; future "what's open?" queries would have wasted ~30 min each re-investigating already-shipped fixes, or shipped them as known-issues in release notes. 2 zombies caught in one pass. | validated |
## Category conventions

Use a short kebab-case category to group similar frictions. Prior
sessions have used:

| Category | When to use |
|---|---|
| `codescout-tool` | Friction in a codescout MCP tool (`grep`, `read_file`, `edit_markdown`, etc.) |
| `subagent` | Subagent produced unexpected output or diverged from instructions |
| `plan-prose` | Plan document had drift vs reality (wrong file paths, fictional code, mismatched counts) |
| `architectural` | Discovered structural property of the system that the plan / docs didn't surface |
| `self-friction` | Predicted a friction that turned out to be a false alarm — recorded for transparency |
| `<language>-<library>` | Language- / library-specific footgun (`rust-serde`, `python-typing`) |
| `release-pipeline` | Deployment-time gap (release binary missing, MCP reload needed, etc.) |

Add a new category by writing it as a kebab-case string; no central registry needed.

---

## F-N entry template

Copy this block when appending a new friction. Allocate the next free
ID. Add a matching row to the Index table.

```markdown
## F-N — <one-line title>

**Observed:** <date, session task>

**When:** <what you were trying to do>

**Expected:** <what plan / docs / prior session said>

**Got:** <actual observed reality>

**Probable cause:** <one sentence>

**Workaround:** <what you did to proceed>

**Severity:** low | med | high

**Status:** open | wontfix-false-alarm | fixed-verified | mitigated | promoted-to-bug-tracker | pinned-as-eval-baseline

**Fix idea / Pointer:** <issue # in formal tracker, plan task ID, or "TBD">

---
```

## W-N entry template

Copy this block when appending a new win. A win without a
**Counterfactual** is marketing — name what would have happened
without the pattern, with at least one piece of evidence.

```markdown
## W-N — <one-line title>

**Observed:** <date, session task>

**Pattern:** <the practice that worked>

**Counterfactual:** <what would have happened without the pattern, with evidence>

**Confirming data points:** <list of session moments validating the pattern; aim for ≥2>

**Impact:** low | med | high

**Promote-when:** <criterion for graduating into permanent docs (CLAUDE.md, ADR, etc.)>

**Status:** validated | promoted-to-permanent-docs | archived

---
```

---

## Status vocabulary

Codified so the Index column means the same thing across sessions.

### Friction statuses

| Status | Meaning |
|---|---|
| `open` | Observed, not yet resolved. Default for new entries. |
| `wontfix-false-alarm` | Initial observation was wrong; documented for transparency rather than deleted. |
| `mitigated` | Workaround in place; root cause not fully resolved. |
| `fixed-verified` | Code / process fix landed AND empirically confirmed. (`fixed` alone is too weak — verification is part of the status.) |
| `promoted-to-bug-tracker` | Moved to a formal tracker (`docs/issues/*`, `docs/TODO-*`, GitHub issue). The session log keeps the pointer; the formal tracker owns the lifecycle. |
| `pinned-as-eval-baseline` | Kept verbatim as a reference point for measuring later improvements. Do NOT close — its job is to remain comparable. |

### Win statuses

| Status | Meaning |
|---|---|
| `validated` | Pattern confirmed by ≥1 counterfactual data point. Default for entries with evidence. |
| `promoted-to-permanent-docs` | Moved into CLAUDE.md, an ADR, a skill, or another permanent surface. Session log keeps the pointer. |
| `archived` | Pattern no longer load-bearing — either the underlying system changed or the discipline became automatic. |

---

## F-1 — Bug-file Resume paths cite non-existent layout

**Observed:** 2026-05-17, scouting `docs/issues/2026-05-09-*.md` to start fix work.

**When:** First `symbols(path)` calls after reading the 4 OPEN bug files.

**Expected:** Bug-file Resume sections cited `src/tools/buffer/handlers.rs` and `src/tools/search/grep.rs` as the starting points.

**Got:** `path not found: src/tools/buffer/` and `path not found: src/tools/search/grep.rs`. Real layout: `src/tools/output_buffer.rs`, `src/tools/read_file.rs` (with `read_from_buffer` helper inside), `src/tools/grep.rs` (top-level — no `search/` subdir).

**Probable cause:** Module restructure between 2026-05-09 (when bugs filed) and 2026-05-17 (current session). Bug files are append-only and never refresh path hints when the layout shifts.

**Workaround:** Ran `tree(src/tools)` to discover the real layout, then proceeded from `read_from_buffer`, `Grep::call`, `extract_lines`, `extract_json_path`.

**Severity:** low

**Status:** fixed-verified

**Fix idea / Pointer:** Resume sections for the 3 closed bug files (`grep-buffer-false-negatives`, `read-file-buffer-midpoint-empty`, `read-file-json-path-array-elements`) were rewritten in the same session to cite real symbols (`Grep::call`, `extract_lines` at `src/util/text.rs:23-33`, `extract_json_path` at `src/tools/file_summary/file_summary.rs:419`). Bug #1 (`edit-markdown-insert-after-h1`) still cites `src/tools/markdown/edit_markdown.rs` which is a real path — not affected.

---

## F-2 — 2 of 3 buffer bugs likely stale; code reads correct

**Observed:** 2026-05-17, reading `extract_lines` + `extract_json_path` bodies before writing fixes.

**When:** Investigating root causes for `2026-05-09-read-file-buffer-midpoint-empty.md` and `2026-05-09-read-file-json-path-array-elements.md`.

**Expected:** Per bug-file hypotheses, expected to find chunk-walker not advancing (`#3`) and array-index json_path mishandled (`#4`).

**Got:**
- `extract_lines` (12 lines, `src/util/text.rs:21-33`) — trivially correct line-range filter with existing passing test `extract_lines_out_of_bounds_returns_empty`. No chunk-walker exists.
- `extract_json_path` (`src/tools/file_summary/file_summary.rs:419`) — handles array indexing + has passing test `extract_json_path_array_index` using `$.users[0]`. Comment in code explicitly cites `$.symbols[0].body` as a working case.

**Probable cause:** Either (a) intervening commit closed the bugs without crediting back the bug files, or (b) original reporter observed empty STRING value at the json path (correct return) and misread it as "broken." `Bug #2` (`grep-buffer-false-negatives`) IS real — `Grep::call` has no `@*` ref handling at all.

**Workaround:** Plan to write probe tests mirroring the exact bug-file reproductions. If they pass: close as `wontfix-false-alarm` + commit the test as a regression guard. If they fail: real defect surfaces.

**Severity:** med

**Status:** fixed-verified

**Fix idea / Pointer:** Probe tests confirmed prediction:
- Bug #2 `grep-buffer-false-negatives` FAILED probe → confirmed real → fixed via `grep_in_buffer` + `@`-prefix branch in `Grep::call`.
- Bug #3 `read-file-buffer-midpoint-empty` PASSED probe → closed `wontfix`, test kept as regression pin.
- Bug #4 `read-file-json-path-array-elements` PASSED probe → closed `wontfix`, test kept as regression pin.

---

## W-1 — Scout helper-fn bodies before "fixing" reported bugs

**Observed:** 2026-05-17, bug-fix work stream kickoff.

**Pattern:** Read the `extract_lines` and `extract_json_path` bodies before writing any failing test or any fix. Each was small (~12 / ~50 lines) and surfaced its own correctness immediately.

**Counterfactual:** Without reading the helper-fn bodies first, would have written either (a) instrumentation patches to "debug" non-existent chunk-walker bugs in `extract_lines`, or (b) a "fix" adding array indexing to `extract_json_path` that already exists and is tested. Both = wasted commits + churn on green code. Concrete evidence: bug-file hypothesis "Buffer is chunked internally and the chunk-walker may not advance past the first chunk" was confidently wrong — there is no chunk walker.

**Confirming data points:**
1. `extract_lines` body reveals no chunk walker exists — bug-file hypothesis #3 invalidated in one read.
2. `extract_json_path_array_index` test (passing, `$.users[0]`) invalidates bug-file hypothesis #4 in one read.
3. By contrast, scouting `Grep::call` confirmed bug #2 is real — same scouting move found one true defect among three claimed defects.
4. Probe tests for #3 and #4 passed immediately, proving the prediction; probe test for #2 failed with an explicit panic naming the failure mode (`relative path '@tool_*' requires an active project`), letting the fix shape itself.

**Impact:** med

**Promote-when:** This is a specific instance of the existing reconnaissance discipline. Already covered by `codescout-companion:reconnaissance` skill. No promotion needed — count this as a confirming data point for the skill, not a new rule.

**Status:** validated

---
## F-3 — Plan test assertions cited non-existent `RecoverableError.hint` field

**When:** Pre-dispatch reconnaissance for the jsonpath negative-slice
implementation plan (`docs/superpowers/plans/2026-05-18-jsonpath-negative-slice.md`).
About to dispatch Task 1 subagent.

**Expected (plan):** `RecoverableError` has accessible `.hint: Option<String>`
field; plan tests used `err.hint.as_deref().unwrap_or("")`.

**Got (scouted reality):** `RecoverableError` at `src/tools/core/types.rs:169`
exposes `pub message: String` and `pub guidance: Option<Guidance>` — there is
NO `.hint` field. There IS a method `.hint() -> Option<&str>` that returns the
text only for the `Guidance::Hint` variant. The `Display` impl's own comment
explicitly recommends `to_string().contains(...)` for test assertions because
it renders `"{message} — Hint: {text}"` and is the documented stable contract:

> "Display renders only `message`. The structured `hint` and `recovery_steps`
> are intentionally omitted here so existing `to_string().contains(...)` test
> assertions stay stable."

Wait — re-reading: Display renders `message` PLUS `" — {field_name}: {text}"`
when guidance is present. The contract is: `to_string().contains(hint_text)`
holds. So tests can use `err.to_string().contains("...")` regardless of which
guidance variant is attached.

**Probable cause:** Plan was written from the design spec; spec didn't pin
the assertion-side accessor shape; writing-plans phase didn't scout
`RecoverableError`. Standard "scout helper-fn bodies" rule (W-1 in this same
session log) applies to type shapes too.

**Workaround:** Edit the plan's Task 2 + Task 3 test code to use
`err.to_string().contains("...")` everywhere a hint-text or message-text
assertion is made. Drops the `.hint` field reference. Less brittle than
`err.message.contains(...)` because it also covers cases where the failing
substring lives in the guidance text rather than the message text.

**Severity:** med — would have caused first subagent's tests to fail to
compile; controller would then absorb the failed-task drift mid-dispatch.

**Status:** open → fixed-verified after plan edit lands this turn.
## W-2 — Pre-dispatch recon caught test-shape error before any subagent ran

**When:** About to dispatch Task 1 of the jsonpath negative-slice plan to a
fresh subagent (subagent-driven-development mode).

**Pattern:** Before the first subagent dispatch on a plan that names *types*
in test assertions (not just *fns*), invoke the reconnaissance skill and
scout each referenced type's actual field/method shape. Specifically:
`symbols(name=<TypeName>, include_body=true)` for any type whose accessors
the plan tests mention.

**Counterfactual:** Without this scout, Task 2's first subagent would have
written `err.hint.as_deref().unwrap_or("")` and failed `cargo check` on the
first parse test. The subagent would have flailed (probable retries with
`err.guidance`, or `err.hint().unwrap_or("")`, or `err.to_string()`) without
the Display-impl contract context. Best case: 1 extra round-trip per failing
test (~11 round-trips for the 11 parser tests in Task 2). Worst case: the
subagent gives up, controller re-scopes plan mid-dispatch, F-N entry written
*after* drift instead of *before*.

**Confirming data points:**
1. F-3 (this session) — `RecoverableError.hint` field cited by plan does not
   exist; scout caught it pre-dispatch.
2. Pending: any future plan that names types in assertions.

**Impact:** med — saves ≥1 failed subagent task and prevents controller
context absorption of cascading test failures. The saving compounds
across the 4 implementation tasks in the plan.

**Promote-when:** A second pre-dispatch recon catches a similarly hidden
type-shape mismatch (any plan, any type). At 2 datapoints, promote to
CLAUDE.md's "Before dispatching the first subagent of an implementation
plan, scout every type whose accessors the plan asserts on" rule.

**Status:** validated — single datapoint, drift caught + fixed in the same
turn before any subagent dispatch. Awaiting promotion criterion.
## F-4 — `edit_markdown action="replace"` with a heading argument clobbers the whole section body

**Observed:** 2026-05-18, while adding the "When the Substrate Catches Itself" section to `docs/observations.md`.

**When:** Tried to add a new H2 section after the existing `## The Plugin Closes the Loop` via `edit_markdown(action="replace", heading="## The Plugin Closes the Loop", content="<new section + trailing closer line>")` — expecting insert-after-like semantics.

**Expected:** `action="replace"` with `heading=X` would either replace only the heading line or operate on a localized region near the anchor.

**Got:** The full body of `## The Plugin Closes the Loop` (~30 lines of SessionStart / SubagentStart / PreToolUse hook narrative + the marketplace install hint) was overwritten wholesale with the new section's content. The original body was destroyed.

**Probable cause:** `edit_markdown action="replace"` with a `heading` argument has "set the body of this section to `content`" semantics. The argument is the section anchor, not an insertion anchor; the operation wipes from the heading's end through to the next sibling heading. Not aliased with `insert_after`.

**Workaround:** Caught by the Frog discipline's post-edit verify (`read_markdown` after every write). Reconstructed the original body from an earlier in-session `read_markdown` snapshot, ran `edit_markdown action="replace"` again with the original content to restore it, then `edit_markdown action="edit"` for cosmetic blank-line repairs. Three extra round-trips beyond the intended insert.

**Severity:** med — data loss within session, fully recovered, but easy to miss without verify-after-edit.

**Status:** fixed-via-bug-tracker (Option A shipped this session); see `docs/issues/2026-05-18-edit-markdown-replace-clobber.md` (status: fixed, closed 2026-05-18).

**Fix idea / Pointer:** Option A shipped — destructive-scope warning added to `long_docs()` and per-variant action descriptions in the schema for `EditMarkdown` in `src/tools/markdown/edit_markdown.rs`. Top-level `description()` stays under the 300-char budget (caught by `server::tests::tool_descriptions_stay_under_budget` on first attempt). Option B (force flag + size threshold) deferred until Option A is observed to be insufficient — bug-tracker entry retains the Option B sketch and three regression tests it would need.

---

## F-5 — HEAD detached from `experiments` without `git checkout` originating in this session

**Observed:** 2026-05-18, after several `/reload-plugins` calls + one `/mcp` reconnect cycle while working on observations.md.

**When:** Session started on `experiments` per the git status snapshot at the top of the system prompt. After multi-step work (tracker rectification, hook verification, observations.md edits), ran `git commit` for the observations work — output showed `[detached HEAD a70816b5]`, NOT `[experiments a70816b5]`.

**Expected:** `git checkout`-style branch detachment requires an explicit checkout call. Host operations (`/reload-plugins`, `/mcp` reconnect) and codescout MCP tool calls should leave HEAD on its current branch.

**Got:** `git reflog -15` revealed three unattributed HEAD moves between my last commit (`81a6d136`, on `experiments`) and the detached commit (`a70816b5`):
```
HEAD@{1}: checkout: moving from experiments to d5bf7116
HEAD@{2}: checkout: moving from 59f6b53c… to experiments
HEAD@{3}: checkout: moving from experiments to 59f6b53c
```
HEAD was actively bounced between `experiments` and parallel-session commit SHAs (the `feat(file_summary)` work the other session shipped). My session issued zero `git checkout` calls during that window.

**Probable cause:** Unknown. Candidates:
1. `codescout-companion` PostToolUse hook on `mcp__.*__workspace` (`cs-activate-project.sh` or `worktree-activate.sh`) running `git checkout` as a side effect.
2. Parallel session's commits propagating into shared workspace state in a way the host translates to a HEAD move on my side.
3. `/reload-plugins` re-running SessionStart-style hooks that touch git refs.

**Workaround:** Detected via the `[detached HEAD ...]` string in `git commit` output. Recovered via W-3 (`git merge --ff-only`). No data lost.

**Severity:** high — silent data-loss vector. Commits on detached HEAD are reachable only via reflog and expire on `git gc`. The user only sees the failure if they read the `git commit` output carefully — easy to miss.

**Status:** open — root cause unknown.

**Fix idea / Pointer:** Open a bug file with the reflog snippet preserved. Audit companion hooks that touch git state — likely candidates: `hooks/cs-activate-project.sh`, `hooks/worktree-activate.sh`, anything in the SessionStart hook chain. If a hook is moving HEAD as a side effect of workspace activation, scope it to only run when the active project itself changed, not on every reconnect.

---

## W-3 — `git merge --ff-only` as the atomic recovery primitive under concurrent-work HEAD detachment

**When:** 2026-05-18, recovering commit `a70816b5` (observations.md narrative) from detached HEAD after `/reload-plugins` + `/mcp` cycle silently moved HEAD to a parallel session's commit SHA (`d5bf7116`).

**Pattern:**
1. Read full state in a single command — `git reflog -15 && git branch -v && git rev-parse HEAD && git symbolic-ref HEAD` — so the observation set is internally consistent before any write.
2. Recover with `git checkout <target-branch> && git merge --ff-only <recovered-sha>` in a single command. The `--ff-only` flag is the atomic safety: it succeeds silently if `<recovered-sha>` is a strict descendant of the target branch's current tip (history is not fabricated, only the branch pointer moves), and it fails loudly if anything diverged between the read and the write.

**Counterfactual:** Naive recovery `git branch -f experiments a70816b5` would force-move the branch ref regardless of whether `experiments` had moved between observation and action. If a parallel session shipped a commit to `experiments` in the gap between my `git reflog -15` read and the `git branch -f` write, the force-move would silently traverse that commit — the F-13 failure mode incarnate. `--ff-only` removes the traversal hazard by making git refuse to invent history; if `experiments` has moved, the merge errors out and the operator reconciles manually rather than discovering the loss later via reflog.

**Confirming data points:**
1. F-13 (2026-05-17, prior session) — `git reset --soft HEAD~1` erased a parallel session's T-13 commit because HEAD moved between observation and `reset` action. Recovered via reflog-quoted SHA. Driving incident for the existing CLAUDE.md § Concurrent-Work Rules block.
2. This session (2026-05-18) — F-5 detached-HEAD recovery used `git merge --ff-only a70816b5` after a single combined `reflog -15` read; experiments tip was `d5bf7116`, recovered-sha's parent was also `d5bf7116`, ff-only succeeded silently. Working tree clean, no data lost.

**Impact:** high. Concurrent-work git is the single most common silent-data-loss vector in multi-session work; `--ff-only` is one of the few git primitives that fails-loudly on stale-tip.

**Promote-when:** Two concretes reached (F-13 + F-5). Promote to CLAUDE.md § Concurrent-Work Rules with an explicit primitive callout: *"For detached-HEAD recovery, use `git checkout <branch> && git merge --ff-only <recovered-sha>` in one command. Never `git branch -f <branch> <recovered-sha>` — it traverses parallel commits silently."*

**Status:** promotion-eligible (criterion fires this session).

---

## W-4 — Pre-fix recon caught wontfix bug (BUG-037 was already shipped)

**Observed:** 2026-05-18, about to implement
`docs/issues/2026-05-18-edit-code-replace-misses-outer-attrs.md`.

**Pattern:** Before writing code to "fix" a behavior reported as buggy
by a prior subagent's escape-hatch fallback (e.g. `python3 re.sub`),
scout the symbol that owns the behavior — including its sibling tests.
If a regression test pins the current behavior with a `BUG-XXX`
reference in its name or doc comment, the current behavior is
deliberate, not a bug. Update the filed bug to `wontfix` and pivot to
the real surface (usually documentation).

**Counterfactual:** Without this scout, I would have written code to
extend `editing_start_line`'s walk-back to include attribute-only
blocks above an `impl`/`fn`. That code would have broken the
`editing_start_line_does_not_walk_back_to_outer_attribute_on_impl_block`
regression test pinned by BUG-037, which catches the more dangerous
failure mode (silently dropping `#[async_trait]`-style attributes).
The TDD red-bar would have fired at cargo test, but only AFTER I'd
written the wrong-direction fix — at least 1 wasted round-trip plus
reviewer cycles.

The scout also caught that the bug description I just filed (≤2
minutes prior) was inaccurate: it claimed "outer attributes are NOT
included in the replace range," but the actual default behavior IS
inclusive via BUG-031 walk-back. The narrower BUG-037 guard is what
the user's pain point ran into.

**Confirming data points:**
1. F-3 (this same session log) — `RecoverableError.hint` field cited
   by a plan was non-existent; scout caught it pre-dispatch.
2. W-2 (this same session log) — the W-N for F-3.
3. **W-4 (this entry):** scout caught a wontfix-bug-filing mistake AND
   prevented a regression-test-breaking fix attempt. Two failure
   modes prevented in one scout.

**Impact:** high — scout prevented (a) bad bug filing being treated as
ground truth, and (b) wrong-direction fix attempt that would have
broken pinned regression coverage. The cost of NOT scouting compounds:
filed bug → planned fix → coded fix → red bar → debugging → revert →
correct fix. Each step is 5-20 minutes; total cost could be 1-2 hours
of churn.

**Promote-when:** Already at 3 datapoints (F-3, W-2, W-3). At 3
distinct datapoints, the recon habit is no longer probationary.
Promote to CLAUDE.md as a permanent rule:

> Before writing code to fix a behavior reported as wrong, scout the
> symbol that owns it AND its sibling tests. If a regression test
> pins the current behavior with a `BUG-XXX` doc reference, treat
> that as a strong "the current behavior is intentional" signal.
> Validate the bug filing's claim against the test pin before
> committing to a fix direction.

**Status:** validated — promotable; awaiting CLAUDE.md edit.

## F-6 — HEAD non-compiling on `experiments`; clippy 1.95 toolchain bump exposed 11 dormant lints

**Observed:** 2026-05-20, pre-commit recon for a 3-way commit split of uncommitted changes (IL3 narrow, probe sentinels, gfx1101 arch bump). User said `cargo clippy` must pass per CLAUDE.md; clippy failed with 11 errors, raising the question of whether the uncommitted diff introduced them.

**When:** Phase-1 scout: `git stash push -u` + `cargo clippy --all-targets -- -D warnings` against bare HEAD.

**Expected (working assumption):** HEAD `experiments` compiles cleanly. Uncommitted diff might or might not introduce lints; stash-and-reclippy isolates the blame.

**Got (scouted reality):** Two independent problems entangled.

1. **HEAD does not compile.** With uncommitted diff stashed, `cargo clippy` failed with `E0063: missing field guide_hints_emitted in initializer of ToolContext` at `src/tools/run_command/tests.rs:372`. The `guide_hints_emitted` field was added to `ToolContext` in commit `68947c4a feat(prompts): first-call hint mechanism for get_guide topics`, but the run_command test fixtures were not updated in the same commit. The uncommitted `tests.rs` changes (`+guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(Default::default()))` × 12 sites) are silently *fixing* this broken state, not adding new functionality. The split-commit plan must keep these test fixture edits paired with the IL3 commit (or be its own commit) — they are load-bearing.

2. **Clippy 1.95 added new lints** (URLs cite `rust-clippy/rust-1.95.0/`). With uncommitted diff applied, compilation passes, and 11 pre-existing lints fire across 6 unrelated files:
   - `src/librarian/preview/memory.rs:16`, `src/logging.rs:66`, `src/tools/file_group.rs:38` — `unnecessary_sort_by` (3)
   - `src/retrieval/sync.rs:174`, `src/tools/symbol/call_graph/traversal.rs:86` — `useless_conversion` on `.into_iter()` (2)
   - `src/tools/markdown/edit_markdown.rs:51, 174, 269` — `unnecessary_cast usize as usize` (3)
   - `src/util/path_security.rs:444, 452, 459` — `collapsible_match` in tool-name validate arms (3) — NOT in the IL3 functions edited by the uncommitted diff, in the unrelated `validate_tool_for_path` match block.

**Probable cause:**
- Problem 1: `feat(prompts)` commit landed without `cargo test` against `run_command` test fixtures, OR was tested locally with stale build artifacts that didn't recompile the affected test file. Pre-commit `cargo test` rule (CLAUDE.md) was elided.
- Problem 2: Rust toolchain auto-updated to 1.95 between last clean clippy run and now. New lints are mechanical.

**Workaround:**
- Problem 1: keep the `tests.rs` field-init edits in the same commit as the IL3 narrow, or as a precursor compile-fix commit. Don't split them off as "test cleanup."
- Problem 2: run `cargo clippy --fix --allow-dirty --allow-staged` to auto-apply the 11 fixes, then commit as `chore(clippy): adopt clippy 1.95 suggestions`. Keep it separate from feature commits to keep the lint-bump signal legible in `git log`.

**Severity:** med — Problem 1 means HEAD on a public-facing branch is broken; any developer cloning `experiments` would fail `cargo test` without the uncommitted local edits. Problem 2 is mechanical cleanup but blocks the project's CLAUDE.md "clippy clean before commit" gate.

**Status:** fixed-verified (2026-05-25). The clippy-clean state shipped to master as part of the cumulative session-work batch in merge `d1742c46`; no single labeled "clippy 1.95 cleanup" commit landed, so the entry stayed `open` despite the underlying condition being long-resolved. Verified this turn: `cargo clippy --all-targets -- -D warnings` on current experiments HEAD exits 0 with no warnings. Lesson — distributed-fix entries need an explicit close pass; the absence of a labeled commit doesn't mean the bug is still open.

**Fix idea / Pointer:** This recon session — split-commit plan adjusted to (1) clippy 1.95 auto-fix cleanup, (2) IL3 narrow + tests.rs field init + bug doc, (3) probe sentinels, (4) docker-compose gfx1101 + backtick-eof bug doc.

---

## F-7 — `references` undercounts vs `call_graph`; root is live-incomplete-LSP vs persistent edge-cache, NOT position

**Observed:** 2026-05-21, debugging the references-undercount bug
(`docs/issues/2026-05-21-references-undercounts-vs-call-graph.md`). Live
`references(symbol="format_read_file", path="src/tools/read_file.rs")` returned 3;
`call_graph(direction="callers")` returned 17; `grep` ground-truth = 17 call sites
in `src/tools/edit_file/tests.rs`.

**When:** Phase-1 root-cause scout of the two tools' resolution paths, before any fix.

**Expected (initial hypothesis):** `references` queries LSP at the *item* start
(column of `pub`) instead of the identifier, so rust-analyzer misfires and
underreports. (Plausible because a wrong-token reference query returns garbage.)

**Got (scouted reality):**
- `references` (`src/tools/symbol/references.rs:54-59`) resolves position via
  `find_unique_symbol_by_name_path` → `sym.start_line/start_col`, then calls
  `client.references()` (`textDocument/references`, `include_declaration: true`,
  no truncation — `src/lsp/client.rs:1032-1058`).
- `SymbolInfo.start_line/start_col` come from **`selection_range.start`** (the
  identifier), NOT `range.start` (`src/lsp/client.rs:159-161`; pinned by test
  `convert_document_symbols_uses_selection_range`). **Position is correct** →
  position-bug hypothesis REJECTED.
- references' build-dir + scope filters dropped 0 here (`excluded=0`), so the live
  `textDocument/references` itself returned only 3 — the undercount is upstream of
  all our code.
- `call_graph` (`src/tools/symbol/call_graph/mod.rs:222-234`) reads a **persistent
  SQLite `EdgeCache.lookup_callers(symbol)` by name**; on cache hit it returns the
  17 edges with **no live LSP call**. The cache was populated earlier by
  `resolve_one_hop` when the set was complete.

**Probable cause:** `references` is at the mercy of rust-analyzer's *live* index
state for the queried symbol's references; for the large `cfg(test)` file
`edit_file/tests.rs` (~4400 lines) RA returned an incomplete set (3 of 17) at query
time. `call_graph` masks this by serving a complete persisted cache. NOT YET
CONFIRMED — pending the warming + non-test-symbol experiments below.

**Workaround:** use `call_graph(direction="callers")` for "who calls X"; fall back
to `grep` for non-call references. Treat a low `references` count as suspect.

**Severity:** high — a navigation tool silently returning ~18% of real callers will
mislead any refactor that trusts it.

**Status:** open — root mechanism (live-RA incompleteness) needs the confirming
experiment. Bug tracker: `docs/issues/2026-05-21-references-undercounts-vs-call-graph.md`.

**Fix idea / Pointer:** if confirmed RA-incompleteness, references needs either a
warmth/completeness guard (re-query until stable, or `did_open`+settle) or should
consult the same EdgeCache call_graph uses. Investigate `resolve_one_hop`
(`src/tools/symbol/call_edges/resolver.rs`) — does it use callHierarchy or
references? If callHierarchy is more complete than textDocument/references on RA,
references could switch to it.
## F-8 — `format_read_file` dispatches on `type` field, collided with json_path output

**Observed:** 2026-05-23, debugging "0 lines" output from `read_file(json_path=...)` on both `@tool_*` buffers and real JSON files.

**When:** Two consecutive failures — first reproducing the user-reported friction with `artifact(get, full=true)` → `read_file(@tool_*, json_path="$.body")` returning `"0 lines\n\n  Buffer: @file_*\n  hint..."`. Then auditing for other sites surfaced the same shape in `read_json_path_nav` for plain JSON files.

**Expected:** A valid `json_path` extraction renders with line count + content (small) or line count + buffer ref (large).

**Got:** `"0 lines"` regardless of extracted content size — even when the underlying `@file_*` buffer contained 128 lines of body.

**Root cause:** `format_read_file` (src/tools/read_file.rs:678) dispatches on `val["type"].as_str()` FIRST. Both `read_from_buffer` (line 175) and `read_json_path_nav` (line 354) emitted `"type": type_name` where `type_name` came from `extract_json_path` (values like `"string"`, `"object"`, `"array"`, `"number"`). These all fell through to `format_read_file_summary`'s `_ => {}` fallback case; `line_count` was never written by these paths, defaulted to `0`. Result: `"0 lines\n"` rendered with no content branch ever consulted.

The bug was invisible until a tool returned `type: <a value not in {source, markdown, json, toml, yaml, config, generic}>`. All in-tree summarizers happen to emit one of the seven known types, so test coverage didn't surface the gap.

**Workaround applied:** Renamed `"type"` to `"value_type"` in both emission sites. `format_read_file` no longer dispatches to summary mode for these results; Content mode (small) and "Old no-content buffered mode" (oversized) handle them correctly. Oversized branch also gains `total_lines: line_count` so the buffered-mode formatter renders an accurate count.

**Severity:** med — degrades agent UX on every `read_file(json_path=...)` call extracting a scalar/markdown body. No data loss; the buffered content was still reachable via the `@file_*` ref the formatter printed alongside the misleading "0 lines". User reported it via this session's transcript.

**Status:** fixed-verified — commit `16c5cfd2 fix(read_file): rename json_path output 'type' to 'value_type'`. Verified post-`/mcp` reconnect with `artifact(get, full=true)` → `read_file(@tool_*, json_path="$.body")` → `128 lines\n\n  Buffer: ...`. Also verified inline path with `read_file("scripts/package.json", json_path="$.private")` → `1 line\n\ntrue`.

**Fix idea / Pointer:** Defensive improvement candidate (not done in this commit): make `format_read_file_summary` log/warn on unknown `type` variants rather than fallthrough rendering `"{line_count} lines\n"` with no body branch. Would have caught the collision at first sighting.

## F-9 — `audit_doc_refs` `fail_on` arg silently ignores `med` and `low` despite docs advertising them

**Observed:** 2026-05-24, pre-implementation reconnaissance for the
`codescout audit-doc-refs` CLI subcommand (H-5 + R-1 shipping).

**When:** About to write the CLI arg parser with `--fail-on` accepting
`high | med | low | never`, per `CLAUDE.md` § Standard Ship Sequence step 5
and the librarian schema docstring.

**Expected (docs):** `librarian(action="audit_doc_refs", fail_on="med")`
returns exit_code=1 when at least one finding has `severity: med`. Same for
`low`. Documented in two places:
- `src/librarian/tools/librarian.rs` schema: *"exit_code 1 when findings reach this severity (high | med | low | never)"*
- `CLAUDE.md` § Standard Ship Sequence step 5 cites `audit_doc_refs --fail-on med`

**Got (scouted reality):** `src/librarian/tools/audit_doc_refs/mod.rs::build_response`
(line 542+) hard-codes only two truthy arms:

```rust
let exit_code: i32 = match fail_on {
    "high" if findings.iter().any(|f| f.resolution.severity == Severity::High
        && !matches!(f.resolution.verdict, Verdict::Resolved | Verdict::External)) => 1,
    "any" if n_broken + n_unknown > 0 => 1,
    _ => 0,
};
```

`fail_on="med"` and `fail_on="low"` fall through the wildcard arm and return
exit_code=0 — silently behaving like `never`. The `Severity` enum has all
three variants (High/Med/Low at line 65) so the data is there; the gating
arm just never references Med/Low.

**Probable cause:** Schema/docs were written aspirationally before
build_response was extended to honor the lower thresholds; the extension
never happened. CLI shipping is the natural forcing function — a real
`--fail-on` flag has to either match the docs or silently lie.

**Workaround (this session):** CLI `--fail-on` accepts only the values the
MCP code actually honors: `high | any | never`. Matches existing behavior;
docs in `librarian.rs` schema + CLAUDE.md need a follow-up reconciliation
(either extend build_response or correct the docs).

**Severity:** med — would have produced a CI gate that silently lets `med`
findings through despite the user believing `--fail-on med` was active.
A noisy bug (silent no-op gates are the worst kind) but not cascading;
controller absorbs the divergence by accepting only verified values.

**Status:** mitigated — CLI accepts only verified values for this session.
Follow-up: decide between extending build_response or rewriting docs;
either ships in a separate change.

**Fix idea / Pointer:** `src/librarian/tools/audit_doc_refs/mod.rs:542`
(build_response). Either add `"med" if findings.iter().any(|f| matches!(f.resolution.severity, Severity::Med | Severity::High) && ...)` arms, or downgrade the docs to `high | any | never` to match reality.

## F-10 — RELEASE-TODO advertises "CI pipeline" as unchecked; workflow already exists, push trigger points at nonexistent branch

**Observed:** 2026-05-24, scouting `.github/` directory before writing the CI
workflow for H-5 + R-1.

**When:** About to `create_file(.github/workflows/ci.yml, ...)` based on
RELEASE-TODO High Priority item *"CI pipeline — GitHub Actions workflow
running `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test` on
every PR. Single biggest protection against bad contributions."*

**Expected (docs):** No CI workflow exists; needs to be created from scratch.

**Got (scouted reality):** `.github/workflows/ci.yml` exists with 5 jobs:
- `fmt` (cargo fmt --check)
- `clippy` (cargo clippy -- -D warnings)
- `test` (3×3 matrix: linux/macos/windows × default/local-embed/no-features)
- `tool-docs-sync` (lints tools manual stays in sync with `src/tools/*.rs`)
- `msrv` (cargo check on 1.82)

Two bugs in the existing workflow:
1. `on.push.branches: [main]` — the repo's protected branch is `master`
   (per CLAUDE.md § Branch Strategy). Push-trigger is dead.
2. No `audit-doc-refs` job — the lint that catches doc/code drift on PRs.

**Probable cause:** RELEASE-TODO was authored before CI was built and never
updated when CI shipped. The `main` vs `master` branch mismatch is a
copy-from-template artifact (most repos default to `main`).

**Workaround (this session):** Skip the scratch creation; surgically edit
the existing `ci.yml` to (a) fix the push branches list, (b) add the
audit-doc-refs job. Update RELEASE-TODO to reflect the partial-shipped
state.

**Severity:** med — would have produced a duplicate / clobbering workflow
file. The existing workflow is invisible to a controller that trusts
RELEASE-TODO, so a from-scratch write was a real risk.

**Status:** mitigated — edits target the existing file this session.

**Fix idea / Pointer:** `.github/workflows/ci.yml`, `docs/RELEASE-TODO.md`
"High Priority" section.

## F-11 — CI runner missing mold linker required by `.cargo/config.toml`

**Observed:** 2026-05-24, CI smoke test for H-5 + R-1 after sccache fix
shipped. Audit Doc Refs / Clippy / MSRV jobs fail with
`collect2: fatal error: cannot find 'ld'` despite sccache now installing
correctly.

**When:** After mozilla-actions/sccache-action@v0.0.7 fix (7f107d8e) lands;
investigating "next layer" of pre-existing CI rot. Initially diagnosed as a
runner-image regression per researcher MCP synthesis on
`collect2: cannot find 'ld'`. Researcher cited slim-image binutils
omissions and `-fuse-ld=lld` configuration as causes. Scouted local config
to verify which applies.

**Expected (assumption):** Either the runner image is missing
`build-essential`, or the project uses default cc/ld (no special linker
config). Standard GitHub Actions ubuntu-latest should have `ld` in PATH.

**Got (scouted reality):** `.cargo/config.toml` lines 5-7:
```toml
[target.x86_64-unknown-linux-gnu]
linker = "cc"
rustflags = ["-C", "link-arg=-fuse-ld=mold"]
```
The project mandates the **mold linker** for x86_64 Linux. `collect2` is
GCC's internal linker driver; it reports the missing program as `'ld'` in
its error message regardless of which linker name was actually requested.
Researcher synthesis pointed at this exact false-positive pattern (their
note: *"the system fails to locate specific linkers like `ld.lld` when the
`-fuse-ld=lld` flag is invoked, rather than a total absence of the GNU
linker"*). Mold has the same shape — `-fuse-ld=mold` requires mold to be on
PATH.

Same config also mandates `rustc-wrapper = "sccache"` and `jobs = 64`. The
file looks like a developer's local performance config (mold + sccache +
64 jobs) but is committed to the repo.

**Probable cause:** `.cargo/config.toml` was authored for local build speed
(mold is much faster than ld for large Rust projects) and committed
because it provides project-wide consistency. CI was never updated to
install mold because CI hasn't successfully run since at least 2026-04-13
(per F-10 + the historical run count).

**Workaround:** Add `rui314/setup-mold@v1` step before `cargo build`
invocations in every affected job (clippy, test matrix, msrv,
audit-doc-refs). Format job unaffected (no compile step). Mirrors the
sccache install pattern.

Alternative: override the project rustflags in CI via env, e.g.
`CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS=""`. Less clean —
diverges CI from local build behavior.

**Severity:** med — would have required a third CI iteration without the
.cargo/config.toml scout (the researcher MCP correctly pointed at the
`-fuse-ld=` pattern; scout confirmed which linker). Without the scout I'd
have tried `apt-get install -y build-essential binutils` (the obvious
guess) and watched it fail because the real issue isn't binutils.

**Status:** fixed-verified (2026-05-25). Commit `29075470 fix(ci): install mold linker required by .cargo/config.toml` (now on master via merge `d1742c46`) added `rui314/setup-mold@v1` to all four affected jobs — clippy, test, msrv (toolchain 1.88), audit-doc-refs. Verified by grep: 4 occurrences in `.github/workflows/ci.yml` at lines 30, 55, 114, 127. The "two pre-existing items behind this one" (macos/windows runner divergence, tool-docs-sync drift) were also addressed in the Windows portability rounds 1-8 + `4e475bad fix(tests): macOS /private/var symlink`. Same distributed-fix bookkeeping pattern as F-6 — entry stayed `open` because the fix landed under a `fix(ci):` label rather than an explicit "closes F-11" reference.

**Fix idea / Pointer:** `.github/workflows/ci.yml` — add
`uses: rui314/setup-mold@v1` step (after `mozilla-actions/sccache-action`,
before `Swatinem/rust-cache@v2`) in jobs clippy, test, msrv,
audit-doc-refs. Also: `.cargo/config.toml` may belong as
`.cargo/config.toml.example` with a CI-friendly version checked in instead.

## F-12 — Dismissed `references` "use call_graph" warning → shipped half-fix to `extract_surface`, missed build.rs duplicate

**Observed:** 2026-05-24, during CI rot rehab after CI run `26356842338` showed
Windows builds panicking on `extract_surface` with CRLF source.md.

**When:** Mid-fix scout. About to edit `src/prompts/source.rs::extract_surface`,
ran `references(symbol="extract_surface", path="src/prompts/source.rs")` to
verify call sites. Tool returned 3 results AND emitted this warning:

> `warning: references found 3, but call-hierarchy found 5 call sites —
>  rust-analyzer's textDocument/references is incomplete for this symbol.
>  Use call_graph(symbol, direction="callers") for the authoritative caller
>  set.`

**Expected (what I should have done):** Read the warning, run `call_graph` per
its instruction. The 2 missing call sites would have surfaced `build.rs:71`
calling its OWN `extract_surface` (duplicate parser declared at `build.rs:86`,
unreachable from `references` because build.rs lives outside the LSP project
index — it compiles as a separate unit).

**Got (what I did):** Skimmed the warning, treated the 3 hits as the full set,
shipped the CRLF fix only in `src/prompts/source.rs`. Pushed commit `c83b5544`,
verified local tests pass, monitored CI run `26357101302` — Windows tests
failed with the **exact same panic** as before. Spent another 5–10 minutes
investigating "why didn't the fix take" before realizing build.rs has a
duplicate parser the build script actually runs. Shipped follow-up fix in
`af64c737`.

**Probable cause:** F-7 (same tracker, 2026-05-21) already pinned the root
mechanism — `references` queries live `textDocument/references` which is
incomplete on some symbols. The tool surfaces this clearly with a per-call
warning naming the workaround. I dismissed it as a generic "tool noise" line
instead of a specific load-bearing pointer.

**Workaround:** When the `references` warning fires, **always** run
`call_graph(direction="callers")` before assuming the result is exhaustive.
Especially for: build scripts (build.rs), proc-macros, dev-dependencies,
doctests, or any code that lives outside the main crate's LSP index. Their
callers are invisible to live rust-analyzer.

**Severity:** med — cost was one wasted CI cycle (~10 min) + ~5 min of
investigation. Bigger lesson: any tool that explicitly tells you it's
incomplete is signalling at exactly the seams where the incompleteness will
bite. F-7 already documents the *mechanism*; F-12 documents the *cost of
dismissing the surfaced warning*.

**Status:** fixed-verified — `af64c737` shipped CRLF-tolerance to both
parsers. CRLF test in `src/prompts/source.rs` pins LF→CRLF byte-equality.
build.rs has no test surface (it runs at compile time on Windows runners; CI
matrix is the verification).

**Fix idea / Pointer:** None for the tools themselves — both `references`
and `call_graph` work as designed. The remediation is **behavioural**: read
the warning, follow its instruction. Consider promoting this to CLAUDE.md
under a "Tool warning discipline" section if a third datapoint lands.
References: F-7 (root mechanism), `docs/issues/2026-05-21-references-undercounts-vs-call-graph.md`.
## W-5 — Test against semantics, not representation: deserialize before asserting on cross-platform output

**Observed:** 2026-05-24, during Windows CI rehab session — round-2 commit (98907430) shipped 14/16 fixes cleanly but two of them broke on the actual Windows runner despite passing locally on Linux.

**Pattern:** When patching a cross-platform test that asserts on a string containing path or path-like data, do not assert against the serialized text. Parse the response and assert against the deserialized data field instead. Test against the **semantic value**, not the **serialization artifact**.

**Counterfactual:** Without this discipline, round-2's two regressions cost a full CI cycle (~10 min) plus a round-3 fix commit. The specific failures:

1. `run_command_output_keeps_absolute_project_paths` (src/server.rs):
   - Asserted `text.contains(&abs)` where `text` is the JSON-serialized MCP response and `abs` is a raw filesystem path.
   - On Unix: forward-slashes don't need JSON escaping → assertion passed locally.
   - On Windows: JSON-serialized backslashes are doubled (`\` → `\\`) but `abs` has single backslashes → `contains` fails.
   - Fix: parse the JSON, extract `parsed["stdout"].as_str()`, assert against the deserialized string. Platform-agnostic.

2. `resolve_refs_substitutes_cmd_ref` (src/tools/output_buffer.rs):
   - Asserted `resolved.chars().nth("grep hello ".len()) == Some(MAIN_SEPARATOR)`.
   - On Unix: position 11 of `grep hello /tmp/...` is `/` (separator AND absolute-path prefix).
   - On Windows: position 11 of `grep hello C:\Users\...` is `C` (drive letter). MAIN_SEPARATOR is `\`, found at position 13 not 11.
   - Fix: accept either a leading separator OR a drive-letter prefix at positions 0-1. Both shapes valid for "absolute path".

The deeper lesson: a single-OS local pass is necessary but **not sufficient** for cross-platform correctness. Asserting against serialized text bakes in platform-specific serialization quirks (JSON escaping, separator characters as path prefixes). Asserting against deserialized data tests the substantive property under test (path roundtrip) rather than the encoding.

**Confirming data points:**
1. R2026-05-24 Windows CI run 26360060746 — 2 of 5 round-2 fixes regressed on Windows despite Linux pass.
2. Both failures shared the same root cause: assertion against representation, not semantics.

**Impact:** med — saves ≥1 CI cycle (10-15 min) per cross-platform test fix that touches output/serialization. Higher impact for tests that ship to multi-OS matrices where each cycle costs N×OS.

**Promote-when:** A third cross-platform CI fix that would have benefited from the "deserialize-then-assert" discipline. At 3 datapoints, promote to `CLAUDE.md` under a "Testing Patterns" section: "When asserting on responses that contain platform-varying data (paths, line endings, drive letters), parse and assert against the deserialized data field. The serialized text bakes in JSON escapes, separator characters, and other platform quirks that mask correctness drift."

**Status:** validated — single datapoint (this session), pattern explicitly captured in round-3 commit (bc05c0b3) message and code comments. Awaiting promotion criterion.

## W-6 — Cross-platform representation choices must be applied at every read AND write seam

**Observed:** 2026-05-24, Windows-default CI rehab continuation. Round 5 (6771cc1a) introduced forward-slash normalization for catalog writes (`artifact::upsert`, `artifact_id_from_abs`). This closed 6 of 18 failures but left 12 — including tests whose write path was now normalized but whose **read** path (LIKE patterns, scope filters, contains-substring checks) still used native separators.

**Pattern:** When you choose a normalized representation for storage (forward-slash strings, lowercased, base64, whatever) — apply that normalization at **every** boundary that produces a string compared against the stored form. Read-side normalization is not optional; it's the symmetric half of the choice. Missing read-side normalization is a silent miss because:
- Unit tests on the writer pass (forward-slash output matches forward-slash assertion).
- Unit tests on the reader pass (native LIKE pattern matches native stored string — until the writer changes).
- The cross-cutting bug only surfaces in integration where writes flow into the reader.

**Counterfactual:** Without W-6's discipline, round 5 looked complete (6 fixes shipped, indexer tests green) but 12 of 18 originally-failing tests still failed. Each was a separate boundary I'd missed:
- `delete_orphan_repos` LIKE pattern (2 tests) — wiped every row because the keep clause matched nothing on Windows.
- `path_prefix_clause` in scope filter (1 test) — filtered out all rows because the prefix had backslashes but stored rows had forward-slashes.
- `audit_doc_refs::parser::md_file` (2 tests) — map keys had backslashes; assertion expected forward-slashes.
- `audit_doc_refs::severity::matches_archive/matches_issues` (no failures yet, but latent) — `path.to_string_lossy().contains("docs/archive/")` returns false on Windows because the stringified path has backslashes.

The cost of missing these in round 5 was a wasted CI cycle (~15 min) and a confusing "we shipped 6, why are 12 still red" investigation.

**Confirming data points:**
1. Round 5 → Round 6 in this session: 6 boundaries forgot, 1 (delete_orphan_repos) caused total-row-deletion that would have been a destructive bug in production catalog migration.
2. Pending: any future representation choice in a multi-platform codebase.

**Impact:** high — applies to representation choices broadly (path separator, case normalization, encoding). Saves a CI cycle per missed boundary. Could prevent destructive production bugs (R5 nearly did, on the orphan-cleanup path).

**Promote-when:** A second representation-choice rollout that surfaces missed read-side boundaries. At 2 datapoints, promote to `docs/PROGRESSIVE_DISCOVERABILITY.md` (or a new `docs/conventions/cross-platform-representation.md`) as a checklist for representation rollouts: "list every boundary that produces or compares the representation string; apply normalization at each."

**Concrete checklist (provisional, for future rollouts):**
1. Write site (DB upsert / serialization output): normalize on write.
2. ID generation: normalize on hash input.
3. Query LIKE patterns / IN clauses / WHERE comparisons: normalize on query construction.
4. In-memory filter clauses that compare against the stored form: normalize on filter construction.
5. Substring matching against literal forward-slash patterns (`contains("docs/...")`): normalize the path before checking.
6. Test assertions using the representation: assert against normalized form or normalize both sides before compare.

**Status:** validated — single datapoint (this session), but the pattern is broad enough that the next rollout will validate quickly.

## W-7 — Recon-driven distributed-fix sweep closes zombie-open entries

**Observed:** 2026-05-25, triage scout after user prompt "lets look at the
new frictions and fix them." Initial grep across four tracker surfaces
(U-N usage-frictions, H-N hookify, bug-fix session-log, R-N recon-patterns)
returned 4 open F-N entries in the bug-fix session log: F-5, F-6, F-7, F-11.

**Pattern:** When a fix lands as part of a larger labeled commit
(`fix(ci): install mold linker required by .cargo/config.toml`, or as
ambient cleanup folded into a "cumulative session work" merge) rather than
a commit explicitly naming the tracker entry it closes, the tracker entry
stays `open` indefinitely. The bookkeeping gap is silent — no test
fails, no CI gate trips, no PR comment flags it. The entry only flips
when someone manually verifies it.

Two of four "open" F-N entries this session turned out to be already
fixed:

- **F-6** (clippy 1.95 dormant lints) — verified by `cargo clippy
  --all-targets -- -D warnings` exit 0 on current `experiments` HEAD. The
  cleanup shipped distributed across multiple post-2026-05-20 commits and
  was carried to master in merge `d1742c46`.
- **F-11** (CI missing mold linker) — verified by grep on
  `.github/workflows/ci.yml`: 4 occurrences of `rui314/setup-mold@v1` at
  lines 30, 55, 114, 127, matching the proposed fix's 4-job scope exactly.
  Shipped as `29075470 fix(ci): install mold linker required by
  .cargo/config.toml`.

The Index table at the top of the session log was also stale — F-7, F-9,
F-10, F-11 were never added when those entries were inserted. Recon
caught this as a second-order bookkeeping gap and synced the rows.

**Counterfactual:** Without this scout pass, the next "what's open?"
query would have continued to count F-6 and F-11 as actionable backlog.
Concrete cost over time: one or both would eventually pull a session
into re-investigating an already-shipped fix (estimated 15-30 min of
context per re-investigation), or worse — a user-facing release-readiness
report would have advertised them as known issues. Two zombie entries
caught early; over a year of distributed-fix workflow the steady-state
count would likely grow.

**Confirming data points:**
1. F-6 — clippy 1.95 cleanup (this session).
2. F-11 — CI mold linker install (this session).
3. Pending — any future "verify-open-N" scout that flips ≥1 entry.

**Impact:** med — saves N×30min per zombie entry detected, plus
prevents false-positive items in human-facing backlog reports.

**Promote-when:** A third zombie-open detection from a future recon
scout. At 3 datapoints, promote to a project-wide cadence rule in
CLAUDE.md: "Before any 'what's open?' report, run a verify-open pass
on bug-fix session-log entries with `Status: open` older than 14 days —
distributed fixes leave entries zombie-open by default."

**Related patterns:** the same shape applies to `docs/issues/*.md` bug
files whose Fix section cites a SHA but whose `status:` frontmatter never
flipped — see CLAUDE.md Standard Ship Sequence step 4 (the archive-move
discipline) for the analogue at the bug-tracker level.

**Status:** validated — 2 datapoints in one scout pass. Awaiting third
to promote.

## Template for new entries

<!-- Insert new F-N / W-N entries above this line via:
     edit_markdown(action="insert_before",
                   heading="## Template for new entries",
                   content="## F-N — title\n...")
     Also update the matching Index / Wins Index table row at the top. -->
