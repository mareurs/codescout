---
kind: tracker
status: active
title: Codescout Lessons 2026-05-20 — Bug + Hint Follow-up
owners: []
tags: ["foreign-session-feedback", "bugs", "agentic-surface", "hints"]
---

# Session Log — Codescout Lessons 2026-05-20 — Bug + Hint Follow-up

> **Purpose:** Work-stream tracker for follow-up on `codescout-lessons.md` —
> a real LLM coding session's report (Claude Opus 4.7, 2026-05-20, ~170 tool
> calls against `pc-kb-assistant`). The report names 4 concrete bugs in
> codescout MCP tools plus several hint / plugin frictions. This session log
> compounds the bug investigations + fixes that follow.
>
> **Anchor artifact:** `docs/lessons/2026-05-20-codescout-lessons-pc-kb-assistant.md` (tracked, moved from repo root on commit; the foreign session is a Claude Opus 4.7 run against the `pc-kb-assistant` project).
>
> **Scope:** Bugs first (Bug 1–4 + PostToolUse hint consistency + bootstrap-tool
> gap). Tracker-proliferation redesign (lessons §"Tracker proliferation") and
> Iron-Laws session-start tile (P3) deferred to sibling work streams once
> they earn their first move.
>
> **How to use:** Append F-N / W-N entries via
> `edit_markdown(action="insert_before", heading="## Template for new entries",
> content=...)`. Add a row to the Index / Wins Index table for each new entry.
>
> **Lifecycle:**
> - Created 2026-05-20 in response to foreign-session feedback.
> - Appended-to across every session that touches a bug from the lessons file.
> - Entries with `Status: open` carry forward across sessions.
> - Promotion to permanent surfaces (CLAUDE.md, ADRs, `docs/issues/*`)
>   happens when the entry's `Promote-when` / `Fix idea` criteria fire.
> - File archived (moved to `docs/trackers/archive/`) when all 6 in-scope
>   items reach `fixed-verified` or `promoted-to-bug-tracker`.

---

## Index

| ID | Date | Severity | Category | Status | Title |
|----|------|---------:|----------|--------|-------|
| F-0 | 2026-05-20 | n/a | anchor | pinned-as-eval-baseline | `codescout-lessons.md` foreign-session feedback artifact |
| F-1 | 2026-05-20 | high | codescout-tool | fixed-verified | `symbols(name=..., include_body=true)` returns stub + recipe, recipe doesn't resolve — fix shipped on experiments at `bfa2f8bc`, awaits cherry-pick to master |
| F-2 | 2026-05-20 | med | codescout-tool | fixed-verified | `edit_file` rejects entire batch on a single def-containing edit; safe edits in same batch also rolled back (shipped `dd120fc5`) |
| F-3 | 2026-05-20 | med | codescout-tool | fixed-verified | `edit_markdown(action="replace", heading=...)` silently absorbs trailing `---` horizontal-rule separator into next section (shipped `462ad1e4`) |
| F-4 | 2026-05-20 | med | codescout-tool | fixed-verified | `json_path="$.symbols[0].body"` on `symbols(include_body=true)` returns summary not body — same root cause as F-1, same fix (`bfa2f8bc`) |
| F-5 | 2026-05-20 | med | codescout-tool | promoted-to-bug-tracker | LSP-backed tools return opaque `"LSP server disconnected"` when rust-analyzer rustup component is missing; actionable rustup-error stderr is dropped |
| F-6 | 2026-05-20 | med | architecture | fixed-verified | Lessons-file's 3-primitive proposal (Decisions / Session logs / Current state) is a category collapse — 12 observed shapes across 3 projects do not fit cleanly |
| F-7 | 2026-05-20 | med | adoption | open | Frontmatter `topic` (1/36) and `time_scope` (0/36) are zombie columns in codescout trackers — fields exist as catalog columns, nobody fills them |
| F-8 | 2026-05-20 | med | schema | open | Tags semantically overloaded — topic + shape + lifecycle all stuffed into `tags:` because dedicated fields don't exist |
| F-9 | 2026-05-20 | med | schema | open | Tracker-level frontmatter `status:` drifts from entry-level prose `**Status:**` — multiple incompatible enums coexist with no documented authority |
| F-10 | 2026-05-20 | low | docs | open | `[[wiki-link]]` slug convention is half-introduced — does not exist in this repo or MRV or kotlin outside one foreign-imported session log |
| F-11 | 2026-05-20 | low | librarian | fixed-verified | `retired` status missing from `HIDDEN_STATUSES` → in-place redirect trackers (MRV `2-lane-strategy.md`) show in default queries. Fix shipped `2003e91a` |
| F-12 | 2026-05-20 | high | adoption | open | `kind: unknown` is the #1 row in the librarian catalog (550 of ~2k, >25%); discipline gap is at the FIRST axis, not at missing axes |

## Wins Index

| ID | Date | Impact | Pattern | Counterfactual | Status |
|----|------|-------:|---------|----------------|--------|
| W-1 | 2026-05-20 | med | Scout existing `docs/issues/` before drafting new F-N for a foreign-reported bug | Without recon, would have re-investigated `symbols(include_body=true)` from scratch — ~30 min on a fix that landed 2 commits ago; would have written a duplicate fix proposal | validated |
| W-2 | 2026-05-20 | high | After shipping any codescout fix, verify the running MCP binary actually contains the fix — don't trust `cargo build --release` alone | Without the post-`/mcp`-reconnect IL3 probe (`ls docs/issues \| head`), would have claimed F-2 + F-3 + IL3 fixes were live and told the user to test against a binary still at `/home/marius/xxx/codescout` (mtime 2026-05-19 20:18, 1 day stale). User would have re-reported the bugs as still-broken and the cycle would have wasted another session debugging "why is the fix not working" | validated |
| W-2 | 2026-05-20 | high | Catalog SQLite dump trumps schema inspection — query `sqlite3 ~/.local/share/librarian/catalog.db 'SELECT kind, COUNT(*) FROM artifact GROUP BY kind ORDER BY 2 DESC'` before proposing a schema extension | Without this query I would have shipped the 5-axis `shape`/`authoring` proposal; `kind` already has 13 values covering ~80% of the proposed `shape` enum. Saved: 1 redundant column + 1 schema migration PR + per-file backfill | validated |
| W-3 | 2026-05-20 | high | Snow Lion Phase 3 self-revision (iterate 3+ times when confidence is high) — overturned 3-axis → 4-axis → 5-axis → kind-extension across 5 turns | Without iterative Phase 3, the 5-axis proposal would have opened this tracker as a tracker-wide ground truth; 6-9 entries would have cited the wrong schema and required rewriting after W-2's catalog dump landed | validated |

---

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

## F-0 — `codescout-lessons.md` foreign-session feedback artifact

**Observed:** 2026-05-20, controller session reading user-supplied lessons file at repo root.

**When:** User pasted `codescout-lessons.md` (now committed at `docs/lessons/2026-05-20-codescout-lessons-pc-kb-assistant.md`), a ~600-line field report written by a Claude Opus 4.7 session executing Sprint 1 follow-up on `pc-kb-assistant` (a separate Python/Pydantic v2/Vertex AI repo). The report cites every claim with the specific tool call, file, error message, or turn it derives from.

**Expected:** N/A — this entry is the anchor for the work stream, not a friction.

**Got:** A credible, evidence-bound report listing:

- **4 bugs** in codescout MCP tools (Bug 1–4 in the file's §"Bugs" section)
- **6 UX frictions** (§"UX friction")
- **1 systemic observation** (tracker proliferation across 10 surfaces)
- **8 design proposals** P1–P8, ranked by expected impact
- **5 open questions**

**Probable cause:** N/A — anchor entry.

**Workaround:** N/A — this F-N is the eval baseline against which future improvements are measured. Do not close.

**Severity:** n/a

**Status:** pinned-as-eval-baseline

**Fix idea / Pointer:** Each Bug N in the file gets its own F-N entry below. The file lives at `docs/lessons/2026-05-20-codescout-lessons-pc-kb-assistant.md` (moved from repo root on commit to keep the top level clean and to establish `docs/lessons/` as the canonical home for future foreign-session reports).

---

## F-1 — `symbols(name=..., include_body=true)` returns stub + recipe; recipe doesn't resolve

**Observed:** 2026-05-20 (reported in `codescout-lessons.md` §"Bug 1"). Investigation pending in current session.

**When:** Foreign session called `symbols(path="src/pc_kb/corpus.py", name="CorpusConfig", include_body=true)` to read a 34-line Pydantic model body.

**Expected:** Response includes the function body inline (the standard contract for `include_body=true`).

**Got:**

```
src/pc_kb/corpus.py (2)
  Class  60-93  CorpusConfig
      (34-line body — use json_path="$.symbols[0].body" to extract)
  Class  96-97  CorpusConfigError
      class CorpusConfigError(ValueError):
          """Raised when a corpus.yaml is missing required keys or fails validation."""
```

The smaller 2-line class returned inline. The 34-line class returned a stub plus the recipe string `(N-line body — use json_path="$.symbols[0].body" to extract)`. Following the recipe — calling `symbols(..., include_body=true, json_path="$.symbols[0].body")` — returned the same stub. Same with `path` omitted. Same with both `include_body=true` and `json_path` set.

**Probable cause:** Two candidate hypotheses (per author):

1. Size threshold above which `include_body` switches to "summary + recipe" mode is set *lower* than the threshold above which `json_path` extraction actually delivers content. The recipe points at a payload that the same response shape does not contain.
2. The recipe's `json_path` expression is wrong — it doesn't dereference the body in the actual response schema.

Both resolve to: the contract advertised in the hint string does not match the contract the tool implements. Per `[[agentic-surface-as-moat]]` (project Snow Lion memory), this is a surface-contract violation, not an implementation detail — the moat leaks truth on the same response.

**Workaround (foreign session, this entry's data point):** `read_file(path=..., start_line=60, end_line=95, force=true)`. Got the body, worked first call. Author burned 3 round-trips before falling back.

**Severity:** high — second-most-frequent navigation flow after `symbols` overview; blast radius is every codescout session.

**Status:** fixed-verified — fix shipped on `experiments` at commit `bfa2f8bc fix(symbols): always inline body in compact text form` (2026-05-18, 2 commits before current HEAD). Fix removes the elision branch in `format_search_symbols` (`src/tools/symbol/display.rs:155-170`) entirely. Body is now always inlined in compact text form; the misleading `(N-line body — use json_path=...)` hint is gone. Tests `symbols_with_long_body_inlines_full_content` + `symbols_inline_path_makes_body_reachable_without_buffer` assert the fix.

**Verified by recon (this session):**
- `grep INLINE_BODY_LIMIT|use json_path src/tools/symbol/display.rs` → 0 matches at HEAD.
- `git log --oneline -- src/tools/symbol/display.rs` → `bfa2f8bc` is most recent commit on that file.
- `git branch --contains bfa2f8bc` → `experiments` only — NOT on master, NOT in any released binary.

**Why foreign session saw it anyway:** they ran whatever release binary `cargo install codescout` distributes — last published release predates `bfa2f8bc`. The bug is fixed at source but not yet delivered.

**Fix idea / Pointer:**

1. **Canonical bug doc:** `docs/issues/2026-05-18-symbols-body-hint-unreachable.md` (already documents root cause + fix + tests).
2. **Distribution action:** cherry-pick `bfa2f8bc` to master per the Standard Ship Sequence in CLAUDE.md, then cut a new release per the Release Cycle section. Foreign session's pain ends when `cargo install --force codescout` picks up the new release.
3. **Stale-SHA cleanup:** bug doc cites old SHA `a336965d`; the rebase that produced `bfa2f8bc` did not update the doc. Low-priority text fix.

---

## W-1 — Scout `docs/issues/` for matching root cause before opening a new F-N on a foreign-reported bug

**Observed:** 2026-05-20, recon for F-1 (Bug 1 from `codescout-lessons.md`).

**Pattern:** Before drafting an investigation plan for a foreign-reported bug, `grep` the project's own `docs/issues/` + `docs/issues/archive/` for matching symptom strings (error text, hint phrase, function name). If a matching bug doc exists, read its `status:` frontmatter first — it may already be fixed.

**Counterfactual:** Without this scout, I would have proceeded to read `format_search_symbols` and trace the include_body code path from scratch (~30 min), drafted hypothesis tests, and written a fix proposal duplicating the one at commit `bfa2f8bc`. The duplicate work would have produced no value; the foreign session's pain is not solved by another in-repo fix — it is solved by shipping the existing fix to master + a new release. Recon shifted the action from "fix" to "ship," which is the correct lane.

Concrete evidence of the saved cost:
- Grep `(N-line body | use json_path)` matched `docs/issues/2026-05-18-symbols-body-hint-unreachable.md` immediately (1 tool call).
- Frontmatter `status: fixed, closed: 2026-05-18` settled the bug's lifecycle in 1 more read.
- Verifying the fix is on `experiments` but NOT on `master` took 2 more git commands.
- Total cost of recon: ~4 tool calls / ~5 minutes. Total cost averted: ~30 min of re-investigation + a duplicate fix commit that would have rebased or conflicted with `bfa2f8bc`.

**Confirming data points:**

1. F-1 (this session) — lessons file's Bug 1 matched an existing `fixed` bug doc; recon caught the stale-report-vs-fixed-state before any code edit.
2. Pending: any future foreign-session report against a tool whose bug surface this repo has already triaged.

**Impact:** med — saves a duplicate-investigation cost per foreign-session report whose bugs overlap our `docs/issues/` ledger. The lessons file has 4 bugs; if even one more matches an existing doc, the pattern compounds.

**Promote-when:** A second foreign-reported bug (from this lessons file or another) is caught at the `grep docs/issues/` step before any code scout. At 2 datapoints, promote to CLAUDE.md as a step in the foreign-feedback intake checklist: "Before drafting F-N entries for externally-reported bugs, grep `docs/issues/` + `docs/issues/archive/` for matching symptoms. Read frontmatter `status:` first."

**Status:** validated — single datapoint; awaiting promotion criterion.

---



---

## F-2 — `edit_file` rejects entire batch on a single def-containing edit

**Observed:** 2026-05-20 (reported in `codescout-lessons.md` §"Bug 2"). No matching bug doc in `docs/issues/` or `docs/issues/archive/` (W-1 grep pass).

**When:** Foreign session called `edit_file(edits=[edit_0, edit_1])` where `edit_0` was a 3-line text replacement (no `def`/`class` keyword) and `edit_1` was a new function insertion containing `def `.

**Expected:** Either (a) both edits apply, or (b) `edit_1` rejected with a useful error AND `edit_0` applies (partial-apply semantics common in batch APIs).

**Got:**

```json
{
  "ok": false,
  "error": "edit[1]: edit contains a symbol definition (\"def \") — use symbol tools for structural changes",
  "hint": "edit_code(symbol, path, action='insert', body=..., position=...) — inserts before or after a named symbol"
}
```

The `edit[1]:` prefix correctly identifies the offending edit. But `edit_0` was also rolled back, even though it was structurally safe.

**Probable cause:** The iron-law gate (route def-containing edits to `edit_code` to prevent LSP corruption, per BUG-027) fires at validation time before any edit is applied. The current implementation appears to fail the entire batch on the first violating edit rather than partitioning into safe + structural subsets.

**Workaround (foreign session):** Issue two calls — one `edit_file` with `edits=[edit_0]`, one `edit_code(action="insert", position="after", symbol=...)` for `edit_1`. Both work. Cost: 1 extra round-trip per mixed batch; happened twice in the foreign session.

**Severity:** med — not data-loss; the workaround is straightforward; but every mixed batch costs a round-trip and the LLM has to notice the failure and re-plan. Compounds in agentic loops.

**Status:** fixed-verified — shipped `dd120fc5` (2026-05-20), "fix(edit_file): batch detect-&-advise lists safe-edit indices" — the author's recommended option (3). Verified by commit subject during the 2026-06-11 verify-open pass.

**Fix idea / Pointer:** Three options ranked by author (lessons file Bug 2):

1. **Partial-apply** — apply safe edits, reject structural ones with indices listed.
2. **Transparent routing** — detect def-containing edits and route them to `edit_code` automatically. Higher "magic factor"; matches iron-law intent more aggressively.
3. **Detect & advise** — keep all-or-nothing reject but list which edits would have been safe: `"the following edits are safe and would have applied: [0]"`. Lowest-risk, most-informative.

Author recommends (3) as the lowest-risk, highest-information change. Snow Lion concurs: (1) bends the all-or-nothing transactional shape that callers may rely on; (2) leaks the iron-law boundary across two tools and complicates the error surface; (3) is purely advisory — the error contract stays the same, only the hint grows. **Next step:** scout `edit_file` validation code to find the batch-rejection point and assess the cost of adding the "safe-edit indices" listing to the error response.

---

## F-3 — `edit_markdown(action="replace", heading=...)` absorbs trailing `---` horizontal-rule separator

**Observed:** 2026-05-20 (reported in `codescout-lessons.md` §"Bug 3"). No matching bug doc in `docs/issues/` (the existing `2026-05-18-edit-markdown-replace-clobber.md` is a different defect — it covers wholesale-body-replacement-as-documented-behavior, not boundary detection of trailing horizontal rules).

**When:** Foreign session called:

```
edit_markdown(path="docs/trackers/mrv-chat-watch/README.md",
              heading="Scan state",
              action="replace",
              content="<table content>")
```

**Expected:** The `---\n\n` separator between `Scan state` and the next sibling heading is preserved.

**Got:** After the call, the next-heading separator had been eaten — new content butted directly against `## How to use`. Tool itself reported `status: ok`; foreign session caught it via a linter notification. Workaround: a follow-up `edit_markdown(heading="How to use", action="insert_before", content="---\n\n")` restored the separator.

**Probable cause:** The `replace` action consumes the section body up to (but not including) the next sibling heading, then re-inserts new content. If the original section ended with `\n\n---\n\n`, that horizontal-rule line lives in the body's tail. Boundary detection appears to roll the trailing `---\n\n` forward into the next-heading's leading whitespace and discard it.

**Workaround:** Follow-up `insert_before` of `---\n\n` on the next heading. Or: include the trailing `---` inside the `content` argument when calling `replace`.

**Severity:** med — silent in the success case. The tool returns `"ok"` but the resulting markdown is structurally degraded. A less-attentive caller would not notice. Compounds across long-form documents where horizontal rules carry semantic weight (section delimiters in trackers, ADRs, session logs).

**Status:** fixed-verified — shipped `462ad1e4` (2026-05-20), "fix(edit_markdown): replace preserves trailing horizontal-rule separator" — the preferred fix (1) boundary detection. Verified by commit subject during the 2026-06-11 verify-open pass.

**Fix idea / Pointer:** Two paths:

1. **Fix the boundary detection** — the section body's tail should include any trailing horizontal-rule lines that are *not* immediately preceded by another heading. The `---` belongs to the section that emits it. Preferred.
2. **Document the behavior** — callers must include `\n---\n` in `content` if they want a trailing separator preserved. Acceptable but pushes the cost to every caller forever.

Author (lessons file P6) ranks fix at medium priority. Snow Lion concurs: option (1) preserves the agentic-moat contract (the tool does what its name suggests); option (2) externalizes a footgun. **Next step:** scout `edit_markdown` boundary-detection code (likely `src/tools/markdown/edit_markdown.rs`) to find the section-body extractor and inspect how it handles trailing-separator lines.

---

## F-4 — `json_path` on `symbols(include_body=true)` returns summary, not body

**Observed:** 2026-05-20 (reported in `codescout-lessons.md` §"Bug 4 (minor)"). The lessons author noted the surface is different from Bug 1 but the root cause may be the same.

**When:** Same as F-1. Foreign session targeted `json_path="$.symbols[0].body"` explicitly on a `symbols(include_body=true)` call.

**Expected:** Body string returned at the named JSON path.

**Got:** Symbol summary object (line numbers + stub), not the body string. Author hypothesis: either the JSON schema doesn't have a `body` key at that path, OR the extraction doesn't dereference it.

**Probable cause:** Confirmed by recon (F-1 scout): identical to F-1. `format_search_symbols` elided bodies > 500 bytes BEFORE the JSON response was assembled — so the `body` field at `$.symbols[N].body` literally did not exist in the response shape under the `Text` output path. The recipe was a fiction at the protocol level, not a `json_path` parser limitation.

**Workaround (foreign session):** Same as F-1 — `read_file(path=..., start_line=N, end_line=M, force=true)`.

**Severity:** med — distinct surface from F-1 (the lessons author tracked them separately), but resolves on the same fix.

**Status:** fixed-verified — same fix as F-1, commit `bfa2f8bc fix(symbols): always inline body in compact text form`. With the elision branch removed, the body is now inlined into the compact text response; the `json_path` recipe is no longer surfaced because there is no longer a stub to extract from.

**Fix idea / Pointer:** Distribution is the only remaining action — cherry-pick `bfa2f8bc` to master + cut a release. Bug doc `docs/issues/2026-05-18-symbols-body-hint-unreachable.md` covers both surfaces.

---

## F-5 — LSP-backed tools return opaque `"LSP server disconnected"` when rust-analyzer rustup component is missing

**Observed:** 2026-05-20, F-3 fix work. Tried `edit_code` 5+ times to insert a helper function into `src/tools/markdown/edit_markdown.rs`. Every call returned `"LSP server disconnected"`. Initial hypothesis: LSP crashed mid-session. Real cause: it never started.

**When:** First `edit_code` call of the session on a Rust source file. Reconnaissance had used `grep` + `read_file` (tree-sitter only, no LSP) up to that point, so the LSP launch hadn't been triggered earlier.

**Expected:** `edit_code` returns ok or a diagnostic error naming a concrete cause (binary not found, init failed, etc.).

**Got:** Five consecutive failures with the single string `"LSP server disconnected"`. No log path, no hint, no `cause:` chain. The agent (me) tried four workarounds in sequence (retry, retry-with-shorter-body, retry-after-LSP-restart-attempt, fall back to `edit_file` insert=append) before stepping out to investigate.

**Probable cause:** The rustup shim `/usr/lib/rustup/bin/rust-analyzer` was on `$PATH` but the `rust-analyzer` component was not installed for the active toolchain. The shim exits 1 with stderr `error: Unknown binary 'rust-analyzer' in official toolchain 'stable-x86_64-unknown-linux-gnu'.` codescout's LSP-launch path captures the disconnect event but drops the stderr that would have made the failure self-diagnosing. See `docs/issues/2026-05-20-lsp-launch-opaque-disconnected-error.md` for the full investigation, evidence, and proposed fix.

**Workaround:** `rustup component add rust-analyzer` (one shot, ~30 seconds). LSP came up immediately on next `edit_code` call — no codescout restart needed.

**Severity:** med — not data-loss; the workaround is one rustup command. But the diagnostic gap cost ~10 minutes of mid-session debugging and forced a less-ergonomic workaround (`edit_file insert=append` + later inlining of a helper) before the root cause was found. The cost compounds for any fresh-rustup-install user: they will hit this on first use and the error gives them nothing to act on.

**Status:** promoted-to-bug-tracker — full investigation lives in `docs/issues/2026-05-20-lsp-launch-opaque-disconnected-error.md` (open, severity medium, owner marius). This tracker entry keeps the session pointer.

**Fix idea / Pointer:** Two-layer fix proposed in the bug doc:

1. **Capture stderr on launch failure.** When LSP child exits before initialize, surface the last ~1 KB of stderr in the agent-facing error. Detect `Unknown binary` substring and emit a rustup-specific hint (`run rustup component add rust-analyzer`).
2. **Pre-flight check.** Invoke `rust-analyzer --version` (2s timeout) before full launch; fail fast on non-zero exit.

Fix-1 alone closes the agentic-surface gap. Fix-2 is the optimization. Next step: scout `src/lsp/manager.rs` launch path to confirm where stderr is dropped.

---

## F-6 — Lessons-file's 3-primitive proposal collapses 12 observed shapes

**Observed:** 2026-05-20, Snow Lion architecture pass across three projects.

**When:** Reviewing codescout-lessons.md §"Tracker proliferation" — proposal: "3 primitives: Decisions / Session logs / Current state."

**Expected (proposal):** 10 surfaces in pc-kb-assistant collapse cleanly into 3 buckets that generalize to other projects.

**Got (scouted):** 12 distinct shapes in production across codescout (29 active trackers + 8 archive), MRV-poc (38), backend-kotlin (35) — ~110 trackers total. Shapes observed with concrete examples:

- **session-log**: codescout `bug-fix-session-log.md`, MRV `bge-m3-migration-session-log.md`, kotlin `architecture-review-session-log.md`
- **bug-catalog**: kotlin `bug-tracker.md` (BUG-IEL-01..06 with stable IDs)
- **augmented-state**: codescout `tool-usage-patterns.md`, MRV `active-plan.md`, MRV `retrieval-roadmap.md` (librarian artifact-IDs, params+body)
- **dated-audit**: MRV `chunking-pipeline-audit-2026-04-30.md`, MRV `eval-ground-truth-audit-2026-04-25.md`
- **phase-defect**: kotlin `openapi-phase5b-defects-2026-05-07.md` (burn-down)
- **roadmap**: MRV `retrieval-roadmap.md`, kotlin `refactoring-tracker.md`
- **followup**: kotlin `knowledge-injection-future-improvements.md`, MRV `demo-followups.md`
- **goal**: codescout `goal-tracker-cross-pollination.md` (acceptance criteria + closing signals)
- **skill-meta**: codescout `reconnaissance-patterns.md` (R-N ledger for the recon skill)
- **redirect**: MRV `2-lane-strategy.md` (`status: retired`, body forwards to canonical)
- **generated**: MRV `retrieval-issues/RI-NNN.md` (rendered from `registry.json` via `_update_retrieval_issues.py`; manual edit forbidden)
- **design-spike**: codescout `augmentation-prompt-template-resolution.md`, `multi-agent-concurrent-coordination.md`, `plan-lifecycle-tracking.md`, `run-command-pipeline.md` ("Status: Scoping — N options, no decision")

Cross those 12 with `authoring` (hand / augmented / generated) and `time_scope` (open-ended / dated) and the 3-bucket model forces category collapse: a dated audit goes either to "session log" (stretching), "current state" (ignoring the date), or "decision" (only if a fix shipped). Author has to pick. Next reader has to re-derive.

**Probable cause:** Lessons-file author had a sample of one project (pc-kb-assistant). Cross-project comparison — Snow Lion's "two concretes before abstraction" — was missing.

**Workaround:** Reject 3-primitive collapse. Two follow-up moves: (1) extend the existing librarian `kind` enum (already 13 values — plan/doc/spec/tracker/adr/memory/bug/experiment/roadmap/runbook/handoff/eval/unknown) by 5: session-log, audit, followup, goal, skill-meta. Covers ~80% of observed shapes without adding a new field. (2) populate underused frontmatter fields (`topic`, `time_scope`) — see F-7.

**Severity:** med — adopting the proposal would have forced ~110 trackers into 3 buckets, requiring per-file disambiguation prose; the prior turn's alternative 5-axis schema would have introduced a redundant `shape` column (overturned by W-2 catalog dump).

**Status:** fixed-verified — proposal rejected by Snow Lion synthesis across 5 turns; alternative path documented in F-12, W-2, W-3.

**Fix idea / Pointer:** This session log. The actionable downstream work lives in F-12 (`kind: unknown` headline) and the proposed kind-enum extension.

---

## F-7 — Frontmatter `topic` and `time_scope` are zombie columns

**Observed:** 2026-05-20, Phase A spike (backfill on 2 codescout trackers to test the field works).

**When:** Snow Lion Phase 3 validation — query catalog before proposing a richer schema.

**Expected:** Existing trackers populate `topic` and `time_scope` per librarian convention.

**Got:**

```
sqlite3 ~/.local/share/librarian/catalog.db \
  "SELECT COUNT(*) FROM artifact WHERE abs_path LIKE '%code-explorer/docs/trackers/%' AND kind='tracker';"
  → 36

sqlite3 ... "... AND topic IS NOT NULL;"      → 1   (tool-usage-patterns.md, free-text descriptor)
sqlite3 ... "... AND time_scope IS NOT NULL;" → 0
```

Both fields exist as catalog columns (`src/librarian/catalog/artifact.rs:11-17`), queryable via FilterNode, but nobody fills them. The frontmatter template in `docs/templates/session-log.md` declares them as `null` placeholders. The librarian.toml classifier rules populate `time_scope` for some path globs (`docs/reviews/**/*.md` → `time_scope: dated_snapshot`, 34 rows so far) but the convention is undocumented and tracker authors don't extend it.

Backfilled `topic` + `time_scope` on `bug-fix-session-log.md` and `lancedb-upgrade-2026-05.md` this session. Catalog NOT updated — no MCP reindex tool is exposed in this session and the librarian-mcp binary is not built in `target/release/`. End-to-end query test of the spike could not run.

**Probable cause:** Fields exist in schema and template; no enforcement, no example, no value beyond "live by example" — the example is empty. Authors don't trust frontmatter to be load-bearing, so they restate in prose (see F-9) and overload `tags:` (F-8).

**Workaround:** Backfill ~30 codescout trackers with semantic `topic:` descriptors + `time_scope:` values (`open-ended` / `dated:YYYY-MM-DD` / `dated_snapshot`). Document the enum in CLAUDE.md alongside the existing tracker conventions section. Compose with F-12's classifier-rule expansion.

**Severity:** med — adopting NEW frontmatter axes (the prior turn's 5-axis `shape`/`authoring` proposal) would have replicated the same zombie pattern.

**Status:** open

**Fix idea / Pointer:** Phase A spike, deferred. Backfill script (1 SQL query enumerating null-topic rows + filename heuristic + ~30 `edit_markdown(frontmatter={set:...})` calls). Compose with the kind-enum extension proposed in F-6.

---

## F-8 — Tags semantically overloaded (topic + shape + lifecycle all in `tags:`)

**Observed:** 2026-05-20, sampling 30 trackers' frontmatter during cross-project scout.

**When:** Snow Lion Phase 3 validation, V-4.

**Expected:** `tags:` carries cross-cutting topic labels (e.g. `retrieval`, `mcp`, `il3`).

**Got:** Three jobs stuffed into one field. Sample:

| Tracker | `tags:` | Smuggling |
|---|---|---|
| `tool-usage-patterns.md` | `[grep, prompt-quality, iron-law-7]` | topic + quality-axis + rule-ref |
| `cross-pollination-adrs.md` | `[adr, goal-tracker, cross-pollination, reflective]` | shape (`adr`, `reflective`) + topic |
| `goal-tracker-cross-pollination.md` | `[goal, cross-pollination, dogfood]` | shape (`goal`, `dogfood`) + topic |
| `lancedb-upgrade-2026-05.md` | `[lancedb, deps, watching]` | topic + lifecycle (`watching`) |
| `get-guide-topics.md` | `[prompts, get_guide, surface-d]` | topic + workstream marker |

Authors smuggle shape and lifecycle into `tags:` because dedicated fields don't exist. Tags do three jobs.

**Probable cause:** Schema gives no place for shape (`session-log` vs `audit` vs `goal`) or sub-lifecycle (`watching`, `scoping`). Authors invent.

**Workaround:** Either extend `kind` enum (covers shape, see F-6 alternative) and add `status: watching|scoping` to recognized statuses (covers sub-lifecycle, see F-9 and F-11), OR document a tag-prefix convention (`shape:session-log`, `phase:watching`) and migrate. First path is cleaner because the librarian already filters by `kind` and `status` as shortcuts.

**Severity:** med — overload makes `tags:` unusable for clean queries (`find tags=[watching]` mixes topic-tag and lifecycle-tag matches).

**Status:** open

**Fix idea / Pointer:** Composes with F-6 alternative path. Drop tag-overload after kind-enum extension lands.

---

## F-9 — Tracker-level frontmatter `status:` drifts from entry-level prose `**Status:**`

**Observed:** 2026-05-20, grep `^\*\*Status:\*\*` across `docs/trackers/`.

**When:** Snow Lion Phase 3 validation, V-3.

**Expected:** Either one canonical status (frontmatter only) or one canonical place (prose only).

**Got:** Two scopes, drifting independently.

- **Frontmatter `status:` enum** (librarian default per `src/librarian/tools/find.rs:13` + sample): `active | draft | archived | superseded | retired | design | pending_stakeholder_review`.
- **Entry-level body `**Status:**` values** observed in `archive/artifact-code-linkage-session-log.md` and `experiments-to-master.md`: `fixed-verified`, `validated`, `promoted-to-augmentation-prompt-template-resolution`, `promoted-to-CLAUDE.md-convention`, `deferred-two-concretes`, `✅ Ready`, `🔧 Needs review`.
- **Mid-prose `**Status:**` lines** in `augmentation-prompt-template-resolution.md`, `multi-agent-concurrent-coordination.md`, `plan-lifecycle-tracking.md`, `lancedb-upgrade-2026-05.md`: `Scoping`, `Watching`. These don't match either enum and don't match the frontmatter `status:` of the same files.

Different scopes (tracker-as-whole vs individual F-N entry vs prose summary line) collide on the same `**Status:**` syntax.

**Probable cause:** Template doesn't pin which `**Status:**` lines mean what. Authors invent. Lessons-file's "Current state" primitive collapses both scopes — same mistake.

**Workaround:** Pin entry-level status enum in `docs/templates/session-log.md`. Remove redundant prose `**Status:**` lines that just duplicate frontmatter. CLAUDE.md should explicitly state: frontmatter `status:` is tracker-level; body `**Status:**` (within an F-N entry) is entry-level; do not invent a third.

**Severity:** med — silent drift; readers don't know which status is canonical. Catalog queries ignore body-level state entirely.

**Status:** open

**Fix idea / Pointer:** Template change, then audit pass. Composes with F-11 (HIDDEN_STATUSES extension).

---

## F-10 — `[[wiki-link]]` slug convention is half-introduced; does not exist in production

**Observed:** 2026-05-20, grep `\[\[[a-z0-9-]+\]\]` across `docs/trackers/` in three projects.

**When:** Snow Lion Phase 3 validation, V-6 (cross-reference convention audit).

**Expected (lessons-file P5):** `[[slug]]` is a half-introduced convention in the codebase, worth canonicalizing or removing.

**Got:**

```
codescout/docs/trackers/   : 1 file uses [[slug]] — codescout-lessons-2026-05-20-session-log.md
                                                     (foreign-imported)
MRV/docs/trackers/         : 0 files
kotlin/docs/trackers/      : 0 files
```

The convention does not exist in this codebase outside the one foreign import. MRV-poc's CLAUDE.md cites trackers by 16-hex librarian artifact-ID (`5327b18f00b6f82d`, `d41216646b5922f9`) and relative-path. `[[slug]]` doesn't render on GitHub or in VS Code preview.

**Probable cause:** Foreign-session author introduced it in `codescout-lessons.md`; echoed into this session log via copy-paste during W-1's recon.

**Workaround:** Drop `[[slug]]` entirely. Two-style convention is the documented reality: artifact-ID for augmented trackers (`5327b18f00b6f82d`); relative-path for plain ones (`docs/trackers/foo.md`). Update CLAUDE.md cross-reference section if it cites `[[slug]]` anywhere; verify codescout-lessons.md P5 evaporates ("a convention to canonicalize" is actually "a foreign import to revert").

**Severity:** low — the only place it appears is this very file; clean to remove.

**Status:** open

**Fix idea / Pointer:** Pure documentation move. No code change. Audit the [[slug]] occurrence in this file's W-1 and replace with relative-path.

---

## F-11 — `retired` status missing from `HIDDEN_STATUSES`

**Observed:** 2026-05-20, catalog dump of status values for `kind='tracker'`.

**When:** Snow Lion Phase 3 validation, V-11.

**Expected:** Status enum documented in CLAUDE.md (`active | draft | archived | superseded`) is what the librarian recognizes and hides.

**Got:** Catalog shows additional statuses in production:

```
sqlite3 ... "SELECT status, COUNT(*) FROM artifact WHERE kind='tracker' GROUP BY status;"
  active                       : 96
  archived                     : 33   ← hidden by HIDDEN_STATUSES ✓
  draft                        : 24
  design                       :  1
  pending_stakeholder_review   :  1
  retired                      :  1   ← NOT hidden ✗
```

The `retired` row is MRV-poc's `2-lane-strategy.md`. It uses the in-place redirect pattern (file stays at its original path so incoming links keep resolving; `status: retired`; body forwards to canonical successor). Without `retired` in `HIDDEN_STATUSES`, this tracker surfaces in default `find kind=tracker` listings as if it were live work.

**Probable cause:** Status enum drifted with MRV-poc convention; librarian `find.rs:13` not updated. Two valid archival policies in production (codescout's `git mv` to `archive/` + `status: archived`; MRV's keep-in-place + `status: retired`) and the librarian only knew about the first.

**Workaround:** Add `retired` to `HIDDEN_STATUSES`. Comment update documents the in-place-vs-physical-move distinction. Regression test for the new hidden value was DEFERRED because `edit_code` repeatedly failed with `LSP server disconnected` on this branch (see F-5 for the LSP-stability root cause).

**Severity:** low — single concrete (1 row) but the semantic is clear and the fix is mechanical.

**Status:** fixed-verified — commit `2003e91a` (this session).

**Fix idea / Pointer:** Closed at the const change + comment update. Follow-up: `defaults_hide_retired_when_filter_does_not_constrain_status` regression test, deferred behind F-5 (LSP stability).

---

## F-12 — `kind: unknown` is the #1 row in the librarian catalog (550 of ~2k)

**Observed:** 2026-05-20, first-ever query of librarian catalog distribution.

**When:** Snow Lion Phase 3 validation, V-7 (the single most consequential query of the deep-dive).

**Expected:** Most artifacts classify into one of the 13 librarian kinds (plan/doc/spec/tracker/adr/memory/bug/experiment/roadmap/runbook/handoff/eval).

**Got:**

```
sqlite3 ~/.local/share/librarian/catalog.db \
  "SELECT kind, COUNT(*) FROM artifact GROUP BY kind ORDER BY 2 DESC;"

unknown    : 550   ← #1 row, > 25% of all artifacts
plan       : 440
doc        : 316
spec       : 290
tracker    : 156
adr        :  63
memory     :  63
bug        :  44
experiment :   3
roadmap    :   3
runbook    :   3
handoff    :   2
eval       :   1
```

The librarian already supports 13 kinds. The discipline gap is at the FIRST axis (`kind`), not at missing axes. Adding new frontmatter fields (the prior-turn `shape`/`authoring` proposal) before fixing kind classification would be decoration.

**Probable cause:** Classifier rules in `.codescout/librarian.toml` cover only ~10 globs. Default fallback for everything else is `unknown`. Frontmatter `kind:` field is rarely set by authors. The 156 `kind=tracker` rows benefit from a fallback rule (`docs/trackers/**/*.md → kind=tracker`); the 550 `unknown` rows live in directories with no rule.

**Workaround:** Two paths. (1) Expand `.codescout/librarian.toml` to cover docs/research, docs/plans, docs/specs, and other regular directories — one rule = many files. (2) Backfill frontmatter `kind:` on the 550 unknown rows where the right value is obvious from path or content. Path 1 has higher leverage.

**Severity:** high — the catalog is 25%+ unclassified; any query that filters by kind misses a quarter of artifacts. The lessons-file's "proliferation" headline is a symptom; this is closer to the root cause.

**Status:** open

**Fix idea / Pointer:** Audit `kind: unknown` rows to derive the missing classifier rules. Composes with F-6 (kind-enum extension by 5 values) and F-7 (zombie field backfill).

---

## W-2 — Catalog SQLite dump trumps schema inspection in production data

**Observed:** 2026-05-20, Phase 3 validate of the tracker-primitives architectural proposal.

**When:** After two turns of Snow Lion proposing axis-explicit frontmatter schemas (4-axis, then 5-axis with `shape`/`authoring`). About to open the tracker around the 5-axis proposal.

**Pattern:** Before proposing a schema extension, query the existing schema's distribution in the production catalog:

```bash
sqlite3 ~/.local/share/librarian/catalog.db \
  "SELECT kind, COUNT(*) FROM artifact GROUP BY kind ORDER BY 2 DESC;"
sqlite3 catalog.db \
  "SELECT status, COUNT(*) FROM artifact WHERE kind='tracker' GROUP BY status;"
sqlite3 catalog.db \
  "SELECT time_scope, COUNT(*) FROM artifact WHERE time_scope IS NOT NULL GROUP BY time_scope;"
```

One row count distinguishes "field exists and is load-bearing" (`active: 96`) from "field exists and is zombie" (`time_scope: 36 nulls + 3 distinct values—34 of which come from one classifier rule—= ~1 organic value`).

**Counterfactual:** Without this query I would have shipped the 5-axis `shape`/`authoring` proposal from the prior turn as the tracker's seed F-N. That proposal was redundant: `kind` already has 13 values in production, covering ~80% of the `shape` enum I named. The 5-axis proposal would have introduced a new column requiring a librarian schema migration, frontmatter parser update, `find.rs` filter-shortcut for the new field, and per-file backfill on ~110 trackers. Concrete avoided cost: 1 redundant column, 1 schema migration PR, 1 librarian-code-change PR for filter-shortcut, ~110 frontmatter edits, plus the eventual second migration when the redundancy was caught.

**Confirming data points:**
1. F-6 (this session) — 3-primitive proposal was inferred from filename inspection of one project; catalog dump showed the production `kind` enum already covers most of the shape distinction.
2. F-7 (this session) — `topic`/`time_scope` are zombie columns; existence-of-field is necessary but not sufficient.
3. F-12 (this session) — `unknown: 550` headline only visible from row count, not from sampling filenames.

**Impact:** high — preventing a wrong schema migration is high-leverage; the cost asymmetry between "right axis from start" vs "wrong axis, then second migration" is multi-PR-deep and touches frontmatter parsing, catalog SQL, and downstream querying.

**Promote-when:** A second proposal-from-schema-inspection (in this or another project) is overturned by a catalog query, with measurable savings. At 2 datapoints, promote to CLAUDE.md as: "Before proposing a frontmatter schema change, query the existing catalog distribution first: `sqlite3 ~/.local/share/librarian/catalog.db 'SELECT <field>, COUNT(*) FROM artifact GROUP BY <field> ORDER BY 2 DESC;'`. If the field is zombie, fix adoption before extending. If the field is rich, check whether the proposed new field is redundant."

**Status:** validated

---

## W-3 — Snow Lion Phase 3 self-revision caught over-prescription before any tracker work shipped

**Observed:** 2026-05-20, deep-dive into tracker-primitives architectural redesign.

**When:** Across 5 turns of Snow Lion analysis, three confidence revisions:

- Turn N-3: proposed 3-axis schema (kind / status / shape). Over-prescribed.
- Turn N-2: revised to 4-axis (added time_scope). Still missed kind-as-shape.
- Turn N-1: revised to 5-axis (added `shape`, `authoring`). High-confidence claim; was wrong.
- This turn: catalog dump revealed `kind` already has 13 values; `shape` proposal redundant.

**Pattern:** Architecture proposals that survive iterative Phase 3 validation produce stronger conclusions than first-pass confidence indicates. When validation overturns a proposal, the revised proposal is closer to substrate, not further from it. Phase 3 must be run REPEATEDLY for big-blast-radius proposals — single-pass is insufficient.

**Counterfactual:** Without iterative Phase 3, I would have opened this tracker around the 5-axis proposal in turn N-1. Cost: every F-N citing the 5-axis schema (6-9 entries) would have wrong axis citations once W-2's catalog dump revealed `kind`-redundancy. Any backfill PR opened against the 5-axis schema would have introduced a redundant column requiring a follow-up migration. The actual fix that shipped from this work-stream is a 1-line const change (F-11, commit `2003e91a`) — dramatically smaller than any schema proposal Snow Lion entertained.

**Confirming data points:**
1. F-6 (this session) — 3-primitive was wrong; 5-axis was also wrong.
2. F-11 (this session) — the only fix shipped this work-stream was the 1-line `HIDDEN_STATUSES` change.
3. W-2 (this session) — one sqlite query overturned multiple turns of inference.

**Impact:** high — Snow Lion proposals shape downstream PRs; preventing a wrong shape at the source has multi-PR blast radius.

**Promote-when:** A second multi-turn analysis where iterative Phase 3 validate flips a high-confidence proposal in a measurable way. At 2 datapoints, promote to CLAUDE.md as: "Snow Lion proposals carrying schema or architecture changes require at least one Phase 3 validation against production distribution data (catalog dump, import graph, or call-graph sample) BEFORE tracker-opening or PR-drafting. High-confidence claims that have not faced validation are guesses dressed as findings."

**Status:** validated

---

## W-2 — Verify the running MCP binary contains the fix; build alone is not enough

**Observed:** 2026-05-20, after cherry-picking 3 fix commits to master + pushing + force-pushing experiments. User did `/mcp` reconnect and asked me to verify. First test (`ls docs/issues | head -3`) tripped the OLD IL3 logic — the fix that should have allowed bounded-LHS pipes still blocked them. Inspection found `~/.cargo/bin/codescout` mtime was 2026-05-19 20:18 (1 day stale), and the binary had been installed from a stale clone path `/home/marius/xxx/codescout` rather than this working tree.

**Pattern:** After shipping any codescout fix that should be observable through the MCP surface (IL3 rule change, new tool, new hint string, error-message rewording, response-shape change), run a one-line probe through the live MCP that exercises the change BEFORE claiming the fix is live. If the probe behaves the old way, three layers can be stale:

1. **`target/release/codescout`** — stale if `cargo build --release` hasn't run since the change landed in `HEAD`.
2. **`~/.cargo/bin/codescout`** — stale if `cargo install --path . --force` hasn't run since `target/release` was rebuilt. (Often points at a stale clone path, not the active working tree.)
3. **The running MCP server process** — stale if `/mcp` reconnect happened BEFORE the binary update at layers 1 or 2.

The canonical sequence to ship a fix to the live session is:

```
git commit (or cherry-pick) on the source
→ cargo build --release          (refreshes target/release/codescout)
→ cargo install --path . --force (refreshes ~/.cargo/bin/codescout from this tree)
→ /mcp reconnect                 (the running server re-spawns from the new binary)
→ one-line MCP probe that exercises the change
```

None of layers 1–3 surface an error if you skip them — the system silently runs the stale binary. The probe is the only verifier.

**Counterfactual:** Without this probe step, I would have written the W-1 "foreign session blocked until release" claim under the assumption that THIS session's MCP host was already running the fix. The user's next action would have been to test F-2 (mixed-batch edit_file) against a binary still missing the pre-pass, observed the old all-or-nothing rejection, and re-reported it as a regression. Concrete evidence: IL3 fired on `ls | head` immediately after `/mcp` reconnect — a fix that has been on `experiments` for 2 days (`dffaaf84`, 2026-05-18) and on `master` for 0 days had not yet reached the running server. The mtime check (`stat ~/.cargo/bin/codescout`) named the gap in 1 command. Without it, debugging "why doesn't the fix work" would have taken ≥1 session round-trip.

**Confirming data points:**

1. W-2 (this session) — IL3 probe tripped old binary post-reconnect; `stat ~/.cargo/bin/codescout` showed mtime 1 day stale and install-from-path pointing at `/home/marius/xxx/codescout` instead of code-explorer. `cargo install --path . --force` resolved it.
2. Pending: any future fix where the verify-by-probe step catches a stale running binary.

**Impact:** high — prevents a re-report-bug cycle that costs at least one session round-trip. Compounds every time a fix lands. The post-ship probe takes <10 seconds.

**Promote-when:** A second data point confirms the probe step caught a stale-binary case other than the IL3 one. At 2 datapoints, promote to CLAUDE.md "Standard Ship Sequence" as a new step 5 (or to the existing post-cherry-pick instructions):

> *After cherry-pick lands on `master` and push completes, but BEFORE telling the user the fix is live, run `cargo install --path . --force` and a one-line MCP probe that exercises the change. If the probe behaves the old way, the running binary is stale — the `/mcp` reconnect alone is not enough.*

**Status:** validated — single datapoint, observable failure caught + fixed before any user re-report. Awaiting promotion criterion. Related artifact: the surprise install-source path (`/home/marius/xxx/codescout`) suggests a separate F-N for "cargo install --path . --force should be re-run from the canonical project root after any branch/repo move"; deferred until that friction reproduces.

---

## Template for new entries

<!-- Insert new F-N / W-N entries above this line via:
     edit_markdown(action="insert_before",
                   heading="## Template for new entries",
                   content="## F-N — title\n...")
     Also update the matching Index / Wins Index table row at the top. -->
