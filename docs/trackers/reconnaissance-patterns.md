---
kind: tracker
status: active
title: Reconnaissance patterns
owners: []
tags:
  - reconnaissance
  - skill-meta
  - scout
---

# Reconnaissance patterns

Per-project R-N ledger for the `codescout-companion:reconnaissance`
skill in this project. See the canonical bootstrap, append rules,
sync flow, and R-N entry template in the skill's
`SKILL.md` and `references/reconnaissance-patterns-template.md`.

Three buckets: **hits** (scout caught drift), **misses** (scout missed,
downstream gate caught), **proposals** (vocabulary expansions for the
skill).

## Index

| ID | Date | Verdict | Pattern | Evidence (session-log) |
|----|------|---------|---------|------------------------|
| R-1 | 2026-05-19 | hit → promoted | Pre-dispatch grep for asserts on `include_str!`'d constants | mcp-prompt-redesign F-1 + W-1 |
| R-2 | 2026-05-19 | miss | Scout missed constant-write patterns (`.replace(TOKEN, ...)`) | mcp-prompt-redesign F-2 |
| R-3 | 2026-05-19 | miss → promoted | Scout limited grep to one file/crate; cross-file asserts slipped | mcp-prompt-redesign F-2 |
| R-4 | 2026-05-19 | miss | Grep undercounts struct-field construction sites by 2-3× | mcp-prompt-redesign F-3 + W-2 |
| R-5 | 2026-05-19 | proposal | Add "compiler as scout" as a Phase-1 tool alongside grep | covers R-4 |
| R-6 | 2026-05-28 | hit | Explicit recon invocation on substrate before mechanism design | prompt-guide-refactor F-2 + W-2 |
| R-7 | 2026-05-28 | miss → applies-R-1 (promoted) | Invariant test on `include_str!`'d file not pre-enumerated | prompt-guide-refactor F-4 + W-3 |
| R-8 | 2026-05-28 | miss → proposal | `edit_markdown(action='replace')` shape unverified on marker-bearing section | prompt-guide-refactor F-7 |
| R-9 | 2026-05-28 | proposal → drafted | Session-state recon for subagent dispatch | prompt-guide-refactor F-6 + W-4 |
| R-10 | 2026-05-29 | miss → proposal | Buffered tool output parsed for structured extraction without a completeness scout | metadata-filtering F-4 + W-1 |
| R-11 | 2026-05-30 | hit → proposal | Concept docs diverged from code on concurrency semantics (GRADLE_USER_HOME "isolation"; per-path mux) | issues/2026-05-30 concurrency bug files |
| R-12 | 2026-05-30 | hit → proposal | Plan's proposed data structure cited the symptom layer, not the structural layer (flat `ActiveProject` HashMap vs existing `Workspace` registry) | concurrency-fix F-1 |
| R-13 | 2026-05-30 | hit → proposal | Cross-repo doc drift: codescout `CLAUDE.md` stale vs `claude-plugins` hook (cd-passthrough removed, wrong filename, +9 undoc'd hooks); intra-repo `audit_doc_refs` structurally can't see it | commit 7187396a |
| R-14 | 2026-06-01 | hit (confirmed) | Specialist cited a dated memory (`outputguard-cross-cutting-law`, 2026-05-07) as a load-bearing design claim (`@ref` buffers process-local); scouted current code before the design rested on it — confirmed | `output_buffer.rs:42` |
| R-15 | 2026-06-03 | hit | Scout external-tool on-disk state against bug-doc claims before a fix depends on addressing it (analyzer dir 128-bit hash ≠ codescout 64-bit `ws_hash`) | kotlin-lsp-disk F-1 |
| R-16 | 2026-06-04 | hit → promoted | Pre-dispatch scout of the plan's OWN splice code caught a double-newline bug (+ substring-overlap test mis-routing 3× → CLAUDE.md; whole-workspace `cargo fmt` churn caught at pre-commit diff-scout) | edit_file-normalized-fallback (this session) |
| R-17 | 2026-06-05 | hit | Spot-check sibling callers of a just-fixed shared helper before closing the bug class (`references(clamp_range_to_parent)` found `do_remove`/`do_replace` shared the off-by-one) | bug-fix W-9 + issues/2026-06-05-edit-code-insert-after-last-python-method |


## R-1 — Pre-dispatch grep for asserts on `include_str!`'d constants

**Verdict:** hit

**Observed:** 2026-05-19, MCP prompt channel redesign work stream
(`docs/trackers/mcp-prompt-redesign-session-log.md` F-1, W-1).

**Pattern:** Before rewriting a content file (`source.md`, embedded
templates, etc.) that backs a static constant via `include_str!`,
grep the codebase for asserts on that constant. Specifically:

```
<CONST>.contains(...)
<CONST>.find(...)
<CONST>.matches(...)
snapshot calls naming the surface file
```

Enumerate every test that will fail post-rewrite and name them in
the implementer's dispatch prompt.

**Evidence:** Without R-1, U4 implementer would have run the 4
planned `redesign_invariants` tests, hit 6 unplanned
`SERVER_INSTRUCTIONS`-asserting failures, and either reported
DONE_WITH_CONCERNS or BLOCKED. Estimated cost saved: 6-12 subagent
round-trips.

**Counterfactual confirmed by:** F-1 enumeration in
`mcp-prompt-redesign-session-log.md`, evidenced by ≥4 tests deleted
during U4 that were NOT in the plan's "1 test may break" prediction.

**Promote-when:** R-1 already validated once. Promote to SKILL.md
after a second `include_str!` rewrite work stream confirms the
pattern. Concrete addition: `SKILL.md § Phase 1 — Scout`, sub-bullet
"For `include_str!`'d content files, grep `<CONST>.contains / .find /
snapshot` to enumerate asserting tests."

**Status:** promoted to SKILL.md (claude-plugins:f842848, 2026-05-28). Added as a 5th bullet under Phase 1 — Scout, citing R-1 + R-7 by name with the loophole-closing cross-reference from the "When NOT to Use" rewrite (same commit). Promote-when criterion fired with 2/2 datapoints — R-1 (mcp-prompt-redesign work stream, 2026-05-19) and R-7 (this session's prompt-guide-refactor F-4 + W-3, 2026-05-28).

---

## R-2 — Scout missed constant-write patterns (`.replace(TOKEN, ...)`)

**Verdict:** miss

**Observed:** 2026-05-19, same work stream
(`mcp-prompt-redesign-session-log.md` F-2).

**Pattern that failed:** The scout grepped reads of the constant
(`<CONST>.contains`, `.find`, etc.) but did NOT grep *writes into*
the constant via runtime token substitution (`SERVER_INSTRUCTIONS
.replace(SYMBOL_NAV_TOKEN, &nav_content)`). When the token left
`source.md`, the `.replace` became a silent no-op — the
language-specific nav block was dropped at runtime. Recon missed it;
the spec reviewer flagged it during U4 review.

**Cost absorbed:** 1 extra fix-up subagent dispatch (U4 fix-up).

**Pattern proposal (folds into R-5):** Phase 1 grep should include
constant **writes** as well as reads:

```
<CONST>.replace(<TOKEN>, ...)
<CONST>.replacen(...)
write_str! / format! using the constant
```

For string-substitution prompts, also enumerate every `TOKEN`-style
constant declared near the surface and grep callers.

**Promote-when:** R-2 + one more "write-side substitution missed"
miss → promote the expanded grep vocabulary to SKILL.md.

---

## R-3 — Scout limited grep to one file/crate; cross-file asserts slipped

**Verdict:** miss

**Observed:** 2026-05-19, same work stream
(`mcp-prompt-redesign-session-log.md` F-2, second half).

**Pattern that failed:** The scout grepped `src/prompts/` for
asserts on the rewritten content. A 7th broken test
(`server_instructions_documents_goal_tracker_discovery`) lived in
`src/server.rs` — outside the scout's grep scope. Recon missed it.

**Pattern proposal:** Phase 1 grep must default to the **workspace
root**, not the directory of the file being changed. Constants and
their callers cross crate / module boundaries; assertion sites do too.

**Cost absorbed:** 1 extra deletion in the U4 fix-up.

**Promote-when:** R-3 already validated as a needed default. Cheap
fix: add a sentence to `SKILL.md § Phase 1 — Scout` — "Grep scope
defaults to workspace root, not the file being modified."

**Status:** promoted to SKILL.md (claude-plugins:787cdec0, 2026-05-23). Added as a 4th bullet under Phase 1 — Scout, citing this R-3 row by name. Promote-when criterion fired with 1/1 datapoint, per the tracker's note ("already validated as a needed default").

---

## R-4 — Grep undercounts struct-field construction sites by 2-3×

**Verdict:** miss

**Observed:** 2026-05-19, same work stream
(`mcp-prompt-redesign-session-log.md` F-3, W-2).

**Pattern that failed:** For "add a required field to widely-used
struct", scout grepped `ToolContext\s*\{|ToolContext::new` and
counted 13 sites. Reality required ~30 (one test file alone had 24
construction sites — many on single lines the regex matched once
per file rather than per occurrence; many nested inside macros and
helper factories).

**Cost absorbed:** Implementer fell back to a `perl -i -0pe` bulk
pass driven by `cargo build` errors. Two files double-inserted;
deduped manually. Net result correct but the controller-side scout
gave a wrong estimate of blast radius.

**Pattern proposal (covered by R-5):** For exhaustive enumeration
of construction sites of a struct that gains a non-`Option` field,
use `cargo build` as the scout. The compiler reports every missing
field; grep only approximates.

**Promote-when:** validated once already. Pairs with R-5 for the
expansion.

---

## R-5 — Add "compiler as scout" as a Phase-1 tool alongside grep

**Verdict:** proposal

**Source:** R-4 + W-2 in
`docs/trackers/mcp-prompt-redesign-session-log.md`.

**Proposal:** `SKILL.md § Phase 1 — Scout` currently lists grep,
`symbols`, and `references` as the scout's tools. Add a fourth:

> **For non-`Option` field additions and similar exhaustive
> enumeration problems, use the compiler as scout.** Add the field
> (or whatever forces every site to update), run `cargo build`, and
> let the compiler enumerate every site via "missing field" errors.
> This is exhaustive by construction. Grep is for *finding* a
> representative site; the compiler is for *counting* all of them.

**Why this is a phase-1 tool, not a phase-4 fallback:** the scout's
job is to estimate blast radius before dispatch. Wrong blast radius
estimate → wrong dispatch (one subagent vs N, or one prompt with 13
enumerated sites vs the right "use compiler-driven enumeration"
instruction). The compiler-as-scout pattern *informs the dispatch
prompt itself*, not just the implementation.

**Caveats:**
- Works only when the change *forces* all sites to update (required
  field, trait method without default, etc.). Default-`None`
  optional trait methods don't trigger compile errors.
- Cost: one `cargo build` cycle per scout pass. For codescout that's
  ~30-60s — acceptable.

**Threshold to promote:** R-4 + one more datapoint where a
struct-field-style change benefits from this approach. Currently
1/2.

---

## R-6 — Explicit recon invocation on substrate before mechanism design

**Verdict:** hit

**Observed:** 2026-05-28, prompt+get_guide refactor work stream
(`docs/trackers/prompt-guide-refactor-session-log.md` F-2 + W-2).

**Pattern:** Before locking the v1 design for a new runtime mechanism
(in-band hard-injection of get_guide content), invoked
`/codescout-companion:reconnaissance` to scout the actual substrate.
Read `ToolContext::guide_hints_emitted`, `CodeScoutServer::build_context`,
the workspace-reset trigger at `ActivateProject::call`, and existing tests
at `server.rs:2711-2840`. Discovered the ledger lives on `CodeScoutServer`
(per-MCP-session, shared via Arc across all per-request ToolContexts
including subagents) — NOT on `Agent` state as the brainstorm had assumed.

**Evidence:** Without the scout, task #3 in the brainstorm would have
shipped a parallel per-Agent ledger, conflicting with the existing one
(2 sources of truth) or superseding it (breaking 6 existing tests at
`src/server.rs:2711-2840`). The substrate finding ALSO vindicated Iron
Law 6 architecturally — subagents are structurally blind to topics the
parent triggered (W-2), so the "parent must brief" law isn't stylistic
but substrate-mandated.

**Counterfactual confirmed by:** F-2 and W-2 in
`docs/trackers/prompt-guide-refactor-session-log.md`. Recon-before-build
prevented at least 150 LOC of duplicate mechanism, AND surfaced the
architectural reality that anchors Iron Law 6.

**Promote-when:** R-6 is a single datapoint of "explicit invocation
produces win" — pair with R-1 type hits to argue for promoting "always
scout substrate state before locking a design that assumes specific
storage" to SKILL.md. Currently 1/2.

---

## R-7 — Miss: invariant test on `include_str!`'d file not pre-enumerated (R-1 applies)

**Verdict:** miss → applies-existing-pattern R-1

**Observed:** 2026-05-28, same work stream
(`docs/trackers/prompt-guide-refactor-session-log.md` F-4 + W-3).

**Pattern that failed:** Added Iron Law 6 (+282 bytes) to
`src/prompts/source.md` without first enumerating invariant tests on
the rendered `server_instructions` slice. The 2200-byte cap test
`source_md_under_cap` at `src/prompts/mod.rs:1037-1046` fired
loudly on `cargo test --lib prompt`, blocking the edit. R-1
("Pre-dispatch grep for asserts on `include_str!`'d constants",
validated 2026-05-19) would have caught this — `MAX_INSTRUCTIONS_CHARS`
and the `redesign_invariants` module both turn up in a 5-second grep.

**Pattern proposal (folds into R-1 promotion):** R-1 was already
validated as needing SKILL.md promotion; this is the second datapoint.
Cost of skipping recon here: 1 failed `cargo test`, 1 surgical cut to
make room (`Gate:` quote lines in Iron Laws 2/4/5), 1 amend cycle.
Estimated time penalty: ~5 minutes vs. ~30 seconds for the grep.

**Cost absorbed:** 1 minor scope expansion (gate-quote cut bundled
into the Iron Law 6 commit) + 1 amend on a working-tree-recovery
incident downstream. Recoverable.

**Promote-when:** R-1 promotion is now 2/2 datapoints (R-1 + R-7).
Ship the SKILL.md promotion this turn or next.

**Status:** R-1 promotion triggered same turn (claude-plugins:f842848, 2026-05-28). R-7 serves as the second datapoint that closed R-1's promote-when criterion; the new SKILL.md Phase 1 Scout bullet names BOTH R-1 and R-7 as evidence.

---

## R-8 — Miss: `edit_markdown(action='replace')` shape unverified on marker-bearing section

**Verdict:** miss → proposal

**Observed:** 2026-05-28, same work stream
(`docs/trackers/prompt-guide-refactor-session-log.md` F-7).

**Pattern that failed:** Used `edit_markdown(action='replace',
heading='## Deeper guidance', ...)` on `src/prompts/source.md` without
first scouting the section's body. The body contained inline
`<!-- @end -->` and `<!-- @surface onboarding_prompt -->` HTML-comment
markers that demarcate prompt surfaces; replace wiped them, breaking
the build (`surface 'onboarding_prompt' not found`). Hit a second time
on the next edit attempt — lost the intro paragraph that lived between
the `<!-- @surface onboarding_prompt -->` opener and the next heading.
Both losses were caught only by the build's downstream gates
(extract_surface panic, snapshot test regen detecting the gap on diff).

**Pattern proposal (new vocabulary for SKILL.md § Phase 1 Scout):**
*"When using `edit_markdown(action='replace')`, FIRST read the
section's body with `read_markdown(heading=...)`. Replace overwrites
the entire body verbatim. If the body contains structural HTML-comment
markers (`<!-- @surface NAME -->`, `<!-- @end -->`, project-specific
sentinels), the new content must explicitly include them or the
replace will drop them silently."*

The F-7 fix (commit 80f2fbca) adds a programmatic gate that catches
this at the editor level. R-8 is the human-discipline counterpart
that prevents the gate from ever needing to fire.

**Cost absorbed:** 2 edit attempts, 1 destructive working-tree recovery
incident (separately captured at `~/.buddy/memory/common/never-git-checkout-to-exclude-wip.md`),
1 commit amend. ~15 minutes of friction + 1 erosion of user trust.

**Promote-when:** R-8 + one more "replace dropped structural content"
incident (e.g. in a tracker template that has separator lines) → promote
to SKILL.md § Phase 1 Scout. Currently 1/2.

---

## R-9 — Proposal: session-state recon for subagent dispatch

**Verdict:** proposal

**Source:** F-6 in
`docs/trackers/prompt-guide-refactor-session-log.md` + the verification
subagent's self-assessment (W-4).

**Pattern that failed:** Dispatched a subagent with full Iron Law 6
briefing (file paths, symbol names, F-2/W-2 finding pointer). The
briefing was rated "self-discovery cost ≈ zero" (W-4) but the parent's
predictions about V2 auto-inject behavior were wrong — the subagent's
first `symbols()` call DID fire `progressive-disclosure` injection
that the parent claimed was already triggered. Cause: the parent
didn't communicate the **session-state ledger** — which topics had
actually been re-triggered in the post-`/mcp`-reconnect window.

**Proposal:** Add to SKILL.md § Phase 1 Scout, sub-bullet for
subagent-dispatch case:

> **For subagent dispatch, also scout session-level state** — what
> topics has the parent triggered, what workspace is active, what's
> already in the @ref buffer. The `guide_hints_emitted` ledger is
> per-MCP-session and shared between parent and subagent; the subagent
> can't see it from inside a tool call. Brief it explicitly:
> *"I've triggered: [librarian, progressive-disclosure]"* lets the
> subagent predict its own V2 auto-inject behavior accurately.

**Why this is a phase-1 tool, not a phase-4 fallback:** the scout's
job is to enumerate what the subagent will need. Session-state IS
context the subagent needs; without it, the subagent makes wrong
predictions (per W-4 Section E) and the parent's prediction becomes
falsifiable rather than substrate-derived (per F-6).

**Caveats:**
- The `guide_hints_emitted` ledger has no read-only query tool; the
  parent has to remember what it triggered. Future enhancement could
  expose `workspace(status, include=['guide_hints'])` or similar.
- Wall-clock session vs. post-reconnect window is a real distinction
  (per W-2's amendment). Parent should brief based on
  post-reconnect-window state, not full session history.

**Threshold to promote:** R-9 + one more datapoint where a subagent
mis-predicts V2/session-state behavior. Currently 1/2.

**Status:** drafted into SKILL.md preemptively (claude-plugins:f842848, 2026-05-28). Added as a 6th bullet under Phase 1 — Scout, naming subagent dispatch's session-state-scout requirement and the recommended brief shape (`"I've triggered: [librarian, progressive-disclosure]"`). Ships at 1/2 datapoints because the F-6 critique came verbatim from a verification subagent's own self-assessment (W-4 Section E), which is unusually high signal for a single datapoint — the future subagent who'd benefit from this guidance is exactly the agent that named the gap. Revised promote-when: if R-9's pattern catches a similar miss in a future session → mark `validated`; if no further misses surface within 3 multi-session work streams that involve subagent dispatch, the proactive ship was correct.

---

## R-10 — Miss: buffered artifact body parsed for structured extraction without a completeness scout

**Verdict:** miss → proposal

**Observed:** 2026-05-29, metadata-filtering work stream
(`docs/trackers/metadata-filtering-session-log.md` F-4 + W-1).

**Pattern that failed:** Retrofitting `codescout-usage-frictions` to be
`entry_filter`-searchable required parsing the tracker's body into a structured
array. `artifact(get, full=true)` returned a 36 KB `@tool_*` buffer whose `body`
field was truncated at U-18 by the progressive-disclosure inline budget; the
parse silently produced 15 of 22 entries (U-19..U-25 dropped). No Phase-1 scout
verified that the parsed body was *complete* before it became the input to a
write (`artifact_augment`). The drift was caught only at post-augment
verification, by noticing the get response's `preview.headings` listed entries
beyond the parsed tail.

**Pattern proposal:** Add to Phase-1 Scout — *when a buffered tool output
(`@tool_*` / `@cmd_*`) is the input to a structured extraction or write, treat
its completeness as an unverified shape.* Reconcile the parsed item count against
an independent server-side view (`preview.headings` for artifacts, total/by_file
for search tools) before acting on it. Buffered bodies are silently clipped at
the inline budget; the truncation carries no in-band marker.

**Cost absorbed:** 1 incomplete catalog write (corrected before any consumer
queried it) + 1 re-read (line-range get) + 1 `merge=false` re-augment with a
widened schema enum. Recoverable, but had verification not cross-checked the
preview, a 7-of-22-entry index would have shipped with no error and no git diff.

**Promote-when:** A second instance of a buffered tool output truncating a
structured-extraction input. At 2 datapoints, fold into a Phase-1 Scout bullet
in SKILL.md ("buffered outputs are unverified shape for extraction/writes").

**Status:** proposal — single datapoint (F-4 + W-1, this session). Awaiting a
second occurrence before SKILL.md promotion.

---
## R-11 — Concept docs diverged from code on concurrency semantics

**Verdict:** hit → proposal

**Date:** 2026-05-30

**Scout:** Before running a multi-instance / multi-worktree concurrency experiment on
backend-kotlin, scouted `docs/manual/src/concepts/{cross-process-write-serialization,
kotlin-lsp-multiplexer}.md` against the actual code (`src/lsp/mux/mod.rs`,
`src/lsp/servers/mod.rs`). Two doc-vs-reality gaps surfaced *before* acting:

1. **"Isolated GRADLE_USER_HOME to prevent daemon contention between instances"**
   (`kotlin-lsp-multiplexer.md` § Gradle Isolation) reads as *per-instance* isolation.
   Code: `src/lsp/servers/mod.rs:63` hard-codes a single fixed
   `GRADLE_USER_HOME=/tmp/codescout-mux-gradle` shared by **every** kotlin JVM. The
   isolation is from the user's `~/.gradle`, **not** between worktrees/instances.
2. **Cross-worktree JVM multiplication is undocumented.** Neither doc states that the mux
   socket is keyed on workspace **path** (`src/lsp/mux/mod.rs:14,20`), so N worktrees of one
   repo spawn N JVMs against one shared, unguarded IntelliJ system-path. The mux docs imply
   "one JVM per project"; reality is "one per path."

**Counterfactual:** Without the scout, I'd have framed the experiment as "the mux dedups, so
worktrees are cheap" and mis-read the 6-JVM / shared-system-path result as a bug in my setup
rather than the designed (under-documented) behavior. The scout also corrected the user's
premise that subagents create *separate instances* (they share one server → a different
conflict regime entirely).

**Proposal:** When scouting any "isolation" / "per-X" claim in concept docs, grep the
constant the doc names (`GRADLE_USER_HOME`, `system-path`) and confirm the isolation key
matches the doc's stated granularity. Doc adjectives ("isolated", "per-instance") are
assertions to verify against the keying expression, not facts.

**Evidence (bug trackers):** `docs/issues/2026-05-30-shared-server-global-active-project-race.md`,
`docs/issues/2026-05-30-cross-worktree-kotlin-jvm-shared-system-path.md`.

**Promote-when:** A second scout catches a doc "isolation/per-X" adjective contradicted by a
shared constant. At 2 datapoints, promote to the skill as a Phase-1 rule:
"verify isolation-claim adjectives against the keying expression."
## R-12 — Plan's proposed data structure cited the symptom layer, not the structural layer

**Verdict:** hit → proposal

**Date:** 2026-05-30

**Scout:** Before implementing `docs/plans/2026-05-30-per-request-workspace-pinning.md`, scouted the plan's named seams against current code: `AgentInner` (`src/agent/mod.rs:82`), `ActiveProject` (`:135`), the four resolution accessors, and `Agent::activate` (`:330`). The plan's Design proposed a flat `HashMap<PathBuf, Arc<RwLock<ActiveProject>>>` registry. Scouting `src/workspace.rs:316` revealed the racing slot is actually `AgentInner.workspace: Option<Workspace>`, and `Workspace` is **already** a multi-project registry (`projects: Vec<Project>`, each `Dormant`/`Activated(Box<ActiveProject>)`, + `focused: Option<String>`) carrying an existing per-request resolver `Workspace::resolve_root` (`:373`, "explicit id > file hint > focused"). The correct registry unit is `Workspace`, one abstraction layer above the plan's `ActiveProject`.

**Counterfactual:** Without the scout, Phase 1 builds a flat `ActiveProject` HashMap that collides with the existing `Workspace` abstraction; the collision surfaces only after the structure is wired, forcing a full Phase-1 rewrite plus a wasted call-site migration against the wrong structure. Caught pre-implementation (F-1 in `concurrency-fix-session-log.md`); the correction is a plan revision — net *less* code, since `resolve_root` already exists.

**Proposal:** When a plan's Design names a *data structure* to introduce, scout the existing abstraction at that layer before Phase 0 — grep for the field the plan would add (`projects:`, `workspace:`) and read the struct that owns the racing slot. A plan written from a bug file inherits the bug file's *symptom-layer* framing (here: "single global active project"); the *structural layer* (a `Workspace` nesting N projects) may already implement half the fix. Verify the plan's granularity against the owning struct, not the symptom description.

**Evidence (session-log):** concurrency-fix F-1 (this session).

**Promote-when:** A second pre-implementation scout catches a plan proposing a data structure that duplicates an existing abstraction one layer up. At 2 datapoints, promote to the skill as a Phase-1 rule: "scout the struct that owns the racing slot before trusting a plan's proposed data structure."
## R-13 — Cross-repo doc↔plugin drift is invisible to intra-repo `audit_doc_refs`

**Verdict:** hit → proposal

**Observed:** 2026-05-30, integration-design session (Hermes/OpenClaw + codescout). A workflow mapping agent flagged that codescout's `CLAUDE.md` described companion `pre-tool-guard.sh` behavior that no longer matched the plugin. Scouted the authoritative source before fixing.

**Scout (reality):** Read `../claude-plugins/codescout-companion/hooks/hooks.json` + `pre-tool-guard.sh` headers via `grep` (source-file shell blocked; used codescout `grep` tool). Doc was stale three ways: (1) named hook `semantic-tool-router.sh`, which does not exist (real file: `pre-tool-guard.sh`); (2) matcher documented as `Grep|Glob|Read`, actually `Grep|Glob|Read|Bash|Edit|Write`; (3) the "Cross-repo work (companion ≥ 1.11.1)" block described a `cd`-passthrough escape that was removed when the hook was hardened 2026-05-21 (now all `Bash` → `run_command`, sibling git via `git -C /abs/path`). Plus 9 registered hooks the doc never mentioned.

**Counterfactual (hit value):** An agent trusting the stale `CLAUDE.md` would run `cd ../sibling && git …` expecting passthrough → now hard-`deny`ed → failed Bash call + confusion; or chase a nonexistent `semantic-tool-router.sh`. Scouting `hooks.json` (authoritative source) instead of patching only the one line the user named caught 3× the drift. Fixed in `commit 7187396a`.

**Cross-cutting lesson / proposal:** This drift is structurally INVISIBLE to `librarian(audit_doc_refs)` — that lint scans only the active project's own `docs/**`, so a codescout doc stale about a *sibling repo's* code (the companion lives in `../claude-plugins/`) can never be flagged. Recon caught it only because an agent diffed the two repos. Proposal: when scoping the autonomous ops daemon (integration Pattern 3), point `audit_doc_refs` at BOTH repos, or add a cross-repo doc-vs-source audit mode. Until then, cross-repo plugin↔doc drift has no automated gate — it needs an explicit recon pass.

**Relation:** Same family as R-11 (docs diverged from code on concurrency) but the novel axis is CROSS-REPO + the audit blind spot. R-11/R-12 compared codescout docs vs codescout code; this is codescout docs vs `claude-plugins` code.

**Promote-when:** A second cross-repo doc-drift datapoint (codescout doc stale about `claude-plugins`, or vice-versa) → promote a Phase-1 Scout bullet: "For docs describing a *sibling repo's* code (plugin hooks, cross-repo configs), scout the authoritative source in that repo — `audit_doc_refs` cannot see across repo boundaries."

**Status:** open — single datapoint; proposal awaiting a second cross-repo drift datapoint before SKILL.md promotion.

**Source:** `commit 7187396a` (companion-docs fix, this session); `CLAUDE.md` § "## Companion Plugin: codescout-companion".
## R-14 — Scout specialist-memory-sourced design claims against current code

**Verdict:** hit (confirmed — no drift, but the scout was load-bearing)

**Observed:** 2026-06-01, mid-brainstorm of the peer-delegation protocol. The summoned Architecture Snow Lion cited the project memory `architecture-snow-lion/outputguard-cross-cutting-law.md` (dated 2026-05-07) to assert codescout's `@ref` buffers are process-local — the basis for a hard design finding ("`tool.call` breaks `OutputGuard` across the peer boundary; requester A cannot read peer B's `@tool_X`"). Scouted the current code before letting the design rest on it.

**Scout (reality):** `src/tools/output_buffer.rs:42` — `OutputBuffer { inner: Mutex<BufferInner> }`; `BufferInner` holds `entries: HashMap`, `order: Vec` (LRU), `max_entries` (`new(20)`). Thread-safe **in-memory LRU**, held as `Arc<OutputBuffer>` in the tool context; the `workspace-state` guide classifies buffers as session-resident ("NOT cleared … remain readable"). No disk / shared backing (the `source_path` on `@file_*` entries is the file they *point at*, not a shared registry). `call_content` (`src/tools/core/types.rs:485`) is the dispatch+buffering entry point (test: `call_content_buffers_large_output`).

**Outcome:** MATCH — claim confirmed. The cross-process boundary problem is real; the design's "re-buffer on the requester" resolution stands on read code, not a 3-week-old memory.

**Cross-cutting lesson:** A summoned specialist citing a *dated* memory as the basis for a load-bearing design decision is a seam — exactly like a plan citing a struct field. Memories carry an `updated:` date precisely because they are snapshots. The Snow Lion's Operating Principle 2 ("cite the import, not the diagram") extends to "not the memory either." Scout before the design depends on it; confirm-or-dissolve is itself the value. Had buffers moved to disk since 2026-05-07, the design would have invented a non-problem and bolted on an unnecessary proxy mechanism.

**Promote-when:** a second instance where a specialist/CLAUDE.md memory citation, once scouted, turns out *stale* (drift) → promote a Phase-1 Scout bullet: "Treat specialist/CLAUDE.md memory citations as snapshots; verify the cited symbol/contract against current code before a design or edit depends on it."

**Status:** open — single datapoint; confirmed-match this time.

**Source:** `src/tools/output_buffer.rs:42`, `src/tools/core/types.rs:485`; this session's peer-delegation brainstorm.
## R-15 — Scout external-tool on-disk state against bug-doc claims before a fix depends on addressing it

**Verdict:** hit (caught doc-vs-filesystem gap pre-implementation)

**Observed:** 2026-06-03, systematic-debug pass on the kotlin-lsp unbounded-disk bug (`docs/issues/2026-06-01-kotlin-lsp-analyzer-index-unbounded-disk.md`). About to evaluate fix candidate #2 — "on idle-timeout, remove *that workspace's* analyzer dir" — which presumes codescout can address the analyzer dir from its own `ws_hash`.

**Scout (reality):** Listed the live `--system-path` dirs vs `~/.config/JetBrains/analyzer/workspaces/*`. codescout's `ws_hash` (`src/socket_discovery.rs:10`, `DefaultHasher` → `{:016x}`) is **16 hex chars**; the analyzer dirs are **32 hex chars** (128-bit, IntelliJ path-hash). None of the 3 live system-path hashes (`c85ec91bdbfd1aee`, `26a9e85d58931839`, `7e868829c00fa9b2`) appear among the 8 analyzer dirs.

**Outcome:** GAP — the bug file's Evidence ("`<hash>` matches codescout's `workspace_hash` granularity") conflated *granularity* with *derivable key*. Fix #2 is not viable without replicating IntelliJ's hash (fragile, version-coupled). Re-ranked toward the env-redirect fix (codescout owns the base path) — captured as kotlin-lsp-disk F-1; corrected the bug file.

**Cross-cutting lesson:** Recon's "read the actual response shape, not docs" extends to the *filesystem state of external tools*, not just code symbols and API responses. A bug doc's claim about *where another process writes* and *how it keys those paths* is a seam — verify it against the live tree before a fix design rests on addressing those paths. Cheap (`ls`/`du`), and it dissolved a doomed fix direction before any code.

**Promote-when:** a second instance where a fix design assumed an external tool's on-disk path was addressable from our own key/hash and the live tree disproved it → promote a Phase-1 Scout bullet: "When a fix must locate files another process writes, list the live tree and confirm the key is one we control or can derive — not merely the same granularity."

**Status:** open — single datapoint; gap caught + bug doc corrected this session.

**Source:** `src/socket_discovery.rs:10`; `~/.config/JetBrains/analyzer/workspaces/` live listing; bug `docs/issues/2026-06-01-kotlin-lsp-analyzer-index-unbounded-disk.md`.

---
## R-16 — Pre-dispatch scout of the plan's OWN splice code caught a double-newline bug before dispatch

**Verdict:** hit (caught a correctness bug in plan-authored code at the seam, pre-dispatch)

**Observed:** 2026-06-04, subagent-driven execution of the edit_file whitespace-normalized fallback plan, about to dispatch the Task 4 (integration) implementer.

**Scout (reality):** Re-read the plan's own `match_count==0` apply code at the byte level. `find_normalized_windows` sets `end_byte` to EXCLUDE the matched block's trailing newline (so `content[end_byte..]` re-supplies it), but `reindent_block` re-emits a trailing newline when `new_string` ends in `\n` — so an `old_string`/`new_string` ending in `\n` would splice a DOUBLE newline (spurious blank line). The common no-trailing-newline case was correct, hiding it.

**Outcome:** Fixed in the dispatch before any subagent ran (`let replacement_src = new_string.strip_suffix('\n').unwrap_or(new_string);` before reindent) + a dedicated regression test. Drift never reached the implementer — textbook recon-before-dispatch.

**Cross-cutting lesson:** "Scout the seam before you act" applies to your OWN plan code, not just existing substrate. The writing-plans phase can author a subtly wrong splice/offset whose error is invisible in the common case; a controller re-read at the byte level (where exactly does the replacement land vs where the matched span ends) catches it for one read. Byte-offset / newline boundaries are a seam.

**Related session observations (same work stream):**
- MISS → promoted: plan test fixtures used `old_string`s that were literal substrings of the file, silently routing "normalized-path" tests through the EXACT path (3 instances); caught by per-task + holistic review, not the plan. Promoted to CLAUDE.md Testing Patterns (substring-overlap rule).
- Pre-commit diff-scout: `git diff --stat` before committing revealed Task 6's whole-workspace `cargo fmt` had churned 9 unrelated rustfmt-drifted files; verified pure-formatting via the raw diff (incl. correcting my own false "logic change" alarm from a corrupted grep) and excluded them from the feature. Lesson: in a drifted/shared repo, scope `cargo fmt -- <files>` or use `cargo fmt --check`, and `git diff --stat` before any `git add`.

**Promote-when:** a second instance where a controller re-read of plan-authored offset/splice/boundary code catches a bug pre-dispatch → promote a writing-plans/recon bullet: "Before dispatching an integration task, re-read any plan code that computes byte offsets, ranges, or splice boundaries — author error there is invisible in the common case."

**Status:** open — single datapoint for the splice bug; the substring-overlap sub-pattern reached promotion (3 datapoints → CLAUDE.md).

**Source:** `src/tools/edit_file/mod.rs` `perform_edit` `match_count==0` arm; plan `docs/superpowers/plans/2026-06-04-edit-file-whitespace-normalized-fallback.md`.

---
## R-17 — Spot-check sibling callers of a just-fixed shared helper before closing the bug class

**Verdict:** hit (recon caught a 3× blast radius; live repro + regression tests confirmed)

**Observed:** 2026-06-05, after fixing an `edit_code insert-after` parent-clamp off-by-one in
`do_insert` (last child of a dedent-delimited Python class). About to declare the bug class closed;
user asked to spot-check the flagged replace-path lead.

**Scout (reality):** `references(clamp_range_to_parent)` surfaced two more production callers —
`do_remove` (`edit_code.rs:454`) and `do_replace` (`:515`) — both converting `parent.end_line` into
an exclusive clamp bound with the identical bare-`end_line` off-by-one. Reproduced live against the
shipped binary before fixing: `edit_code replace` on the last method reported `replaced_lines: 5-9`
(excluding the trailing-`assert` line), leaving it orphaned after the new body; `remove` left it
behind. The AST-extractor and `do_insert`-specific reasoning were the wrong layer — the seam was the
**shared boundary helper's call contract**.

**Outcome:** Fixed all three sites (`+ 1`), added `replace_`/`remove_last_python_method_*` regression
tests (both verified fails-without/passes-with by reverting the `+1`), all 54 `symbol_lsp` tests green
including the `bug034_guard_*` over-extension guards. Captured as W-9 in `bug-fix-session-log.md`.

**Cross-cutting lesson:** when a fix corrects how ONE caller derives an argument to a shared
range/clamp/boundary helper, the same derivation error almost certainly lives at the other callers.
`references(helper)` + reproduce each call site's input shape BEFORE closing the bug class. A
single-call-site fix to a multi-caller helper-usage bug ships a partial fix that re-surfaces at full
debugging cost on the untouched paths.

**Also this session (re-confirms R-16's fmt sub-lesson):** whole-workspace `cargo fmt` churned files a
concurrent session left rustfmt-drifted (markdown `.rs` + a `server.rs` reflow). Caught via
`git diff --stat`; will commit file-scoped (`edit_code.rs` + `tests/symbol_lsp.rs`) rather than
`git add -A`. Second datapoint for "scope `cargo fmt -- <files>` / `--check` in a shared repo."

**Promote-when:** a second instance where `references()`-ing the callers of a just-fixed shared helper
catches an under-scoped fix → promote to CLAUDE.md: "When fixing how a caller uses a shared
boundary/clamp/offset helper, scout every other caller of that helper and reproduce each before
closing the bug class."

**Status:** open — single datapoint for the sibling-caller pattern.

**Source:** `src/tools/symbol/edit_code.rs` (`do_insert`/`do_remove`/`do_replace`);
`docs/issues/2026-06-05-edit-code-insert-after-last-python-method.md`; W-9 in
`docs/trackers/bug-fix-session-log.md`.

---
## Template for new entries

<!-- Insert new R-N entries above this line via:
     edit_markdown(action="insert_before",
                   heading="## Template for new entries",
                   content="## R-N — title\n**Verdict:** ...\n...")
     Also update the Index table row at the top. -->
