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
## Template for new entries

<!-- Insert new R-N entries above this line via:
     edit_markdown(action="insert_before",
                   heading="## Template for new entries",
                   content="## R-N — title\n**Verdict:** ...\n...")
     Also update the Index table row at the top. -->
