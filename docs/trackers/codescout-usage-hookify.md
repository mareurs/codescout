---
kind: tracker
status: active
title: Codescout Usage Hookify Candidates — H-N Log
tags:
- pika
- hookify
- promotion-candidates
entry_high_water_H: 8
entry_prefix: H
expects_augmentation: docs/augmentations/docs-trackers-codescout-usage-hookify.yaml
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
  `docs/trackers/archive/pika-phase-1-validation.md`. This is observational
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

**Status:** **NOT shipped as a companion deny — warn-only.** (Was recorded `shipped (deny) — 2026-05-18`; the 2026-06-11 pika audit flagged that as stale and left the cause unconfirmed.)

⚠️ **Resolved 2026-08-06 — the deny hook was orphaned by the `.sh` → `.mjs` port.** Verified at the source, not inferred:

- `hooks/hooks.json:94` registers `${CLAUDE_PLUGIN_ROOT}/hooks/il3-warn-hook.mjs`.
- `hooks/il3-warn-hook.mjs:2` — *"Port of il3-warn-hook.sh. Advisory only: allows the call, injects a context"*.
- `hooks/il3-deny-hook.sh` is present on disk, absent from `hooks.json`, and is still `.sh` while every registered hook in that directory has migrated to `.mjs`.

So the answer to the audit's open question is **neither reverted nor never-registered**: the migration ported the *warn* variant forward and left the *deny* variant behind in the old language. Nothing chose warn-only — it is migration residue. The hard deny observed in practice is codescout's **server-side `run_command` gate**, confirmed live ×4 in the 2026-08-06 session (U-30): each rejection arrived as a companion `additionalContext` warning followed by a server `IL3 violation — … BLOCKED`.

**Why warn-only is worse than it looks.** Because the companion allows the call, its warning is delivered as context *on an already-committed call* — the model reads the rule after choosing the shape, which is the position where a warning changes least. The server then rejects, so the user pays two round-trips for one mistake. Deny at the hook layer with the corrected command pre-composed would cost one.

**Next action (pick one, do not leave both):**
1. Port `il3-deny-hook.sh` → `il3-deny-hook.mjs` and register it in `hooks.json`, keeping the bounded-LHS carve-out (`ls`/`cat`/`awk`/`sed`/`find -maxdepth N`) and the script-internal-pipe exemption that motivated `warn` originally; or
2. Delete `il3-deny-hook.sh`, accept server-side denial as the only gate, and stop describing companion IL3 as a deny rule anywhere.

Option 1 is the better trade only if the hook can also *rewrite* — a deny that merely repeats the server's verdict earlier saves nothing but a little latency. U-30's cases 2 and 3 (compound commands whose first clause is bounded, unbounded pipe hiding in a later clause) are the shapes any new predicate must catch.

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

**Status:** **shipped (deny) — 2026-05-24 (claude-plugins:4587283d).**

**Promotion evidence:**
- U-2: 3 same-turn slips in session `42874b1a`, all blocked by the in-server gate's "Use read_markdown for markdown files" rejection. Same-turn recurrence (3 slips before the in-server rejection changed behavior) was the decisive signal — substrate-route required.
- The original Promote-when bar required "a second user-asked scan (different cc_session_id) writes ≥1 more read_file_markdown slip row". That bar was inherited from H-1's shape but doesn't actually apply here: the in-server gate already certifies the predicate is universally invalid, so the "session-local quirk" concern is N/A. Shipping ahead of the literal bar with this rationale; revising the H-2 Promote-when to "in-server gate confirms predicate universally invalid + ≥1 same-turn recurrence" would have been the methodologically clean alternative but yields the same outcome.

**Hook details:**
- File: `claude-plugins/codescout-companion/hooks/il4-deny-hook.sh` (50 lines, mirrors `il3-deny-hook.sh` shape).
- Predicate: `tool_name` matches `mcp__.*__read_file` AND `tool_input.path` ends in `.md` / `.MD` / `.Md` / `.mD` (case-insensitive `.md` suffix).
- Narrow ship: `.markdown` and `.mdx` extensions NOT matched. Add if usage data shows demand — currently zero observed slips on those extensions.
- Tests: `il4-deny-hook.test.sh` covers 18 cases (deny variants, source-ext allows, narrow-ship allows, wrong-tool sentinels, malformed input). 18/18 PASS.
- `hooks.json` PreToolUse matcher `mcp__.*__read_file` now points at `il4-deny-hook.sh`. Placed immediately after the IL3 entry for symmetry.

**Notes:**
- Asymmetry with H-1: H-1 started `warn` because pipes are legitimate
  inside `bash -c "…|…"` script bodies. H-2 has no analogous
  false-positive — `.md` is `.md`. Direct-deny is correct first ship.
- Same-turn recurrence (3 slips in one turn) is the dominant signal,
  not cross-session count. The model did not learn from the first
  in-server rejection within the turn — memory route too slow;
  substrate route required.

**Valid:** dated 2026-09-02

**Re-verified 2026-09-02 — still shipped and wired; the § *Hook details* above are stale in their
specifics.** The hook is `hooks/il4-deny-hook.mjs`, a **Node** script, not the
`il4-deny-hook.sh` ("50 lines, mirrors `il3-deny-hook.sh` shape") this entry describes; it was
ported at some point and nothing updated the prose. `hooks.json:103` points at the `.mjs` correctly.
The test file is still `il4-deny-hook.test.sh`, which is what makes the drift look like a missing
script rather than a rename.

**I nearly filed a broken-hook bug off that.** A targeted `ls` of the path this entry names returned
*No such file or directory* while its test file sat beside it and `hooks.json` still matched
`il4-deny-hook` — a shape that reads exactly like a live PreToolUse matcher pointing at nothing. What
corrected it was iterating **every** script `hooks.json` references and checking each for existence:
zero missing. A lookup scoped to one name cannot distinguish *gone* from *renamed*; a check over the
whole namespace can. Second occurrence of that exact save within ten minutes — see `I-4`'s
label-vs-content table — which is `R-3` twice in one sweep.



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

**Status:** **shipped (deny) — 2026-05-23.** Test landed at `src/server.rs::tests::companion_surfaces_reference_only_real_tools` (codescout:257d1236) with three layers: positive `mcp__codescout__<name>` matcher check, `*__<name>` case-statement filter check, and known-stale-name sentinel. Walks `../claude-plugins/codescout-companion/hooks/*.sh` + `hooks.json`; skips gracefully when the sibling repo is missing.

**Architectural decision (Snow Lion):** lint lives in codescout, reads companion via stable `../claude-plugins/` relative path. Chosen over alternatives (lint-in-companion, shared tool-list artifact) because the read is opt-in with graceful skip, the runtime dependency direction stays clean (companion → codescout), and a single CI run catches both surfaces.

**Caught latent drift on first run** (validates the design):
- `hooks.json:70` — pre-edit-hint matcher still cited `replace_symbol` (missed by U-14 fix)
- `pre-edit-hint.sh` — header comment + hint text still cited `replace_symbol`

Both fixed in claude-plugins:c95adee. The companion side is now caught up; future drift surfaces immediately in CI.

**Filters added** (lint-FP control, similar to H-5 precursor work):
- Skip `*.test.sh` files (regression sentinels that intentionally cite stale names)
- Allowlist `activate_project` as a host-harness equivalent of codescout's `workspace`
- Scrub shell `#` comments before stale-name check (lets header docs explain consolidation history)

**Notes:** the existing repo-side lint (`prompt_surfaces_reference_only_real_tools`) has a per-token allowlist for non-tool identifiers (param names, etc.). Companion-side lint must mirror that allowlist or expose it as configuration to prevent false positives on legitimate non-tool tokens.



### H-4 — Drop companion compression-reminder once server-instructions survive compaction

**Pattern (original framing):** the companion `SessionStart` hook duplicates Iron Laws content already canonical in `src/prompts/source.md::server_instructions`. Multiple copies in context — three by U-4's count — looked like drift (U-5, U-6), token bloat, and an inversion of "canonical is the source of truth."

**Confirming data (preserved for history):**
- **U-4** (triplication: canonical + companion + buddy).
- **U-5** (compression-reminder dropped the bounded-LHS carve-out for Law 3 — derived surface lost precision).
- **U-6** (compression-reminder cited stale tool names — derived surface drifted faster than canonical).

**Status:** **wontfix — measurement disproved the original predicate (2026-05-23).**

Per `docs/architecture/mcp-channel-caps.md`, the canonical `server_instructions` is delivered ONCE at MCP `initialize` and capped at ~2 KB. After `/compact`, the client does not re-send `initialize`; it reuses the existing session, so the initial response is **not re-served**. The summarized post-compact context contains only whatever the harness compressed.

That makes the companion compression-reminder load-bearing as the **post-compact safety net** — the SessionStart hook re-fires on resume, restoring the Iron Laws in a compact-friendly bullet form. Dropping it would leave the post-compact model without a tool-rules anchor.

**Revised verdict:** keep the companion compression-reminder. The triplication identified in U-4 is correctly layered defense, not redundancy. To prevent drift between the three copies, **H-3 now provides the lint** (companion-surface tool-name check, shipped codescout:257d1236) — drift in the companion copy surfaces as a CI failure rather than silent inconsistency.
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

**Status:** **shipped (enforcing) — 2026-05-24.** CLI subcommand `codescout audit-doc-refs` landed at `src/cli/audit_doc_refs.rs` (wired into `Commands::AuditDocRefs` in `src/main.rs`). CI job `audit-doc-refs` added to `.github/workflows/ci.yml` running on every PR + push to `master`/`experiments`. Gate is now **`--fail-on high`**: any new hi-severity drift fails the build. The previously-blocking historical drift (`docs/adrs/2026-05-13-semantic-anchors-qdrant-payload.md` citing the since-refactored `src/embed/index.rs`) was reconciled by dropping the specific path from the smoke-test verification narrative while preserving the verification claim. Full-tree audit at master HEAD: 0 high-severity findings. F-9 captured a separate drift between docs and code: `fail_on` accepts `med`/`low` in docs but the engine only honors `high`/`any`/`never`; CLI surface restricted to verified values for this session.

**Notes:**
- The audit tool already classifies findings as `verdict ∈ {missing, ambiguous_basename, resolved_basename}`. CI should only fail on `verdict=missing severity≥med`; `ambiguous_basename` is informational (could be a basename collision; not necessarily wrong); `resolved_basename` is OK.
- Once active, this hook closes the loop on U-7 by making the failure mode loud at PR time instead of session time. The companion to H-3 (which catches tool-name drift in companion surfaces): H-5 catches path/link/symbol drift in doc surfaces.

**Valid:** dated 2026-09-02

**Re-measured 2026-09-02, and the load-bearing figure is now false.** The wiring verifies:
`Commands::AuditDocRefs` at `src/main.rs:204`, CLI at `src/cli/audit_doc_refs.rs`, CI job
`audit-doc-refs` at `.github/workflows/ci.yml:601` running
`./target/release/codescout audit-doc-refs --no-emit-tracker --fail-on high --json --project .` at
`:626`. **What does not hold is *"Full-tree audit at master HEAD: 0 high-severity findings"*.**

`librarian(action="audit_doc_refs")` today: 1796 files, 59901 refs, **4 high-severity**, all in this
project. They split two and two, and the split is the finding:

| finding | verdict |
|---|---|
| ~~`docs/manual/src/tools/ast.md:4` → `src/tools/ast.rs`~~ | **RETRACTED — was not drift.** Page deleted 2026-09-02, finding gone |
| `docs/socraticode-borrow-tracker.md:34` → `src/tools/usage.rs` | **not drift either** — the path is named inside a backlog item *describing work on that file* |
| `docs/PROBES.md:146` → `src/serve` | **false positive** |
| `docs/PROBES.md:146` → `src/lsp/m` | **false positive** |

**The "genuine" column above was wrong when first written, and the error is worth more than
the correction.** `ast.md:4` was graded *genuine — path does not exist* by checking whether
`src/tools/ast.rs` resolved, **without reading the sentence containing it**. That sentence was a
tombstone: *"Removed 2026-09-01. … `src/tools/ast.rs` was deleted along with them … This page is
kept as a redirect because older docs, plans and session logs link to it."* The page was a
deliberate redirect with four inbound links, not stale drift — **so the classification reproduced,
by hand, the exact defect it was classifying.** Same for `socraticode-borrow-tracker.md:34`, where
the path sits in a work item *about* that file.

**So all four findings are one defect and none is drift:** `audit_doc_refs` cannot distinguish
**mentioning** a path from **citing** one, which makes the sentence *"X was deleted"*
unrepresentable about `X`. Removal notices, truncation examples and historical work items are all
unwritable under the gate. `IC-6`, both halves — no escape for the inline case, and the escape that
does exist (`<!-- audit-doc-refs:ignore -->`, `parser.rs:447`) is scoped to the next **heading**, so
it cannot name one token: applying it to `PROBES.md`'s section would silence **27** real refs in the
one document whose job is telling you which instrument to trust.

**Resolution taken 2026-09-02 (operator's call, after the correction was put to them):** `ast.md`
deleted outright rather than marked — file and catalog row via `artifact(action="delete")`, since it
was a catalog `doc` artifact and a `git rm` would have orphaned the row; `SUMMARY.md`'s TOC entry
removed with it. **Left untouched deliberately:** the two inbound references in
`docs/issues/archive/2026-09-01-listfunctions-and-listdocs-are-unregistered-tools.md:185` and
`docs/superpowers/plans/2026-03-01-docs-full-audit-plan.md:54,573` — an archived bug file and a
completed March plan are historical snapshots, and this repo's archive rule is that rewriting them
to satisfy a linter falsifies the record. Also untouched: `tests/e2e/harness.rs:359`, whose doc
comment names the deleted tool in the same mention-not-citation way and is correct as written.

**Still open:** the three remaining findings, all needing a parser change rather than a doc change.

**The two false positives are a parser defect, not doc drift, and they are self-referential.**
`PROBES.md:146` mentions `src/serve` and `src/lsp/m` **as quoted examples of truncated paths** —
illustrating a `claude-plugins` bug where a 200-char `args` cut destroys the path it was preserving.
`audit_doc_refs` has no escape for *mentioning* a path, so it read the examples as citations and
graded them high. That is CLAUDE.md § *Parsers Over a Namespace*, no-escape half, verbatim: *"a
documentation example of citation syntax counted as a real citation"* — and it lands on the check
this intervention gates CI with.

**Consequence, and it is not recorded anywhere else:** CI gates on `--fail-on high`. If its
`--project .` scoping sees what this run saw, the gate is either failing on two unfixable findings
(unfixable because the doc cannot cite the example any other way) or its scope differs from the
default paths in a way nobody has written down. **Not resolved here** — my run used default paths
rather than CI's exact invocation, so the honest claim is *the figure moved from 0 to 4 and two of
the four cannot be fixed by editing the doc*. Establishing whether CI is currently red is the next
step and needs the CI invocation run verbatim, not this one.

**Also observed:** `scan_meta.degraded: true`, `lsp_languages_degraded: ["rust"]`, cause
`lsp_behind_index` — which `HY-6` already records as a flag that reports a live LSP as offline. The
four findings above are `file_path` refs and do not depend on the LSP, so the degradation does not
undercut them; it would matter for symbol refs.


### H-6 — Audit classifier: reader-side / placeholder path FP class

**Pattern:** `audit_doc_refs` treats every path-shaped string as a candidate for resolution against the project's `git_root`. Two doc surfaces violate that assumption:
1. **Instructional placeholders** — `path/to/copilot-codescout`, `path/to/codescout/Skills` etc. The doc explicitly tells the reader these are placeholders (line 22 of `docs/agents/copilot.md`: *"as a placeholder for wherever you cloned it"*). Resolver can't know that, reports `missing → hi-sev`.
2. **Reader-side repo paths** — agent-onboarding docs cite `.github/skills/`, `.github/agents/`, `.github/hooks/`, `.vscode/mcp.json` as paths to create in the *reader's* repo, not codescout's. Same structural shape as a real local path; resolver reports missing.

**Confirming data:**
- **U-17** — Post-fix (faa77dd7) measurement: hi-sev concentrated in `docs/agents/copilot.md` (20), `docs/agents/claude-code.md` (14), `docs/agents/cursor.md` (3). The `path/to/` filter dropped 5 placeholder FPs in copilot.md; the residual ~37 are reader-side paths (`.github/...`, `.vscode/mcp.json`, `.cursor/mcp.json`, `.cursor/rules/`) across all three agent-onboarding docs. Bug is in the audit, not in the docs.
- **U-15** — prior FP class of the same family (`origin/master` git refs misclassified as paths). Established the pattern: `looks_like_path` accumulates reject-prefix rules.

**Proposed hookify rule:** layered fix, ship cheapest first.

- **(A) Placeholder prefix reject** — extend `looks_like_path` to reject `path/to/` prefix (catches ~6 of 39 FPs in agent docs). One-line addition in `src/librarian/tools/audit_doc_refs/parser.rs`, next to the existing `origin/` / `upstream/` rejections. **Shipped** with this entry — codescout:faa77dd7.
- **(B) Per-doc opt-out via frontmatter** — recognize `audit_file_paths: false` in markdown frontmatter to skip path resolution for that file. Cleanest long-term: lets each doc owner declare reader-side intent. Requires parser to read frontmatter, schema docs, and a per-finding suppression mechanism.
- **(C) Default scope exclusion** — exclude `docs/agents/**` from `DEFAULT_AUDIT_GLOBS` (currently `["docs/**/*.md", "CLAUDE.md", "**/CLAUDE.md", "**/README.md"]`). Cheapest catch-the-rest fix but loses coverage for any *real* drift inside agent docs.

**Recommendation:** ship (A) now (small, same shape as U-15 fixes, low risk). Defer (B) vs (C) to a design call — (B) is more principled but more code; (C) is one-line but lossy. Empirical input needed: are there any *real* drift findings in `docs/agents/*.md` that (C) would silence? If no, (C) is safe; if yes, (B) is required.

**Promote-when:**
- **(A) → shipped** when the parser test `parser_rejects_path_to_placeholder` lands on `master`.
- **(B) or (C) → shipped** when one of:
  - A third reader-side / placeholder FP class lands in U-N (suggests the reject-prefix list approach won't scale).
  - The hi-sev count from agent docs blocks H-5's deny-stage promotion in practice (i.e. clean CI is otherwise within reach but agent-doc FPs are the residual).

**Status:** **shipped — 2026-05-24.**

- **(A) Placeholder prefix reject** — **shipped** codescout:faa77dd7. Catches `path/to/X` placeholders in `looks_like_path`. Dropped ~5 placeholder FPs.
- **(C) Default scope exclusion** — **shipped** codescout:9fa04f0b. New constant `DEFAULT_AUDIT_EXCLUDES = ["docs/agents/**"]` applied only when `args.paths` is None. Explicit `paths` argument bypasses the exclusion so callers can audit the subtree on demand. Tests: `default_scan_excludes_docs_agents` + `explicit_paths_override_default_exclude`. Dropped ~29 hi-sev refs.
- **(B) Per-doc frontmatter opt-out** — **not shipped, deferred indefinitely.** Cleaner per-doc design but (C) was sufficient: with `docs/agents/**` excluded by default, the only place the reader-side FP class would resurface is a new sibling directory of agent docs (e.g. `docs/integrations/`). At that point either extend the exclude constant or reconsider (B).

**Adjacent discoveries during the U-17 triage that landed in their own commits:**
- **Class B resolver fix** (codescout:68840b4b) — was NOT in the original H-6 scope. Surfaced during the docs/agents FP categorization: 8 of the 40 hi-sev refs were `../manual/...` cross-doc links that the resolver was joining to repo_root instead of `md_file.parent()`. Real audit bug; benefits the whole project. Tests: `resolver_link_with_dot_dot_resolves_relative_to_md_file_parent` + 2 siblings.
- **docs/agents/*.md content refresh** (codescout:01ec2890) — stale tool-name references (`list_symbols` × 5, `find_symbol` × 3, `search_pattern`, `find_file`) and an incorrect multi-project workspace.toml example. These would have been silent drift if the audit FPs had stayed in the way.

**Notes:**
- This is the third FP class identified in the audit. First two (U-15: Rust `::` separator + git refs) shipped 61bc678b. Pattern is clear: classifier needs an extensible reject mechanism. Could justify a refactor into a single `REJECT_PREFIXES: &[&str]` constant + iter check; not done now to minimize blast radius.
- The classifier-can't-model-intent diagnosis is the **persistent** root cause. Each FP class is a symptom; the deeper question is whether `audit_doc_refs` should ever resolve paths it didn't explicitly recognize as local-intent. An "allowlist" approach (only resolve paths matching `^(src|docs|tests|scripts|target|Cargo)/`) would invert the current default — would need its own design call.


### H-7 — Do NOT deny `read_file` on source extensions (rejected); at most warn on full no-range large-source reads

**Pattern:** A transient proposal this session was to deny `read_file` on source extensions (`.rs/.kt/.ts/.go/…`), mirroring H-2's `.md` deny. Investigation **rejects** it.

**Confirming data:**
- **U-27** — `usage.db` across 4 projects: source `read_file` is 82–94% sliced line-range reads; imports / glue / macros / lossy-language reads have **no `symbols` equivalent**. A blanket deny would block the legitimate majority.
- Mechanism: `read_full_file` already redirects large full-reads to a symbol outline and emits a "prefer symbols" hint on small ones — the only waste case is *already* self-governed. A deny is redundant where it would be safe and harmful where it would bite.

**Proposed hookify rule:** **none (rejected).** If anything, a *warn* (never deny), scoped to the narrow predicate below — and even that is low-value given the tool's self-governance:
- **Predicate:** `read_file` with a source extension AND no `start_line`/`end_line`/`offset`/`limit` AND the file is indexed & large.
- **Decision:** warn only.
- **Reason text:** "Full read of a large source file returns the symbols outline anyway — use symbols(path) for structure, symbols(name=…, include_body=true) for a body, or a line range for imports/glue."

**Promote-when:** do NOT promote. Revisit only if `usage.db` shows a *recurring, costly* full-no-range large-source read pattern the tool's own redirect fails to curb.

**Status:** **deferred — rejected by design (tool self-governs).** Contrast H-2: the `.md` deny is correct because `read_markdown` *fully supersedes* `read_file` for markdown. No analogous superseding tool exists for source — `symbols` is a lossy projection, not a replacement.

**Adjacent (sibling F-22):** `pika_observations` keys on errors, not silent-`success`, so silent-success misuse is invisible to a Pika scan (F-22 filed a follow-up). Relevant here: a future warn-hook would be the *only* way to observe the full-no-range pattern, since the DB scan structurally cannot.


### H-8 — Native `Bash` is invisible to `usage.db`, so no Pika scan can measure shell-mediated tool misuse — and an eval is live on exactly that axis

**Pattern:** Every H-N above proposes a hook whose value is judged against `usage.db`. That instrument records **codescout MCP calls only**. When shell work goes through native `Bash` rather than `run_command`, it leaves no row — so any misuse committed through the shell is not merely under-counted, it is **structurally unobservable**, and a Pika scan returns a clean, plausible number rather than an error.

**Confirming data (measured 2026-09-01, this repo):**

```
sqlite3 .codescout/usage.db \
  "SELECT COUNT(*), SUM(tool_name LIKE '%ash%'), COUNT(DISTINCT tool_name) FROM tool_calls;"
→ 52769 | 0 | 26
```

**52,769** recorded calls, **zero** matching `%ash%`, 26 distinct tool names — all codescout tools (`run_command` 19,398; `grep` 5,694; `read_file` 5,213; `symbols` 4,394). Re-derived here rather than carried from memory `gotchas`, which asserts the same thing dated 2026-08-27.

Session evidence, **corrected downward by `H-7` and stated with its unit**: this session committed **3** Iron-Law-1 violations through `Bash` where a codescout tool fully supersedes — one `awk` range-extraction of a function body (`symbols(name="seed_ledger", include_body=true)` returns the same body *plus* the doc comment the awk range silently dropped, verified by running it), and two `grep -n '<symbol>'` on a `.rs` file (`references(symbol, path)` / `grep(pattern, path)`). The count was first written as **5**; `H-7` adjudicates the other two out — they were `read_file` line-range reads, which is precisely the 82–94% sliced-read class `H-7` measured and defended. The inflated figure is recorded because it is the failure this entry is about: the number was assembled by someone who had not yet read the ledger's own prior ruling.

**This does NOT contradict `H-7`; it is the half `H-7` could not see.** `H-7` rejected denying `read_file` on source extensions on measured grounds — `symbols` is a lossy projection, not a replacement, and the tool already self-governs via `read_full_file`'s redirect. All three of those properties are properties of **a codescout tool**. Native `Bash` has none of them: no redirect, no hint, no row. `H-7`'s own closing note already names the shape — *"`pika_observations` keys on errors, not silent-`success`, so silent-success misuse is invisible to a Pika scan"* — and this is that argument one layer out, where the blindness is total rather than partial.

**Why it is urgent rather than tidy:** `CLAUDE.md` records `security.shell_command_mode` as a **live eval arm** — the project is actively measuring native `Bash` against `run_command` and has not ruled. The instrument that would settle it can see **one arm**. Any verdict drawn from `usage.db` during this evaluation is a comparison between a measured arm and an unmeasured one, and it will read as a clean result. That is a measurement-integrity defect under an in-flight decision, not a hook wish.

**Proposed hookify rule — a RECORDER, not a gate:**

- **Predicate:** `PostToolUse` on native `Bash`.
- **Decision:** neither `deny` nor `warn` — **record**. Write a `tool_calls` row with `tool_name='bash'`, the command's first token, and a classification of the argument's extension, so the existing schema can see the arm.
- **Reason text:** none surfaced to the agent. A recorder that nags is a warn hook wearing a disguise, and `H-7` is the standing evidence that warns on this surface are low-value.

**Explicitly NOT proposing a deny on `Bash`-reads-of-source.** Two reasons, and the second is the one that would have bitten: `H-7`'s reasoning partly transfers even though its evidence does not; and a deny keyed on "Bash touches `*.rs`" would have blocked this session's `sed -i` mutation runs, which were the work rather than the mistake (`bug-fix-session-log:F-94` is their product). **Gate nothing until the recorder has produced a base rate** — proposing a threshold before the measurement is what `H-7` had to spend a `usage.db` sweep to undo.

**Promote-when:** the recorder ships and 2+ weeks of rows exist. *Then* ask whether a warn is justified, against a real denominator. If the `shell_command_mode` eval concludes before that, the recorder is still the thing that makes the conclusion checkable afterwards.

**Status:** proposed — recorder only; no gate, no threshold, no carve-out yet.

**Valid:** dated 2026-09-01

**Rests on:** the `sqlite3` count above, run in this repo this session; `H-7`'s U-27 sweep for the adjudication that cut 5 → 3; and `CLAUDE.md` § *Companion Plugin* for `shell_command_mode` being an open arm rather than a setting.
