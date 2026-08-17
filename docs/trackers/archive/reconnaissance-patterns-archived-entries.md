---
kind: tracker
status: archived
title: Reconnaissance patterns — archived entries (superseded)
owners: []
tags:
  - reconnaissance
  - archive
  - skill-meta
---

# Reconnaissance patterns — archived entries

Entries moved out of `docs/trackers/reconnaissance-patterns.md` on 2026-08-16
under the archive-cadence policy ratified the same day
(`docs/trackers/archive-cadence-policy.md` § Ratified).

**These are superseded, not wrong.** Each states a law that survives elsewhere in
stronger or more general form. They are kept because the evidence in them is the
record of how the surviving law was earned, and because entries cite each other:
a citation of an archived id must still resolve to something.

| archived | survivor | why the survivor is enough |
|---|---|---|
| `R-2` | **R-4 + R-34** | R-2 patches grep vocabulary one pattern at a time; compiler-as-scout (R-34) makes the enumeration exhaustive by construction, and R-4 keeps the grep-undercount evidence. |
| `R-5` | **R-34** | R-34 states compiler-as-scout in a strictly more general form than R-5's call-site-only framing. |
| `R-7` | **R-1** | R-1's own Status line records R-7 as its closing datapoint; the shipped SKILL.md bullet names both. |
| `R-20` | **R-49 + R-3** | R-20 is an instance of R-49 (your own bug file is a hypothesis) told from the search side; R-3 holds the grep-scope half. |
| `R-27` | **R-26** | Identical law, more compactly stated. R-27 differs only in whether the false claim came from a grep line or a subagent's prose. |
| `R-31` | **R-49 + R-3** | Second instance of the same law as R-20; both self-describe as validating R-3. |
| `R-37` | **R-28** | The file itself labels R-37 a recurrence of R-28. |
| `R-38` | **R-55** | R-55 adds why the rule misfires (task self-classification) and a concrete retrigger. |
| `R-40` | **R-69** | R-69 is the general form: any document naming code it does not contain. |
| `R-43` | **R-26 + R-50** | The positional-binding mechanism is an instance of 'a grep hit is not a verified fact', not a separate law. |
| `R-46` | **R-63** | R-63 states the same rule with a stronger failure mode. |
| `R-56b` | **R-47** | R-47 covers caller enumeration; only R-56b's 'probe an unreported caller' clause is unique and is folded into the note below. |
| `R-59b` | **R-62** | R-62 is sharper: a no-op committed as a fix, then cited as done. |

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

---

## R-5 — Add "compiler as scout" as a Phase-1 tool alongside grep

**Verdict:** proposal

**Source:** R-4 + W-2 in
`docs/trackers/archive/mcp-prompt-redesign-session-log.md`.

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

---

## R-20 — A bug-file's hand-cited fix-plan line list is not the blast radius

**Verdict:** hit (validates R-3)

**Observed:** 2026-06-09, scouting the only open bug
(`issues/2026-06-09-onboarding-prompt-uses-project-not-project-id.md`)
before editing `src/prompts/builders.rs`.

**Pattern that worked:** R-3's "grep the workspace root, not the file being
changed" applied verbatim — here against a *bug file's* Fix section, not a
formal plan doc. The bug file cited tests at `run_command/tests.rs:286-287`;
a workspace-root `grep project=` surfaced a third assertion at `:257`
(`build_per_project_prompt_contains_project_context`) that pins the OLD
string and flips green→red once the builder emits `project_id=`. The same
grep proved the cited fixture (`tests/fixtures/prompt_surfaces/onboarding_prompt.md`)
has 0 matches — the builder prompts are ephemeral `.codescout/tmp/` files,
not part of that snapshot surface, so no fixture/`ONBOARDING_VERSION` work.

**New wrinkle vs R-3:** the stale line list lived in a `docs/issues/*` bug
file's Fix/Resume prose, which reads as authoritative ("apply edits at lines
X,Y,Z"). A subagent handed the bug file would treat its line list as the
blast radius. Bug-file Fix sections deserve the same "line list is a starting
point, not the contract" skepticism as plan docs.

**Cost avoided:** ≥1 red `cargo test` cycle (line 257) + a fixture hunt on a
0-match surface + a possibly-spurious `ONBOARDING_VERSION` bump.

**Promote-when:** R-3 is already promoted; this is a confirming datapoint that
extends its scope to bug-file Fix sections. If a 3rd such case lands, widen the
SKILL.md Phase-1 bullet to name bug-file line lists explicitly.

**Status:** hit; logged. Extends promoted R-3 to bug-file fix plans.

**Fix idea / Pointer:** bug-fix-session-log F-15 + W-10; this session.

---

---

## R-27 — A subagent's control-flow *mechanism* claim is a hypothesis; read the fn body before a fix depends on it

(hit) `/reconnaissance` invoked mid-fix for `get_guide` returning a `@tool_*` handle instead of the full guide. An `Explore` subagent dispatched to map the buffer-routing mechanism returned a confident, internally-contradictory report: it claimed `OutputForm::Text` *"forces inline output always"*, listed `symbols`/`read_markdown`/`memory`/`call_graph` as proof, and recommended **"Option A: override `output_form()` to Text"** (marked *Recommended*) — yet in the same report also wrote *"the buffering check still applies (line 554)"*. Reading `Tool::call_content` (`src/tools/core/types.rs:485-615`) resolved the contradiction in reality's favour: the overflow gate `if exceeds_inline_limit(&json)` runs **unconditionally** at ~`:554`, *before* the `OutputForm` match — which only selects `format_compact` vs pretty-JSON inside the small-output `else` branch. So `Text` never bypasses buffering, the listed tools ARE buffered when large, and Option A would have left the 14 KB guide buffered exactly as filed. The real fix is orthogonal to `output_form`: a `force_inline()` trait hook (default `false`) gating the overflow branch (`exceeds_inline_limit(&json) && !self.force_inline()`), overridden `true` on `GetGuide`. Lesson: a subagent describing *how* code branches is doing Phase-1 location dressed as Phase-2 confirmation — the internal contradiction ("forces inline always" vs "check still applies") was the tell. Treat a delegated mechanism claim exactly like a grep hit (R-26): it locates the relevant function; it does not confirm the branch *order*. One `symbols(<fn>, include_body=true)` settles it. Kin R-26 (grep ≠ mechanism), R-19 (assert checkable facts only post-read), R-9 (session-state recon at the dispatch boundary — this is its consumption-side mirror).

**Evidence:** bug-fix F-19 + W-14; verified read-only against `src/tools/core/types.rs:485-615` (`Tool/call_content`); fix + regression test in `src/tools/guide.rs` (commit pending).

---

## R-31 — A bug-file's "never parsed" claim, evidenced by a file-scoped grep, missed a parser one call-hop away

**Verdict:** hit (validates R-3 + R-26)

**Observed:** 2026-06-14, pre-fix recon for the read_file `offset`/`limit` bug (`issues/2026-06-14-read-file-offset-limit-silently-ignored-on-buffers.md`).

**Pattern:** When a bug file asserts a parameter is "never parsed anywhere" and backs it with a grep scoped to the file(s) being modified, that audit cannot see a parser reached one call-hop away in another module. Follow the param into the helpers the modified function *calls*, not just the file's own text.

**What happened:** The bug's E-1/E-2 grepped `read_file.rs` + `output_buffer.rs` and concluded `offset`/`limit` "are never parsed." But `read_full_file` calls `OutputGuard::from_input(input)` (`src/tools/output.rs:89`), which DOES parse both (`optional_u64_param(input, "offset")`, `…"limit"`, L96-97). The scout caught it by reading the body of the *called* helper, not by re-grepping — R-26 ("read the body, don't trust the line-match") applied to a callee rather than to the symbol under edit.

**Counterfactual:** Taking "never parsed" at face value, Fix A option (a) (add `offset`/`limit` to `read_file`'s schema + map to line semantics in the buffer path) would have shipped a schema that silently collides with `OutputGuard`'s page-offset/page-size semantics on the real-file path — two meanings for one param name, and a schema that advertises an `offset` the real-file path never honors for navigation. The scout redirected the fix to option (b) (path-local error) before any code changed.

**Proposal:** none new — reinforces R-3 (grep workspace root, not the file) and R-26 (read callee bodies before narrating "confirmed"). Datapoint toward making "follow the param into callees" an explicit Phase-1 step whenever a claim is about whether a param is *read* (vs. *used*).

**Evidence:** bug-fix F-22; `docs/issues/archive/2026-06-14-read-file-offset-limit-silently-ignored-on-buffers.md`; `src/tools/output.rs:89-112`; `src/tools/read_file.rs:555-664` (`read_full_file` builds the guard).

---

## R-37 — Miss (recurrence of R-28): get_guide topic added without enumerating the description-budget + topic-count gates

**Observed:** 2026-07-03, adding the `untrusted-content` get_guide topic (data-vs-directive rule, audit-log A-5).

**Scout done:** grepped `GUIDE_TOPICS` references (2 files — correct seam for registration) and read the guide.rs test *names*, but did not read their *assertions* before editing. Two gates fired at `cargo test --lib`: `server::tests::tool_descriptions_stay_under_budget` (description 309/300 chars after appending the topic name) and `tools::guide::tests::get_guide_lists_topics_with_no_arg` (hardcoded `names.len() == 8`).

**Cost:** one failed gate cycle (~2 min), pre-commit; zero shipped harm — the full `--lib` run caught what R-28's narrow filter once let ship.

**Fixes:** trimmed the description (" + summaries" dropped, 297/300) and made the count assertion derive from `GUIDE_TOPICS.len()` so the next topic can't re-trip it (gate de-rot, not just gate compliance).

**Generalization (validates R-28/R-1/R-7):** "enumerate the full gate set of a prompt surface" includes the *tool-description* surface, not just source.md slices — any string a tool exposes through the MCP schema has a budget test in `server::tests`. When adding to an enumerated surface (topics, tools, laws), also grep the tests for hardcoded counts (`len() == N`) and convert them to derive from the canonical const while you're there.

**Verdict:** miss — downstream gate absorbed it. Second datapoint for R-28's promote-when: distill "before editing any tool description or enumerated prompt surface, run the surface's budget/count gates first (grep `server::tests` for the tool name)" into codescout memory `reconnaissance`.

**Evidence:** gate2 output (309/300; `left: 9, right: 8`); fixes in `src/tools/guide.rs` (description + derived count); gate3 green (2868 passed).

---

## R-38 — Miss: re-derived a finding a concurrent session had already MEASURED; the existing audit-log wasn't scouted first

**Observed:** 2026-07-03, hardening the tracker-hygiene D1 eval (#22). Independently
"discovered" the precision gap (could the skill flag `README`/`CONVENTIONS` meta-files as
D1?) and built a tier-3 precision assertion — then, on finally reading the claude-plugins
Hamsa audit-log, found a concurrent session had already audited AND measured exactly that
concern (n=5 across sonnet + haiku; outcome: the false-positive doesn't reproduce).

**Miss:** the "seam" for a *derive-a-finding* or *record-a-finding* task includes the
existing KNOWLEDGE state — the audit-log / tracker entries on the same subject — not just
the code + eval. I scouted the code and the scenario but not the ledger, so I re-derived a
settled measurement. Not wasted (my net-new contribution was a standing regression gate
over their one-time measurement), but the duplication was avoidable, and I flagged it in
the plugins follow-up.

**Verdict:** miss (caught downstream by reading the ledger).

**Proposal:** before deriving OR recording a new finding — especially in a
concurrently-edited shared ledger — scout the existing audit-log / tracker entries on that
subject first (`artifact(action="find", semantic=...)` / read the ledger section). Treat
"has this already been measured / recorded?" as a seam to scout, exactly like a struct's
fields. Promote-when: a second re-derivation-of-prior-work miss → codescout memory
`reconnaissance`. Kin: R-19 (scout home internals before cross-project claims).

---

## R-40 — usage.db error counts are commit-mixed AND time-spanning; verify a candidate against current code before fixing

**Observed:** 2026-07-10, mining usage.db (72 DBs) for fixable input-shape frictions after shipping the param-alias unification.

**Scout done:** ranked candidate frictions by error count, then — before writing any fix — read the current code for each. The `read_file` `json_path` quoted-key friction (`$["1.5"]`, `$["2.1.3"]`) had ~200 events, the LARGEST non-alias cluster. But `file_summary.rs::split_on_unbracketed_dot` (comment cites `Bug 2026-07-01-...dotted-object-keys-unreachable`) plus `parse_bracket`'s `strip_matching_quotes` branch already handle it — the friction was closed 2026-07-01. The telemetry spanned months and mixed pre- and post-fix commits.

**Verdict:** hit — a count-only ranking would have sent me to "fix" an already-fixed bug. This is R-29's phantom one axis over: R-29 is cross-*project* mixing, R-40 is cross-*time* staleness — same commit-SHA-keyed DB, same root cause. The real target (`artifact` filter inversion, 22 events) was confirmed by a *live* re-repro; json_path was a ghost.

---

## R-43 — Miss: `#[cfg]` gating claimed from a grep hit; attributes bind positionally and grep hides the gap

**Verdict:** miss (downstream gate — the compiler — caught it; narrows R-19, backstopped by R-5)

**Observed:** 2026-07-25, dependency leanness review (`git2` question → full manifest audit). No scout was run before the claim was stated.

**Pattern:** An attribute-binding claim — "this module is `#[cfg]`-gated", "this field is `#[serde(skip)]`", "this test is `#[tokio::test]`" — can never be made from a grep hit. `grep` prints matching lines with their line numbers but **elides the gap between them**; an attribute binds to the *next item*, which may be several lines away and is usually not itself a match. Read the region (`read_file(start_line, end_line)`) before asserting the binding.

**What happened:** grep over `src/retrieval/mod.rs` for `^(mod|pub mod) (embedder|reranker)|cfg\(feature` returned `L1: #[cfg(feature = "server-stack")]`, `L7: pub mod embedder;`, `L12: #[cfg(feature = "server-stack")]`, `L14: pub mod reranker;`. Read as "both modules are gated" and stated to the user as fact. The file is an alphabetized `pub mod` list: L1's cfg binds to L2 `pub mod artifact;`, L12's to L13 `pub mod qdrant;`. `embedder` and `reranker` are **ungated**. Filtering for two patterns in one grep made the interleaving look like pairing.

**Counterfactual:** The review's headline recommendation would have shipped as "make `reqwest` optional — one-line manifest change, saves 48 crates." That version does not compile: `embedder.rs` mixes reqwest-dependent types (`EmbedderHttp`, `HttpDenseEmbedder`) with the reqwest-free `EmbedOutput` / `SparseVector` / `BatchEmbedder` / `DenseEmbedder` surface that six other modules import, so the real change is a module split. It was caught only because the measurement loop happened to include `cargo check --no-default-features`, which returned 16× `E0433 unresolved module reqwest` in exactly those two files — i.e. R-5 ("compiler as scout") is what saved it, not the grep.

**Proposal:** add to SKILL.md Phase 1 — *"Attribute-binding claims can never be made from a grep hit: grep elides the gap between matching lines, and an attribute binds to the next item. Read the region."* This narrows R-19 (assert checkable facts only after reading this session) to a specific high-frequency substrate class, and pairs with R-5 as the backstop when the read is skipped.

**Evidence:** `src/retrieval/mod.rs:1-17`; `cargo check --no-default-features` → 16 errors confined to `src/retrieval/embedder.rs` + `src/retrieval/reranker.rs`; work-stream narrative in `docs/trackers/dependency-review-session-log.md` F-1 / W-1; kin R-19 / R-5 / R-3.

---

## R-46 — Hit: a target's own test module encodes intent the implementation cannot reveal; read it before choosing between candidate fixes

**Verdict:** hit.

Implementing the memory section-filter fix, the bug file (written earlier the same
session) offered "match any heading level" as its cheapest option and asserted it
"cannot break `language-patterns.md`". Reading `filter_sections`'s **test module**
killed it in one step: `filter_sections_nested_h4_included_in_body` exists to assert
that `####` is *body* inside its `###` section, so promoting every level to a boundary
breaks a deliberate behaviour — and `language-patterns.md`, the only memory with `###`
+ `####`, is precisely the file it would have damaged. The claim was inverted.

The generalisable point: reading `filter_sections` itself shows *what* it does
(`strip_prefix("### ")`) but not *which parts are load-bearing*. Only the tests
distinguish incidental behaviour from contracted behaviour. A fix chosen from the
implementation alone is a fix chosen without knowing what may not change.

Second finding from the same scout, in the data rather than the tests: counting heading
levels across all 21 memories showed 19 carry exactly one H1 as their title. Any
"shallowest level wins" variant that includes H1 therefore makes the title the sole
section and nests everything inside it — filtering would report a match and return the
whole document. Silent, and worse than the bug being fixed. The corpus census, not the
code, is what surfaced that.

Relation to kin entries: R-19 says don't assert a checkable fact about a symbol without
reading it this session. R-46 extends the target from the symbol to its **tests** — and
specifically for the decision "which of N candidate fixes", where the reading needed is
not of the code being changed but of the assertions constraining it. Also kin to R-44
(enumerate the consumer set, not the declaration site): tests are a consumer set that
caller-enumeration does not surface.

**Evidence:** `bug-fix-session-log.md` F-34 (the inverted claim, with severity
rationale) and W-26 (the fixture-vs-implementation diagnosis from the same fix).
Shipped `d668927e`; 19/19 in `memory::filter`, full gate 3439 passed / 0 failed. Bug:
`docs/issues/archive/2026-07-28-memory-sections-filter-matches-h3-only.md`.

**Promote-when:** a second case where a candidate fix is rejected by a pre-existing
test that reading the implementation would not have surfaced. Craft-shaped — true of
any language with a test suite — so the destination is `SKILL.md` Phase 1 as a bullet:
*"choosing between candidate fixes: read the target's test module, not just the target
— tests are where 'must not change' lives."*

---

## R-56b — Hit: a shared constructor with a side effect makes every caller a distinct seam

**Verdict:** hit

2026-08-08, tracing why an unvalidated `project_id` produced directory litter. The reported path was `memory(write)`. `references(from_dir)` returned five call sites — `write`, `read`, two list paths, `delete` — all constructing `MemoryStore::from_dir`, and that constructor calls `create_dir_all` on the path it is handed.

Probing the *non*-reported caller (`read`) surfaced a second, worse symptom the report never mentioned: `available_topics: []` plus the hint "no memory topics exist yet" for a project that does not exist — an absent project reported as merely empty. It also explained an asymmetry two hosts had made look like a configuration difference: `write` leaves a file (visible `??`), `read` leaves an empty directory (invisible), so the same defect presents as 8 entries on one machine and 2 silent ones on another.

**Rule:** when a side effect lives in a *constructor* rather than in the operation, the seam is not the reported call path — it is every caller of the constructor. `references()` the constructor, then probe at least one caller that the report did not name. Reading the constructor is not enough; the interesting variation is in what each caller does *after* it (write a file vs. list an empty dir), and that is what determines the observable symptom.

**Promote-when:** a third instance (R-17 and R-47 are the first two, both via delegates rather than constructors). At three, the SKILL.md seam-class list earns an entry: *a shared construction with a side effect in it — enumerate callers, probe an unreported one*.

---

## R-59b — Miss ×3: implementing from a bug file is a seam, and its Root cause is someone else's unverified reading

**Observed:** 2026-08-08. Three bug files worked back to back
(`63279f39570cd44a`, `9f823aabb84378a0`, `0fad8145011692a9`). All three Root cause
sections were wrong in a way that changed the fix. Full table in F-28,
`docs/trackers/release-promotion-session-log.md`.

**Why recon did not fire.** The skill's *When to Use* covers "before editing code whose
shape you have not verified" — which this was, every time. It did not read as a seam
because a bug file **presents as prior reconnaissance**: it has a Root cause section, a
mechanism in mechanism-language, file paths, sometimes line numbers. It looks like the
scout already happened. It usually did not: a bug file is written at the moment of
discovery, from the response payload and the symptom, by someone who has just lost work.
That is the right trade at capture time and it makes the artifact's confident register
misleading later.

The sharpest instance: the rename bug file said, in its own text, *"inferred from the
response payload — the rename implementation was not read."* It was honest, it was
prominent, and reading it still did not prevent planning a fix around the inference — the
surrounding detail was specific enough to feel scouted.

**Proposal.** Add to *When to Use*: **"Before implementing a fix from a bug file, plan, or
spec someone else wrote."** Its Root cause is a hypothesis unless it cites a command and a
date. Read the function the fix will change, then compare — the comparison is Phase 2, and
in all three cases it produced a different fix, twice a safer one:

- proposed "rename the union" → unsafe: `kind` classifies the FILE, so it rewrites comments
- proposed "refuse and require opt-in" → unavailable: writes are already committed
- assumed "the check ran and declined" → the check did not exist on that path

**Also: a grep count finds candidates, it does not decide.** `grep -c '^kind:'` returned 2
for the corrupted file **and** 2 for a healthy one whose second match sat inside a fenced
example. Whatever the count says, read the block.

**Verdict:** miss — three in one session, cited as F-28. Proposal pending; promote-when is
one more instance of a bug-file Root cause being falsified by reading, at which point it
is four and belongs in `SKILL.md` rather than here.

---


## R-30 — When a structural feature appears to *block* a tool, test the tool's documented alternate

**Verdict:** miss → proposal · **Observed:** 2026-06-13

**Pattern.** When a structural feature appears to *block* a tool, test the tool's documented alternate addressing form before refactoring code around it — read `find_unique_symbol_by_name_path`/`symbol_name_matches` but never tried the qualified `impl Trait for Type/method` form the not-found hint itself *documents*, so concluded `edit_code` was blocked and relocated 11 trait impls for a non-block. Reading the resolver ≠ testing the escape hatch.

**Evidence:** dzo-legibility F-9; ADR docs/adrs/2026-06-13-drop-name-collision-defect.md; kin R-26/R-27 (read ≠ verified)

**Status:** archived 2026-08-17 — compacted from the index row of `docs/trackers/reconnaissance-patterns.md`. No body section was ever written for this entry; this section exists so that citations of R-30 resolve (HY-9 / proposed D12).

## R-32 — An off-the-cuff root-cause unification across a bug cluster is a hypothesis, not a finding —

**Verdict:** hit · **Observed:** 2026-06-14

**Pattern.** An off-the-cuff root-cause unification across a bug cluster is a hypothesis, not a finding — read each bug's implicated code before presenting "they share one root cause / one fix" (3 open "catalog/path" bugs by title → 3 distinct files/mechanisms; only 2 shared a narrower root: global-abspath catalog reasoned-about-as-per-workspace)

**Evidence:** bug-fix F-21; issues 2026-06-13 catalog trio; kin R-12/R-19/R-27

**Status:** archived 2026-08-17 — compacted from the index row of `docs/trackers/reconnaissance-patterns.md`. No body section was ever written for this entry; this section exists so that citations of R-32 resolve (HY-9 / proposed D12).

## R-36 — Struct/config-field removal blast radius includes serde data fixtures, not just source — scope

**Verdict:** hit (validates R-3 / R-20) · **Observed:** 2026-06-28

**Pattern.** Struct/config-field removal blast radius includes serde **data fixtures**, not just source — scope the verification drift-grep to `tests/` (incl. `tests/fixtures/**/*.toml`), not just `src/`. `SecuritySection` has no `#[serde(deny_unknown_fields)]`, so stale keys (`shell_enabled`, `shell_output_limit_bytes`) in two fixture `project.toml`s were silently dropped — `cargo test` stayed green; only the `src/ tests/` drift-grep surfaced them. Same serde-silent-ignore property that makes the user migration a footgun is what hid the fixture drift from compiler + tests. Promote-when (2 datapoints) → codescout memory `reconnaissance` (project-shaped: Rust+serde+TOML).

**Evidence:** issues/2026-06-28-vestigial-shell-output-limit-bytes; tests/fixtures/{rust,kotlin}-library/.codescout/project.toml:18-19; kin R-3/R-20

**Status:** archived 2026-08-17 — compacted from the index row of `docs/trackers/reconnaissance-patterns.md`. No body section was ever written for this entry; this section exists so that citations of R-36 resolve (HY-9 / proposed D12).

## R-55 — A file's open-bug ledger is part of its shape — query it by FILE IDENTITY, not by task category

**Verdict:** miss → proposal · **Observed:** 2026-08-06

**Pattern.** **A file's open-bug ledger is part of its shape — query it by FILE IDENTITY, not by task category.** The bootstrap guide already carries the rule ("Bug or regression work: `artifact(find, kind=bug, status=open)` … don't re-file a filed bug as new") and it was auto-injected into the session's FIRST tool response. It still missed, because its trigger is the task's self-classification: a session framed as *docs + merge prep* answers "am I doing bug work?" with no, right up until it edits a file that has an open high-severity bug against it. Retrigger on the edit instead: before the first change to any file, `artifact(action="find", kind="bug")` constrained to that path or subsystem. What the ledger holds is not reachable any other way — the code does not show it and neither does git log: **chosen-but-unimplemented fix candidates, preconditions on landing, and cross-change sequencing decisions.**

**Evidence:** release-promotion F-2; `docs/issues/2026-07-27-ast-chunker-no-minimum-chunk-size.md` (open, high) named this exact fix as candidate 1 verbatim — reimplemented at `ca442498` with its stated precondition ("validated against the retrieval benchmark … must be measured, not assumed") unmet and its recorded ordering (throughput work first, being vector-identical) jumped; separately re-filed `2026-07-28-audit-doc-refs-json-pointer-false-positive` as new. Both surfaced ~60 tool calls late, by an unrelated verify-open pass, not by recon. Kin R-49 (re-scout your own bug file before implementing it), R-19

**Status:** archived 2026-08-17 — compacted from the index row of `docs/trackers/reconnaissance-patterns.md`. No body section was ever written for this entry; this section exists so that citations of R-55 resolve (HY-9 / proposed D12).

## R-56 — The record of a change is not the change — verify at the artifact, never from the report of it

**Verdict:** miss → proposal · **Observed:** 2026-08-06

**Pattern.** **The record of a change is not the change — verify at the artifact, never from the report of it.** Two shapes, same session. (a) A written prescription goes stale against later edits of *its own file*: three bug files' `## Fix` sections were materially wrong — one closed with "that figure needs correcting regardless" while a later **CORRECTED** note at the top of the same file retracted the comparison the claim rested on; one called a duplication "harmless in size" when a pre-fix test showed it duplicated to EOF; one ranked "exit on stdio EOF" as the cheapest fix when reading the code showed it was already implemented. (b) A *session summary* asserted tracker work that never landed: it recorded WIN-28/WIN-29 rows added to `windows-platform-support.md` and an R-55 entry written — the WIN rows were absent entirely, and R-55 existed as an index row with no body. Proposal: before citing any prior entry as done, grep the target artifact for the ID. Before implementing a bug file's Fix, read the code it names and check for a later dated note in the same file that contradicts it.

**Evidence:** release-promotion F-4; ast-chunker trailing-gap dump (chunk `"}\n\nfn tail_marker…"` lines 12-16 proved EOF-wide duplication); `ResilientStdin` absorbs only `WouldBlock` so 0-byte EOF already propagates; `grep 'WIN-2[89]' docs/trackers/windows-platform-support.md` → 0 matches. **Already at 2 datapoints — promote-when has fired:** `bug-fix-session-log` F-27 records the identical shape from an earlier session ("Prior-session summary claimed two bug files were logged; neither exists on disk or in git history"), and F-24 a near-miss ("documented 'implemented on experiments' but is an uncommitted working-tree change on no branch"). Three independent instances of *the record asserting work that did not land*. Promote to CLAUDE.md as: **before citing prior work as done — an entry, a row, a commit, a file — grep the artifact for it.** Kin R-19 (read the symbol before asserting a checkable fact), R-49, R-55

**Status:** archived 2026-08-17 — compacted from the index row of `docs/trackers/reconnaissance-patterns.md`. No body section was ever written for this entry; this section exists so that citations of R-56 resolve (HY-9 / proposed D12).

## R-57 — When the seam is a TOOL, the scout is one real invocation whose output you read — and first, a

**Verdict:** hit → proposal · **Observed:** 2026-08-06

**Pattern.** **When the seam is a TOOL, the scout is one real invocation whose output you read — and first, a check that the artifact you invoked was built from current source.** Reading an implementation tells you what it intends; running it tells you what it does. Five defects lived in that gap, and every one surfaced from output, not from source: (a) `matched_path_or_any_parents` **panics** rather than errors on an out-of-root path — unknowable from a signature returning `Match`, not `Result`; the first real run died with SIGABRT (exit 134). (b) `resolve_file_line` had no outside-project guard, so a doc citing `/etc/passwd:12` came back **`Resolved`**, range-checked against the host's real file — found because a test written for the panic asserted on the actual verdict. (c) The extractor walks *code-block text* (`Event::Text(content) if in_code_block`), not only spans and links as its own bug file asserted — measured split was 18 prose / 18 fenced / 2 indented, so half the population was invisible to the written claim. (d) `RefPosition` was populated by the parser and read by **nothing**: the discriminator the fix needed already existed, visible only by grepping its uses. (e) mdBook link semantics — ~28 correct links reported missing because the resolver knew one of the repo's two conventions. **Verify the instrument before the measurement:** the local release binary was two days old and predated all four source files of the tool under test; `find src crates -name '*.rs' -newer target/release/codescout` caught it before a single number was trusted. Proposal: for tool-behaviour seams, add to Phase 1 — *invoke once, read the real output, and confirm the binary is newer than the sources you changed.*

**Evidence:** This session: `EXIT=134` panic at gitignore.rs:232 in the vendored ignore-0.4.25 crate (un-code-spanned: it is a registry path, not a repo path); `assertion left != right … /etc/passwd:12 must not resolve, left: Resolved`; `grep RefPosition` → 13 matches, all writes + test constructors, zero reads in the decision path; `ls -la target/release/codescout` 2026-08-04 vs. four newer `audit_doc_refs/*.rs`. Kin R-50 (the view is not the set — the 50-finding cap again made every aggregate wrong, ×5 low), R-19, R-56

**Status:** archived 2026-08-17 — compacted from the index row of `docs/trackers/reconnaissance-patterns.md`. No body section was ever written for this entry; this section exists so that citations of R-57 resolve (HY-9 / proposed D12).

## R-58 — A gate whose verdict depends on the filesystem must be scouted against a CLEAN CHECKOUT, and a

**Verdict:** miss → proposal · **Observed:** 2026-08-06

**Pattern.** **A gate whose verdict depends on the filesystem must be scouted against a CLEAN CHECKOUT, and a rule about paths must be tested in both `foo` and `foo/` form.** Two blind spots, one incident. (a) `Audit Doc Refs` exited 0 locally six consecutive times and went red in CI. The four findings were `.git/worktrees/`, `.claude/worktrees/`, `.codescout/private-memories/` — directories a working tree *has* and a fresh clone has not, so locally the refs **resolved** and produced no finding at any severity. The local run was not a weaker gate; it could not see the class. A working tree is strictly more populated than a checkout (untracked runtime state, worktrees, build output, caches), so every existence check is optimistic in the direction that yields a false pass. Fix, and it costs one command: `git clone --depth 1 file://$PWD /tmp/x` and scan that with the same binary — 881 files, 46673 refs, 0 high, proof rather than a guess, and no CI round-trip. (b) The cap that should have caught two of them **had a passing test** — asserting `.worktrees/my-feature`, a file *inside* an ignored dir. Docs name the directory, `.claude/worktrees/`, and the matcher reads the final path component, which a trailing separator leaves empty; every directory-shaped ref escaped while the suite stayed green. The fixture shape was the blind spot, not the assertion. Proposal for Phase 1: when the seam is a filesystem-dependent gate, scout a clean checkout; when the change is a path predicate, enumerate both separator forms.

**Evidence:** release-promotion W-7; CI run 31106899289 `Audit Doc Refs: failure` with 4 high vs. local `EXIT=0` ×6; fix + fresh-clone proof in `6348dfad`. Kin R-57 (run the tool, don't read it — this is the same law applied to the *environment* rather than the binary), W-5

**Status:** archived 2026-08-17 — compacted from the index row of `docs/trackers/reconnaissance-patterns.md`. No body section was ever written for this entry; this section exists so that citations of R-58 resolve (HY-9 / proposed D12).

## R-62 — A bug file's ## Fix section is a plan, and plans get scouted like any other plan — read the

**Verdict:** hit → proposal · **Observed:** 2026-08-07

**Pattern.** **A bug file's `## Fix` section is a plan, and plans get scouted like any other plan — read the code it prescribes changing before you implement it.** WIN-30's Fix said the background-output test used a fixed wait that should become a bounded poll. The test *already* polled — `for _ in 0..50` with a 100 ms sleep, a 5 s bound — because the Fix had been written from the CI log line (`background command output not captured`) without opening `src/tools/run_command/tests.rs`. Implementing it as written would have been a **no-op committed as a fix**: green suite, archived bug, surviving flake, and a bug file that then asserts the poll was fixed — so the next occurrence reads as a regression of something that never happened. The scout also relocated the real defect, which the prescription had missed entirely: the poll's `if let Ok(v) = out` discarded the `Err` arm, making "never ran" and "still flushing" indistinguishable, which is *why* the CI red carried no information. Prescriptions derived from failure output are the highest-risk kind, because that output is exactly what the plausible-but-wrong mechanism would produce. Proposal for Phase 1: scout a bug file's or plan's prescribed fix like plan code; a `## Fix` naming no `path:line` for the code it changes has probably not opened it.

**Evidence:** WIN-30 (`docs/issues/2026-08-07-windows-ci-timing-flakes-block-the-gate.md`), Root cause + Fix corrected in place; release-promotion F-11 / W-11. The replacement diagnostic paid off on its first run under wine, naming the missing `py` and retiring one member of WIN-27's twelve-test unknown-root-cause cluster. Kin R-19, and the same doc-vs-code drift class as F-3. **Third datapoint, self-referential, same session:** the `grep` bug file I wrote an hour later prescribed inverting `hidden()` into a counting `filter_entry` closure — derived from reading `src/tools/grep.rs:106` but not the `ignore` crate's semantics, where `hidden` also honours `FILE_ATTRIBUTE_HIDDEN` on Windows. Implementing it as written would have silently changed which files are searched on the one platform the repo cannot test locally. So the rule applies to a Fix section written from *partial* code reading too, not only from log lines: scout the dependency's contract, not just the call site that names it

**Status:** archived 2026-08-17 — compacted from the index row of `docs/trackers/reconnaissance-patterns.md`. No body section was ever written for this entry; this section exists so that citations of R-62 resolve (HY-9 / proposed D12).

## R-63 — A target's existing tests encode the behaviour you are about to change; read them before

**Verdict:** hit · **Observed:** 2026-08-07

**Pattern.** **A target's existing tests encode the behaviour you are about to change; read them before editing, not after the gate turns red.** About to invert `resolve_file_symbol` so a mid-index LSP stops reporting existing symbols as missing. Scouting the function first surfaced `resolver_prefers_disk_truth_on_lsp_lag`, which asserted exactly the behaviour being removed — and whose **own name was the argument against its assertion**: it wrote `pub fn bar() {}` to disk and then expected `bar` to be reported missing, when disk truth is that `bar` exists. Renamed and inverted deliberately, with the inversion recorded, rather than discovered as a failing test and "fixed" by adjusting the assertion — which is the likely outcome under time pressure, and which would have quietly re-encoded the old rule. The same scout also found `docs/superpowers/specs/2026-05-16-audit-doc-refs-design.md`'s Tier-1 table stating the old rule as *design intent*, a surface no test covers and `audit_doc_refs` cannot check (prose semantics, not refs). Sibling of R-46: a test module encodes intent the implementation cannot reveal — R-46 read it to CHOOSE between fixes, this read it to discover the fix contradicted a documented one.

**Evidence:** `src/librarian/tools/audit_doc_refs/resolver.rs`; commit `6e608545`; release-promotion #17/#18

**Status:** archived 2026-08-17 — compacted from the index row of `docs/trackers/reconnaissance-patterns.md`. No body section was ever written for this entry; this section exists so that citations of R-63 resolve (HY-9 / proposed D12).

## R-64 — Scout RUNTIME state, not only code, before committing to a long job — a config file's claim

**Verdict:** hit · **Observed:** 2026-08-07

**Pattern.** **Scout RUNTIME state, not only code, before committing to a long job — a config file's claim about the datastore is a claim, and the datastore can be asked directly.** Task #21 was a ~2h `index --force`, justified partly by `.env.gpu` asserting that re-enabling sparse makes a force reindex **mandatory** because chunks written while the flag was set carry an empty sparse vector. Before running it, the live Qdrant collection was queried: it already declared a `sparse` vector (`modifier: idf`), and **92.3% of a 2000-point sample carried POPULATED sparse vectors, 7.7% empty, 0 absent.** The prediction was directionally right and the magnitude — unstated, and load-bearing — was wrong: 7.7% degrade gracefully to dense-only in RRF, so nothing gated the flag flip. `absent` would have been the real hazard, since a missing named vector can error a hybrid query; absent was zero. Generalisation: when the seam is "what state is the datastore actually in", the scout is a query, not a grep — and it is available *before* the expensive operation, not only after it fails.

**Evidence:** `.env.gpu` re-enable note vs Qdrant scroll; commits `da5176d5`, `9886773e`; release-promotion round 9

**Status:** archived 2026-08-17 — compacted from the index row of `docs/trackers/reconnaissance-patterns.md`. No body section was ever written for this entry; this section exists so that citations of R-64 resolve (HY-9 / proposed D12).

## R-65 — Build configuration is a seam, and it was not treated as one

**Verdict:** **miss → proposal** · **Observed:** 2026-08-07

**Pattern.** **Build configuration is a seam, and it was not treated as one.** A `--force` reindex ran seven minutes dense-only against sqlite-vec instead of hybrid against Qdrant, because it was launched with `cargo build --release` — which omits the `server-stack` feature, so `VectorBackend::resolve()` returns `SqliteVec`, which sets `lite`, which sets `dense_only`. Two behaviours changed silently from one build flag. CLAUDE.md documents `cargo rb` for exactly this and `.cargo/config.toml` explains it in six lines, so the information was present and simply not consulted — the seam "which binary am I about to run, and what does its feature set change" was never asked. Caught only by comparing container traffic (dense 73,232 log lines in 2 min, sparse **zero**), i.e. by scouting side effects after the fact. Proposal for Phase 1: **before any job whose behaviour depends on compiled features, scout the build — confirm the binary's provenance and features, not just that a binary exists.** Cheap form: have the tool announce its own resolved configuration in line one, which is what the fix shipped (`backend="qdrant" sparse="on"`). The durable lesson is that a documented rule is not a scout; consulting it is.

**Evidence:** release-promotion F-17; commit `5b77b8b8`; `src/retrieval/code_store.rs::VectorBackend::resolve`

**Status:** archived 2026-08-17 — compacted from the index row of `docs/trackers/reconnaissance-patterns.md`. No body section was ever written for this entry; this section exists so that citations of R-65 resolve (HY-9 / proposed D12).

## R-66 — When a gate reports N failures, scout the gate's own severity and scope policy before fixing N

**Verdict:** hit · **Observed:** 2026-08-08

**Pattern.** **When a gate reports N failures, scout the gate's own severity and scope policy before fixing N things — its silence is a policy statement, not evidence of correctness.** CI's `Audit Doc Refs` failed with exactly **one** high finding: a manual page citing a bug file that had been archived (path moved) the day before. The one-line fix was obvious and would have been wrong-sized in both directions. Reading the tool instead of the finding produced the real shape: `try_basename_fallback` returns `None` for any ref containing `/`, so all 25 dangling citations were genuinely `Missing`; `default_severity` bands `Missing => High`; and `apply_drops` keys on the **citing** document, so `archive/` and `docs/issues/` drop one band each. That is why 24 siblings were invisible — **by design, and correctly**: rewriting an archived snapshot to satisfy a linter that is already ignoring it would falsify the record. So the scope became 12 live surfaces fixed, 13 historical ones deliberately left. The same read then answered a question the gate cannot: `DEFAULT_AUDIT_GLOBS` omits `CHANGELOG.md` and `CONTRIBUTING.md` entirely — 18 broken refs and 8 high findings that no CI run will ever report. **Sub-lesson: a count that moves less than your edit count is a finding.** Fixing 5 markdown refs moved `n_refs_broken` by 3 (9080 → 9077); the missing 2 were the unscanned CHANGELOG pair, and that arithmetic gap is what exposed the coverage hole. Sibling of R-63 (read the target's tests) one level up: read the *judge's* rules, not just the verdict.

**Evidence:** `src/librarian/tools/audit_doc_refs/{resolver.rs:82, severity.rs:21, mod.rs:149}`; CI run 31218234007; `docs/issues/2026-08-08-audit-doc-refs-never-scans-changelog-or-contributing.md`

**Status:** archived 2026-08-17 — compacted from the index row of `docs/trackers/reconnaissance-patterns.md`. No body section was ever written for this entry; this section exists so that citations of R-66 resolve (HY-9 / proposed D12).

## R-67 — Scout the artifact that claims the work is already done — a doc comment, a test name, a count

**Verdict:** hit · **Observed:** 2026-08-08

**Pattern.** **Scout the artifact that claims the work is already done — a doc comment, a test name, a count — because that is the thing which stops the next reader from looking.** Three instances in one PR review, each confirmed at the bytes. (1) `resolve_git_bash_never_selects_the_wsl_launcher` passed because it injected `PATH=C:\Windows\System32`, the spelling the CODE constructs, while Windows setup writes `system32` and `Path` equality folds case only on the drive-letter prefix — so the exclusion never fired on a real host, and the test was named after a guarantee it did not provide. (2) `posix_tokenize`'s doc said it "feeds the security layer's dangerous-command and pipeline checks"; `git grep` returns zero production callers and `is_dangerous_command` still uses `split_whitespace`. (3) `summary.total` printed a confident headline number taken over a truncated set while `by_check` counted the full one. Each names a real hazard and asserts it handled, which is strictly worse than silence: a reviewer greps for the coverage, finds it, and stops. **The scout is cheap and specific — for a claim, find the caller; for a test, ask what mutation survives it; for a count, ask which set it was taken over.** Third sibling to R-63 (read the target's tests) and R-66 (read the judge's rules): read the *reassurance*.

**Evidence:** release-promotion F-21; `src/platform/{windows.rs,mod.rs}`, `src/librarian/tools/doctor.rs`; commit `6f261da9`

**Status:** archived 2026-08-17 — compacted from the index row of `docs/trackers/reconnaissance-patterns.md`. No body section was ever written for this entry; this section exists so that citations of R-67 resolve (HY-9 / proposed D12).

## R-68 — When dispatching reviewers at a diff, the seam is WHICH TREE they will read — and the default

**Verdict:** hit · **Observed:** 2026-08-08

**Pattern.** **When dispatching reviewers at a diff, the seam is WHICH TREE they will read — and the default is the wrong one.** Four subagents were sent at PR #10 while the shared checkout sat on `experiments`, the merge BASE. `symbols()`, `grep()` and `read_file()` all resolve against the working tree, so every reviewer would have read pre-PR bytes and reported confidently on code not in the diff — a review that looks real and is worthless, with no signal distinguishing it from a good one. The brief therefore pinned three refs explicitly (base `47998a33`, head `57ac707f`, tested merge ref `refs/tmp/pr10merge` = `a42f124b`) and mandated `git show <ref>:<path>` into a scratch file before reading. Related and non-obvious: for a `pull_request` event the tested tree is the MERGE ref, a synthetic commit existing on no branch — so "review the PR" and "review the branch tip" are different requests against different bytes (F-23). Iron Law 6 in its sharpest form: a subagent mis-discovering what the controller already knew is a dispatch defect, and the controller is the only party positioned to notice.

**Evidence:** release-promotion W-15, F-23; PR #10 review, 2026-08-08

**Status:** archived 2026-08-17 — compacted from the index row of `docs/trackers/reconnaissance-patterns.md`. No body section was ever written for this entry; this section exists so that citations of R-68 resolve (HY-9 / proposed D12).

## R-69 — Before working from a bug file, plan, or spec, git log -- <the file it names>

**Verdict:** miss · **Observed:** 2026-08-08

**Pattern.** **Before working from a bug file, plan, or spec, `git log -- <the file it names>`.** A bug file is a claim about the world at the moment it was written, and nothing re-validates it. The AST-chunker file was maintained carefully by two sessions — they corrected the "~2h" re-embed to a measured 8.06 min and corroborated the 12% single-line figure from an independent direction — while the fix had already shipped in `ca442498` two days earlier and the file still read "Not yet implemented". Every number was right; the code had moved. Numbers-auditing cannot detect this, because the one surface that changed is the one a numbers audit never touches. The command is one line and would have caught it on 2026-08-07. Generalises past bug files to any document that names code it does not contain.

**Evidence:** release-promotion F-30; chunker file reconciled 2026-08-08

**Status:** archived 2026-08-17 — compacted from the index row of `docs/trackers/reconnaissance-patterns.md`. No body section was ever written for this entry; this section exists so that citations of R-69 resolve (HY-9 / proposed D12).

## R-70 — Before citing a test as coverage, confirm it RUNS — cargo test --test <file> and look for the

**Verdict:** miss · **Observed:** 2026-08-08

**Pattern.** **Before citing a test as coverage, confirm it RUNS — `cargo test --test <file>` and look for the name in the output.** A `#[cfg(feature = "x")]` on a test is a claim that some lane enables `x`; verify it (`grep -rn '<feature>' .github/workflows/`) rather than assume. `server-stack` was in neither `default` nor any workflow, so its tests were never compiled — and a filtered-out test is indistinguishable from a test that does not exist, which is worse than an obvious hole because the file *looks* covered: grep for the symbol, find a test named after it, stop. Three mechanisms make a test unable to fail (deleted with its consumer, asserting a subset under a name claiming all, never compiled) and all three were live in this repo on one afternoon.

**Evidence:** release-promotion F-31; buddy memory `tests-that-cannot-fail`

**Status:** archived 2026-08-17 — compacted from the index row of `docs/trackers/reconnaissance-patterns.md`. No body section was ever written for this entry; this section exists so that citations of R-70 resolve (HY-9 / proposed D12).

## R-71 — A remediation hint inside an error message is a claim about an API, not an instruction —

**Verdict:** hit · **Observed:** 2026-08-08

**Pattern.** **A remediation hint inside an error message is a claim about an API, not an instruction — verify the symbol before writing it.** `qdrant-client`'s failure text reads *"Set check_compatibility=false to skip version check"*, and `check_compatibility` is a struct field; the public builder method is `skip_compatibility_check()`. Writing what the message said would not have compiled. Cheap in Rust, where the compiler is the scout of last resort — the same reflex against a runtime-resolved name (an env var, a JSON key, a CLI flag) fails silently at runtime instead, and the message's authority makes it the least likely line anyone re-checks. Scout cost was one grep of the vendored crate, which also surfaced that the diagnostic is a `println!` rather than `eprintln!` — the actual defect.

**Evidence:** release-promotion F-32 + W-24; `docs/issues/archive/2026-08-08-qdrant-compat-check-printlns-to-stdout.md`

**Status:** archived 2026-08-17 — compacted from the index row of `docs/trackers/reconnaissance-patterns.md`. No body section was ever written for this entry; this section exists so that citations of R-71 resolve (HY-9 / proposed D12).

## R-72b — A git SHA in a subagent brief is a claim with an expiry date — re-derive HEAD at dispatch, and

**Verdict:** miss · **Observed:** 2026-08-13

**Pattern.** **A git SHA in a subagent brief is a claim with an expiry date — re-derive HEAD at dispatch, and tell the agent to report drift.** Briefed an agent with `experiments @ c5f434df`, five days stale. The checkout had moved to a feature branch carrying **+2,592 insertions across the retrieval query path** — the exact subsystem under investigation — so every `path:line` in the returned report silently described a different branch than the one requested. The behavioural findings survived because they were observed against a running server; the mechanism citations did not. Scout cost is one `git branch --show-current && git log --oneline -1` at dispatch time. Cheaper still, add *"verify the branch and tip yourself and report any disagreement with this brief"* to the brief — the agent found it here unprompted, but only because it happened to run `git worktree list`. Generalises to any brief carrying a version, path, or line number the parent has not re-read this turn.

**Evidence:** release-promotion F-34

**Status:** archived 2026-08-17 — compacted from the index row of `docs/trackers/reconnaissance-patterns.md`. No body section was ever written for this entry; this section exists so that citations of R-72b resolve (HY-9 / proposed D12).

## R-73b — Before reasoning about whether a mechanism exists, grep the layer that would own it

**Verdict:** miss · **Observed:** 2026-08-13

**Pattern.** **Before reasoning about whether a mechanism exists, grep the layer that would own it.** Twice in one session I inferred absence from architecture and was wrong: *"MCP pushes no cwd change, so there is likely no signal to miss"* (a `PostToolUse` hook on `EnterWorktree` already fires and instructs — `hooks.json:141`), and *"how would we express an exclusion across two backends with different `project_id` semantics"* (`exclude_languages` was already that mechanism, contract-tested in both stores, divergence documented in-code). Both inferences were structurally sound and both were false, because architecture tells you what *can* exist, not what *does*. The refutation is a single grep of the owning layer — the harness directory for a harness signal, the sibling axis for a filter mechanism — and it is cheaper than the reasoning it replaces. It also changes the work: "design this" becomes "extend that".

**Evidence:** release-promotion F-35

**Status:** archived 2026-08-17 — compacted from the index row of `docs/trackers/reconnaissance-patterns.md`. No body section was ever written for this entry; this section exists so that citations of R-73b resolve (HY-9 / proposed D12).

## R-74b — Before recommending at a trait boundary, enumerate the implementors and read the second one

**Verdict:** hit · **Observed:** 2026-08-13

**Pattern.** **Before recommending at a trait boundary, enumerate the implementors and read the second one.** A fix design was recommended after reading `qdrant.rs:317`'s `must_not` filter; the second `CodeVectorStore` impl opens a database **file** per `project_id` (`sqlite_code_store.rs:70`), making the recommendation inexpressible there. The signature `query(collection, project_id, …)` is what hides it — one parameter name, two meanings — and the pair is contract-tested for parity, so the suite would have stayed green while the backends diverged in behaviour. The scout is `grep -n "impl .* for"` on the trait's file, or `references()` on the trait. The durable form: "the shape of the impl I read" is not "the shape of the boundary", and a parameter name shared by two implementations is not a shared meaning.

**Evidence:** release-promotion F-36; caught by applying architecture-snow-lion Self-Trap 2

**Status:** archived 2026-08-17 — compacted from the index row of `docs/trackers/reconnaissance-patterns.md`. No body section was ever written for this entry; this section exists so that citations of R-74b resolve (HY-9 / proposed D12).

## R-72 — A plan's code block is a claim about an API, and the workspace usually already holds the

**Verdict:** miss (recurrence of R-70) → proposal · **Observed:** 2026-08-11

**Pattern.** **A plan's code block is a claim about an API, and the workspace usually already holds the answer — grep for an in-repo user of the type before transcribing, and check which lane actually compiles the file.** PR #13's `LocalOnnxEmbedder` was transcribed verbatim from its plan (`docs/superpowers/plans/2026-08-08-local-onnx-embedder.md:403`, `fn embed_texts(&self)`) against `fastembed 5`, whose `TextEmbedding::embed` takes **`&mut self`** (`fastembed-5.13.4/src/text_embedding/impl.rs:447`, un-code-spanned: registry path, not a repo path). The correction was already in this workspace, written as a comment: `crates/codescout-embed/src/local.rs:82` — *"fastembed 5 changed embed() to &mut self — Mutex serializes access across spawn_blocking tasks"* — one `grep TextEmbedding crates/` away at plan-authoring time, and the line above it (`Arc<Mutex<TextEmbedding>>`) is the whole fix. R-70's mechanism is what let it reach CI: `src/retrieval/mod.rs:11` gates the module on `local-embed-dynamic`, which no test lane and no command in the PR's own five-line test plan enables — only `cargo check --features local-embed-dynamic --all-targets` (`.github/workflows/ci.yml:157`) compiles the file, so a PR reporting 3396 passing tests had compiled the file it exists to add exactly zero times. The author *did* run that command locally; it died inside `ort-sys`'s build script (CyberArk EPM, `os error 5`) before reaching the crate — an attempted compile that produced no compile, which reads in a test plan like an environment note rather than a coverage hole. Second error, same file, same root: `.unwrap_err()` needs `Self: Debug`, and `TextEmbedding` (`init.rs:127`) has no `Debug` impl, so the reflex `#[derive(Debug)]` is also uncompilable — both wrong fixes cost a full ort build to disprove, the grep cost nothing. Proposal for Phase 1: when plan code calls a third-party type, grep the whole workspace for an existing user before transcribing it; and treat *"which lane compiles this file?"* as part of the feature-gate scout rather than as CI's job — an attempted-but-aborted build is not a build.

**Evidence:** pr-review F-5 + F-6 + W-4; CI run 31377316509 `Feature check (opt-in build configs): FAILURE` (E0596 + E0277) against 16 green lanes; `docs/issues/2026-08-08-cyberark-epm-blocks-ort-sys-build-script.md`. Kin R-70 (a `#[cfg(feature)]` test is a claim that some lane enables it — here a whole module), R-71 (an error message's remediation hint is a claim about an API; here a *plan* is), R-34 (the host build is not the gate for the target the branch exists to support)

**Status:** archived 2026-08-17 — compacted from the index row of `docs/trackers/reconnaissance-patterns.md`. No body section was ever written for this entry; this section exists so that citations of R-72 resolve (HY-9 / proposed D12).

## R-73 — A guard's unit test proves the guard works

**Verdict:** hit → proposal · **Observed:** 2026-08-11

**Pattern.** **A guard's unit test proves the guard works. It does not prove anything calls it — mutate the CALL SITE, not just the function.** Across one 9-task branch, three separate things were correct, unit-tested, and deletable from their call sites with the entire suite green: (a) `guarded_api_key`'s invocation — replacing it with the raw config key survived 3505 tests, and that mutation IS a project.toml credential sent cleartext to `http://embed.example.com`; (b) `guard_sparse`'s invocation — deleted, suite green, which is the silent dense degradation the guard exists to prevent; (c) the ENTIRE project-config read that a whole task existed to add — replaced with `let embeddings = None` and 3647 tests passed. In each case the function's own test genuinely died when the function body was mutated, which is what made the coverage look real. The two questions — *does this guard work* and *does this path invoke it* — read identically and are answered by different mutations. Fix pattern that worked three times: extract the wiring into a named, callable seam (`build_embedder`, `merge_embed_config`, `map_from_env_error`) so the guard sits inside something a test can invoke; testability shaped the factoring, and none of the extractions changed behaviour. Corollary observed twice: an UNCOVERED branch is where the next bug lands — a mutation that SURVIVED (`unwrap_or(fallback)` → `unwrap_or(0)`) marked the exact arm into which a later fix introduced a Critical two rounds afterwards. Proposal for Phase 1 and for every implementer brief: require the call-site mutation explicitly, and treat "the guard has a test" as necessary and not sufficient.

**Evidence:** local-onnx-embedding session log F-1, W-2; `src/retrieval/client.rs` `build_embedder`; branch `feat/local-onnx-query-path`, 15 fix rounds. Kin R-49 (your own bug file is a hypothesis), R-57 (run the tool, don't read it)

**Status:** archived 2026-08-17 — compacted from the index row of `docs/trackers/reconnaissance-patterns.md`. No body section was ever written for this entry; this section exists so that citations of R-73 resolve (HY-9 / proposed D12).

## R-77 — A search that does not cover the space produces the same confident absence as no search at all

**Verdict:** miss (third recurrence) → rule · **Observed:** 2026-08-14

**Pattern.** **A search that does not cover the space produces the same confident absence as no search at all — and feels safer, because you did look. When asserting that no artifact of a CLASS exists, search for the class across the tree, not for a filename in the directory where you expect it.** Briefing an implementer on companion-plugin hook work, I ran `ls` on `codescout-companion/hooks/`, saw `session-start.test.sh`, `worktree-write-guard.test.sh` and five other `*.test.sh` siblings, saw no `worktree-activate.test.sh`, and wrote *"this hook had no test while nearly every sibling has one"* into the brief. **`tests/test-worktree-activate.sh` had existed all along** — 86 lines, 6 tests, on the repo's shared `tests/lib/fixtures.sh` helpers, passing 6/6, covering four cases the new suite did not (silent exit without codescout, fallback worktree detection, the `.codescout` symlink, the embeddings symlink). The repo keeps suites under **two** conventions (`hooks/*.test.sh` and `tests/test-*.sh`) and I searched one, so the implementer correctly built a second, redundant harness against a false premise. This is the **third** instance of the same class in one work stream, and the disguise is what makes it worth its own row: the first two were architectural inference (concluding the `EnterWorktree` hook did not exist — it was at `hooks.json:141`; concluding no exclusion mechanism existed — `exclude_languages` was already contract-tested in both stores), where the tell is *"I reasoned instead of grepping"*. Here I **did** search, and the conclusion still inherited a scope nobody stated. `ls hooks/` answers *"what is in hooks?"*; it never answers *"does this hook have a test?"* Caught only because the reviewer read the repo rather than trusting the brief's premise — the third time in that run that the *verify-don't-accept* instruction paid for itself. Rule for Phase 1: phrase the query as the question you actually have (`grep -rl worktree-activate` across the repo, not `ls` of one directory), and treat any absence claim in a brief as needing a tree-wide search behind it.

**Evidence:** worktree-semantic-search session log F-5; fix `d65f96d`. Kin R-3 (scout limited grep to one file/crate), R-45 (relocating a file needs discovery-by-scan, which caller enumeration cannot substitute for), R-26 (a grep line-match locates a symbol, it does not confirm a mechanism)

**Status:** archived 2026-08-17 — compacted from the index row of `docs/trackers/reconnaissance-patterns.md`. No body section was ever written for this entry; this section exists so that citations of R-77 resolve (HY-9 / proposed D12).

## R-76 — Extracting a decision into a testable unit CREATES a new seam — the dispatch to it — and the

**Verdict:** miss (recurrence of R-73) → rule · **Observed:** 2026-08-14

**Pattern.** **Extracting a decision into a testable unit CREATES a new seam — the dispatch to it — and the seam inherits none of the tests. R-73 said mutate the call site; this is R-73 one layer up, at a trait override, in a session whose own tracker already carried R-73.** The final review of the worktree-delta-search branch found `merge_hits` sorting Qdrant RRF scores as if comparable across two queries — measured on 576k live points, RRF returns `1/(1+rank)`, so a 3-chunk delta scored identically to a 500k-chunk index and the delta took a fixed **3/12** of every page. The fix moved the union into `CodeVectorStore::query_overlay`, overridden by Qdrant. `build_query_filter` then had 4 pure unit tests **plus** a live-Qdrant test proving real effect. The **override that routes Qdrant to it had none**: its only caller is `RetrievalClient::search_in`, no test builds a Qdrant-backed client, and the live test called `wrap.hybrid_query(...)` directly one layer beneath. Deleting the override silently reverted to the two-query default — C1 back — with **3718 tests and all 4 ignored qdrant tests green**, and the fix-wave report described the seam as *"pinned only by `#[ignore]`d live tests"* when it was pinned by nothing. Same session, same shape one layer lower: four Task-7 mutations survived 3688 tests (`file_name()`→`to_string_lossy()` on the delta key; swapping the two `SearchOpts` assignments; deleting the `worktree_state_warning` assignment; swapping the drift-note arguments) — every pure decision function was tested and every assignment wiring them together was not. The aggravating fact is that R-73 already prescribed *"require the call-site mutation explicitly in every implementer brief"* and the fix-wave brief did not, so this is failure to apply a written rule rather than absence of one. Remedy cost one changed call — point the existing live test at the trait method instead of the inherent one — and the confirming mutation produced the run's cleanest evidence: with the override deleted, the "union query" row came back **byte-identical** to the two-query row (delta 3/12, noise 2) while the suite stayed green. Rule for Phase 1 and for every brief: after any extraction, ask *what now chooses this, and is that choice tested?* — and require a mutation that deletes the dispatch, not just one that breaks the callee. Two verification techniques from the same reviews, both cheap and both reusable: **empirical non-vacuity** (copy the tree to scratch, revert only the fixed block, run, and report the pass/fail split — 3/7 and 6/3 were confirmed this way rather than reasoned), and **corroborating a claim about now-unobservable state by arithmetic** (a report's pre-edit grep of 21 matches in 5 files was confirmed post-edit as 43 − 19 new-file − 3 added = 21; as the reviewer put it, *"that arithmetic is not something a fabricated report lands on by accident"*).

**Evidence:** worktree-semantic-search SDD ledger, final whole-branch review + fix-wave re-review; `src/retrieval/code_store.rs` `query_overlay`, `src/retrieval/qdrant.rs` `build_query_filter`, fix `c284786c`. Kin R-73 (mutate the call site, not the function), R-70 (a test that cannot run is indistinguishable from one that does not exist), R-49

**Status:** archived 2026-08-17 — compacted from the index row of `docs/trackers/reconnaissance-patterns.md`. No body section was ever written for this entry; this section exists so that citations of R-76 resolve (HY-9 / proposed D12).

## R-78 — A bug file's ## Symptom is observed; its ## Root cause is usually inferred and then written in

**Verdict:** miss ×3 in one sweep (recurrence of R-59) → rule · **Observed:** 2026-08-14

**Pattern.** **A bug file's `## Symptom` is observed; its `## Root cause` is usually inferred and then written in the same declarative voice. Nothing marks the seam, so the inference reads as the finding — and a wrong one does not merely waste work, it can redirect you away from a worse defect.** Verify-opening eight backlog bugs, three had premises that falsified on contact. `db5fe933` (state-protocol `.sh`) actively warned the fixer OFF five rows, calling buddy's hook "a different, legitimately-`.sh` component"; `ls buddy/hooks/` returns `hook_dispatch.py, hooks.json, judge.env, run.mjs` — no `session-start.sh` exists anywhere, and `hooks.json:4` dispatches `node run.mjs session-start`. Following it would have fixed **4 of 9** rows and closed the bug. `b86d81f1` (IL3 quoted pipe) asserted "the caller already does the quote-aware thing via `pipeline_segments`"; that function scanned raw bytes with **no quote state at all** — the same defect in two functions, one recorded as the other's solution. Following it would have fixed the false positive and left a **measured bypass** in place: a quoted `;` before a real pipe hid it from the enforcer, an unbounded-producer-to-trimmer `git log ... | head -3` returning `exit_code: 0`. `a99388a2` (edit_file ack handle) claimed the `@ack_*` mechanism was broken; it resolves in both call shapes, `git merge-base --is-ancestor 151cc9df 8f724171` is **true** (the fix was already in the SHA the bug cites) and `git log --since` over that area is empty, so the ENOENT was the target file. The aggravating detail: **two of the three carried their own refutation.** `a99388a2` recorded "the handle IS reaching parameter validation" under Symptom and argued past it; `db5fe933` left the companion-side hypothesis `deferred` while stating the buddy-side equivalent as settled. R-59 already says a bug file's Root cause is an unverified reading; what this adds is that the *falsifying command is usually one line and already implied by the file* — `ls`, read one function, one `merge-base`. **Rule for Phase 1:** before implementing a bug's Resume, extract the single command that would falsify its Root cause and run that first; and read the file's own Symptom/Hypotheses for a sentence that already contradicts it.

**Evidence:** bug-fix session log F-39; fixes `2a36bfbf`, `6bb88a22`, `ffaf67a3`. Kin R-59 (the seam this recurs from), R-26 (a grep line-match locates a symbol, it does not confirm a mechanism), R-19 (do not assert a checkable fact unread)

**Status:** archived 2026-08-17 — compacted from the index row of `docs/trackers/reconnaissance-patterns.md`. No body section was ever written for this entry; this section exists so that citations of R-78 resolve (HY-9 / proposed D12).

## R-82 — A tool's error message is a hypothesis about itself, not a reading of its own code — verify

**Verdict:** miss ×2 in one day → hard rule · **Observed:** 2026-08-15

**Pattern.** **A tool's error message is a hypothesis about itself, not a reading of its own code — verify before recording it or acting on it.** Twice in one day I believed one and was wrong both times. (a) `edit_file`'s *"edit contains a symbol definition"* led me to write "naive substring match" into F-44; the guard is actually diff-aware, multi-line-gated and comment-aware, and the real defect was one layer in (an unanchored per-line match). (b) `edit_code`'s *"AST parse failed — the file likely has syntax errors"* was emitted on a syntactically perfect file; the true cause is that tree-sitter's extractor does not surface methods of an impl nested in a function body, which the LSP resolves fine. In both cases the message named a **mechanism**, and in both cases the mechanism it named was not the one operating. The asymmetry is what makes this expensive: an error is written once by an author reasoning about the general case, then read as evidence by every session that hits it — so a wrong one is a false premise with unlimited distribution. Worse, F-46 (same day) shows a test can *pin* the wrong message, so correcting it surfaces as a failing gate. **Rule:** when an error message states a cause and you are about to record it, cite it, or act on it, open the emitting code first — it is one `grep` on the message string. Treat the message as the *symptom*, never the diagnosis. Corollary: when writing an error yourself, prefer a checked cause over an asserted one — `has_syntax_errors` already existed next to the hint that was guessing about syntax errors.

**Evidence:** bug-fix work stream; F-44 (corrected), F-46, W-38; `138de7c5`, `cafa4b37`. Kin R-80 (a bug file's fix rationale is unverified), R-59/R-78 (its root cause is unverified) — same failure with a different author: there the bug file, here the tool itself

**Status:** archived 2026-08-17 — compacted from the index row of `docs/trackers/reconnaissance-patterns.md`. No body section was ever written for this entry; this section exists so that citations of R-82 resolve (HY-9 / proposed D12).

## R-86 — A component's transport or deployment mode is an instrument, and a test that constructs the

**Verdict:** miss (a shipped, archived, "verified" fix) → hard rule · **Observed:** 2026-08-15

**Pattern.** **A component's transport or deployment mode is an instrument, and a test that constructs the simplest one verifies the simplest one.** `eedb308c` claimed to re-sync a file changed on disk. It shipped **inert on the mux path**, which is the path this project uses for Rust, and I archived the bug as fixed. Mechanism: `did_open` records the on-disk signature, and on SOCKET transport it never takes its already-open early return (the mux owns document dedup), so every `document_symbols` re-recorded the current state before the drift check ran — comparing the file against itself — while the mux deduped the `didOpen` so the server kept its stale copy. Two halves silently cancelling. The end-to-end test passed the whole time because it drives `LspClient::start` with `mux: false`: **it exercised the one transport on which the defect cannot appear.** This is the R-81 family with a new instrument — there the build's *feature set*, here the *transport* — and both are invisible in the code you are reading and in the test that passes. **Rule:** before calling a fix verified, name every transport / deployment mode the component has and ask which one the test constructed and which one production runs. If they differ, the test is a smoke test. Prefer asserting a **mode-independent invariant** over parameterising: extracting `SyncedSignatures` made "a notification that may be deduped must not claim delivery" testable without standing up a mux, which is why it had no test before. Corollary: a fix whose correctness depends on **call ORDER** between two functions is one refactor from silent regression — encode the rule in the data (`record(overwrite: bool)`) so ordering stops being load-bearing.

**Evidence:** bug-fix work stream; F-49, W-41; `eedb308c` (inert) → `7c8863f0` (real). Kin R-81 (build configuration as instrument), R-77/R-79 (a bounded probe's empty result), W-41 (live-verify after rebuild)

**Status:** archived 2026-08-17 — compacted from the index row of `docs/trackers/reconnaissance-patterns.md`. No body section was ever written for this entry; this section exists so that citations of R-86 resolve (HY-9 / proposed D12).

## R-83 — "Reproduces only on platform X" is a claim about your tooling, not about the bug — check

**Verdict:** miss (a week stale) → rule · **Observed:** 2026-08-15

**Pattern.** **"Reproduces only on platform X" is a claim about your tooling, not about the bug — check `rustup target list --installed` before believing it.** Two bugs sat scoped to a Windows host for a week (`910c9920` clippy drift, and the framing around `9fa78b81`). `x86_64-pc-windows-gnu` was an installed *target* the whole time, and `cargo clippy --target x86_64-pc-windows-gnu --all-targets -- -D warnings` reproduced both on Linux in 40 s. **Cross-target lint/typecheck needs no linker, no emulator, and no second machine** — it is the platform's `cfg` evaluated locally. It also corrected the count: only **2 of the 3** filed lints still fire; the `thread_local` one was a clippy-version artifact on code that already carried `const { }`. Generalises past Rust: any toolchain with a cross target (`GOOS`, `--target`, a cfg flag) can evaluate another platform's conditional code where you sit. **Rule:** before recording a bug as host-scoped, spend one command asking whether the *checker* can be pointed at that host's configuration. The fix that follows is usually also mis-scoped — here the `needless_return` was never about the keyword, it was that `#[cfg(unix)]` erased every assertion around it and left a test that ran, asserted nothing, and reported `ok`.

**Evidence:** bug-fix work stream; W-39; `2f76136e`. Kin R-81 (the build configuration is an instrument), R-77 family (a search that does not cover the space)

**Status:** archived 2026-08-17 — compacted from the index row of `docs/trackers/reconnaissance-patterns.md`. No body section was ever written for this entry; this section exists so that citations of R-83 resolve (HY-9 / proposed D12).

## R-84 — A count and a category are two claims

**Verdict:** miss → hard rule · **Observed:** 2026-08-15

**Pattern.** **A count and a category are two claims. Verifying the category on a sample does not verify it across the count — and the summary is what decides whether anyone reads the rows.** A bug file stated *"the 8 high findings are historical release sections"*, having checked that label against the `src/embed/*` subset, where it was true. Re-measured on HEAD: **10** highs, in **three** classes. Seven were historical. One was `src/foo.rs` — an illustration of a generated header format, not a reference. And **one was live drift the gate caught the moment it could see the file**: `CHANGELOG.md` cited a `docs/issues/…` path after that bug file had been archived to `docs/issues/archive/`, with the *very next bullet* citing an archived file correctly. The recommended fix — forgive released sections wholesale — would have forgiven the real bug along with the rest. **Rule:** when a filing pairs a number with a label ("the N findings are all X"), treat them as separate claims and re-derive both; if the label came from a sample, say which sample. Sub-rule for reconciliation work: classify every row before designing the sweep, because a sweep designed from the majority class silently absorbs the minority.

**Evidence:** bug-fix work stream; W-39; `130c93a5`. Kin R-80 / R-78 / R-59 (a bug file's own reasoning is unverified), R-54 (before counting rows, ask what one row IS)

**Status:** archived 2026-08-17 — compacted from the index row of `docs/trackers/reconnaissance-patterns.md`. No body section was ever written for this entry; this section exists so that citations of R-84 resolve (HY-9 / proposed D12).

## R-85 — When a test blocks your change, ask what it is a statement ABOUT: a defect, or a contract

**Verdict:** hit ×2, opposite verdicts → hard rule · **Observed:** 2026-08-15

**Pattern.** **When a test blocks your change, ask what it is a statement ABOUT: a defect, or a contract. The two are indistinguishable from the failure message and demand opposite responses.** Both happened the same day, on the same kind of surface. (a) `edit_file_warns_hint_suggests_remove_when_new_empty` failed after I exempted pure deletions from `guard_structural_rewrite`. It pins a **contract** — and `infer_edit_hint` carries machinery built specifically to route the deletion case to `edit_code(remove)`. The block was designed, not overlooked; **my code was wrong and I reverted it** (`80350afe` → `9b902e0a`). (b) `did_change_refreshes_stale_symbol_positions` failed after `document_symbols` learned to re-sync a file changed on disk. Its step 3 asserted that a query after an external write returns *stale* positions — a **demonstration of the defect**, written to justify making callers flush by hand. That premise was the bug; the test became obsolete the instant it closed, and rewriting it was right (`eedb308c`). **Rule:** before yielding to or overriding a failing test, read what it was written to establish. Purpose-built support nearby (a bespoke hint, a dedicated error branch) is evidence of *contract*; prose like "should still return the old…" or "reproduce the … bug" is evidence of *defect*. Getting it backwards costs an implement-then-revert cycle in one direction and a worked-around fix in the other.

**Evidence:** bug-fix work stream; F-47, W-39; `9b902e0a`, `eedb308c`. Kin R-82 (an error message is a hypothesis), F-46 (a test can pin a false diagnosis)

**Status:** archived 2026-08-17 — compacted from the index row of `docs/trackers/reconnaissance-patterns.md`. No body section was ever written for this entry; this section exists so that citations of R-85 resolve (HY-9 / proposed D12).

## R-81 — A build configuration is an instrument, and a zero from the wrong one is evidence about the

**Verdict:** miss (fifth in the R-77 family) → new instrument class · **Observed:** 2026-08-14

**Pattern.** **A build configuration is an instrument, and a zero from the wrong one is evidence about the build.** Live-verifying the new `migrate-memories --in-place` against this project, `cargo run --bin codescout` reported `read: 0` — one sentence from "this project has no semantic memories, nothing to repair", which would have closed a bug on a false premise and retracted a true earlier statement. Cause: `cargo run` builds **default features**, and `server-stack` is not one (`default = ["remote-embed", "http", "librarian"]`), so `VectorBackend::resolve()` took its `cfg(not(server-stack))` arm and listed an empty sqlite-vec *lite* store — while the live server, built by `cargo rb` (`--features server-stack,local-embed`), talks to Qdrant. Two stores, one command, no error either way. Refuted by `memory(action="recall")` returning three memories including `architecture`; the `cargo rb` binary then reported `read: 15, anchors_attached: 37`. R-77/R-79 named the class for *search scope*; this is the same error where the instrument is the **compiled feature set**, which is invisible in the command you typed and in its output. **Rule:** before believing any zero/empty from a locally-built binary that touches live state, confirm the binary was built with the same features as the running service — for codescout that means `cargo rb` and `./target/release/codescout`, never `cargo run`. And corroborate on a second surface: the falsifying `recall` was one call.

**Evidence:** bug-fix work stream; F-43, and the archived memory query-prefix bug's § Resume. Kin R-77 / R-79 (a bounded search's empty result), R-50 (the view is not the set)

**Status:** archived 2026-08-17 — compacted from the index row of `docs/trackers/reconnaissance-patterns.md`. No body section was ever written for this entry; this section exists so that citations of R-81 resolve (HY-9 / proposed D12).

## R-80 — Verify the bug file's argument for NOT fixing, not only its root cause

**Verdict:** hit (extends R-59/R-78 one level up) · **Observed:** 2026-08-14

**Pattern.** **Verify the bug file's argument for NOT fixing, not only its root cause.** Two of the three bug files worked this round carried a section arguing the obvious fix was wrong — `c7ba92f5` had *"Why the obvious fix is wrong"*, concluding both symptoms were "status-string-only" and that widening the shared classifier was "strictly riskier than a status-string residual". One `references(symbol="backend_is_local")` call refuted it: **four** consumers, three of them live query behaviour, and `build_embedder` reads the predicate **three lines above** the construction the file said was unaffected. The live path was already wrong, so widening was the fix rather than the risk — the risk being weighed was the risk of *not* widening (F-41). R-59 and R-78 cover an unverified `## Root cause`; this is the same failure applied to the **fix rationale**, and it is strictly worse: a wrong root cause yields a wrong fix that a test can catch, while a wrong rationale yields *no fix*, wearing a persuasive justification that makes every later verify-open pass skip the file as settled. **Rule:** a `## Root cause` section that argues against fixing is a claim about the blast radius of a change; check it the way you would check any other claim — enumerate the symbol's real consumers with `references`, and read the function that consumes it, not only the function that reports it. The falsifying call is one line and the file itself names the symbol.

**Evidence:** bug-fix work stream; F-41 + `5b7536f5`. Kin R-59 / R-78 (a bug file's root cause is unverified reading), R-3 (grep the workspace, not the file in front of you)

**Status:** archived 2026-08-17 — compacted from the index row of `docs/trackers/reconnaissance-patterns.md`. No body section was ever written for this entry; this section exists so that citations of R-80 resolve (HY-9 / proposed D12).

## R-79 — A bounded search returning nothing is evidence about the search, not about the world — and it

**Verdict:** miss (fourth recurrence of R-77) → hard rule · **Observed:** 2026-08-14

**Pattern.** **A bounded search returning nothing is evidence about the search, not about the world — and it must never authorise a destructive action.** Reclaiming 25 GB of leaked test databases, nine survivors needed attributing to real projects. A check looping each name over three parent directories reported **`backend-kotlin` and `MRV-poc` as orphaned**. Widening to `find /home/marius/work -maxdepth 4 -type d -name <p>` found both immediately: `/home/marius/work/mirela/backend-kotlin` and `/home/marius/work/stefanini/southpole/MRV-poc`. Acting on the shallow result would have deleted **118 MB of live index for the project task #46 tracks 13 pending `worktree_scoped_row` merges against**. R-77 named this class the same day at brief-writing cost; this is the same error where the output is irreversible, which is what earns a separate row. The two stores actually removed each satisfied **both** conditions — no directory at depth 4 **and** older than 30 days — and one (`code-explorer.db`) had had its absence independently measured earlier in the same session when task #45 removed its registry root. **Rule:** a delete may be authorised by a *positive* finding (this path is gone, measured) but never by a *negative* search result alone; require two independent signals, and prefer one established for a different reason.

**Evidence:** bug-fix work stream, `2faaf32a` cleanup (25 GB → 285 MB, `25d5a84d`). Kin R-77 (the class), R-50 (the view is not the set), R-3 (a grep scoped to one file misses cross-boundary sites)

**Status:** archived 2026-08-17 — compacted from the index row of `docs/trackers/reconnaissance-patterns.md`. No body section was ever written for this entry; this section exists so that citations of R-79 resolve (HY-9 / proposed D12).

## R-74 — The controller's own plan text is a claim, not an instruction — write briefs that demand a

**Verdict:** miss ×6 → proposal · **Observed:** 2026-08-11

**Pattern.** **The controller's own plan text is a claim, not an instruction — write briefs that demand a dead mutant rather than a green run, and implementers will correct the plan instead of transcribing it.** Six defects in one plan, every one caught by an implementer or reviewer following the evidence requirement rather than the instruction: (1) Global Constraints mandated `EnvGuard` + `#[serial]` in three places — banned crate-wide by `docs/conventions/test-env-isolation.md`, and written from memory of the convention rather than from it; (2) a guard comparison, `model_dim.unwrap_or(index_dim)` vs `index_dim`, whose operands default to each OTHER so it can never fail in the common case; (3) a comment asserting `strip_prefix("local:")` would swallow `"local-dir:"` — false, byte 5 is `-`, disproved by mutation; (4) a supplied "fix" asserting a library registry against a FRESH LITERAL in the test, so it guarded upstream drift and not our shipped value — the implementer implemented it, mutation-tested it, found it inert, and replaced it with shared constants; (5) a single-fixture mutation test that could not discriminate two mutants, because a guard clause short-circuits before the branch the second one needs; (6) `#[test]` + `futures::executor::block_on` against a fn that calls `spawn_blocking`, which panics on every call including the failure path the test asserted on. The through-line: each was plausible prose that only reading or running the substrate could refute. What made the corrections happen was the brief's standing line — *a test you have not seen fail is not evidence*, and *when an instruction of mine contradicts what you measured, follow the measurement and say so*. Proposal: put both sentences in every implementer brief, and record controller errors in the ledger by number so the rate is visible rather than smoothed away.

**Evidence:** local-onnx-embedding session log F-2, F-3, W-4; plan corrections `893ade0e` + the `docs(plan)` commits on `feat/local-onnx-query-path`. Kin R-62 (a `## Fix` section is a plan and gets scouted like one), R-71 (an error message's hint is a claim about an API)

**Status:** archived 2026-08-17 — compacted from the index row of `docs/trackers/reconnaissance-patterns.md`. No body section was ever written for this entry; this section exists so that citations of R-74 resolve (HY-9 / proposed D12).

## R-61 — Before filing a bug against configuration that looks wrong, grep for a test that asserts it

**Verdict:** hit → rule · **Observed:** 2026-08-07

**Pattern.** **Before filing a bug against configuration that looks wrong, grep for a test that asserts it.** A live `pgrep` of three kotlin-lsp mux instances showed the worktree instance carrying `GRADLE_USER_HOME=/tmp/codescout-mux-gradle-26a9e85d58931839` — its **parent repo's** hash — while its `-Duser.home=` and `--system-path=` both used its own `509dba098b20943c`. That is the exact shape of a cross-instance leak, and it is the family a previously-fixed bug lived in (per-instance system-path, `dc44ac3d`), so the prior felt high. It is asserted behaviour: `kotlin_worktree_shares_gradle_home_but_not_system_path` in `src/lsp/servers/mod.rs` requires *“worktree and its main repo must share GRADLE_USER_HOME”* and *“must NOT share the IntelliJ system dir”* — deliberate, because Gradle caches are huge and worth sharing across worktrees while the IntelliJ index is not. The asymmetry that reads as a bug **is** the design. Recorded as an explicit *non-finding* in the kotlin bug so the next reader does not re-investigate it. Rule for Phase 1: when a config value surprises you, search the suite for its invariant before writing the bug — a deliberate asymmetry and a leak look identical from the process table.

**Evidence:** kotlin-lsp uncapped-heap bug, 2026-08-07 update; pids 173473 / 3148316 / 3478380. Kin R-19 (assert nothing checkable you have not read this session)

**Status:** archived 2026-08-17 — compacted from the index row of `docs/trackers/reconnaissance-patterns.md`. No body section was ever written for this entry; this section exists so that citations of R-61 resolve (HY-9 / proposed D12).

## R-60 — When you implement a surface that already exists elsewhere in the codebase, read the existing

**Verdict:** hit → proposal · **Observed:** 2026-08-07

**Pattern.** **When you implement a surface that already exists elsewhere in the codebase, read the existing one's CODE COMMENTS, not just its shape — they name the bugs already paid for, and you are about to stand where that author stood.** Adding a `completeness_warning` to `symbols` search meant copying the convention from `references.rs`. Its rendering code carries a comment: *“BUG 2026-05-21: surface the completeness warning … in both the zero-refs and the normal branch.”* That is not commentary on `references.rs` — it describes the trap waiting in `symbols`: `format_search_symbols` returns `"0 matches"` from an early return **before** any warning or overflow rendering, so a warning appended at the end of the function would be invisible in the one case it exists for (an agent deciding whether a zero means *absent* or *not searched*). The convention's shape (`result["completeness_warning"]`) transfers on its own; the *lesson* transfers only if you read the comment. Proposal for Phase 1: when reusing an in-repo convention, grep its implementation for `BUG`/`bug` comments — the cheapest available list of what that surface has already cost someone.

**Evidence:** release-promotion W-10 / F-10 session; tests `format_search_symbols_surfaces_completeness_warning_on_zero_matches` + `format_search_symbols_leaves_a_trustworthy_zero_bare`; the mutation reverting the early-return warning kills the first and nothing else. Kin R-57 (read what it does, not what it intends)

**Status:** archived 2026-08-17 — compacted from the index row of `docs/trackers/reconnaissance-patterns.md`. No body section was ever written for this entry; this section exists so that citations of R-60 resolve (HY-9 / proposed D12).

## R-59 — Before building the harness a bug file asks for, read the FALLBACK GATE — the condition that

**Verdict:** hit → proposal · **Observed:** 2026-08-06

**Pattern.** **Before building the harness a bug file asks for, read the FALLBACK GATE — the condition that decides whether the suspected path is even reachable. It can eliminate the hypothesis the harness was designed to measure.** The `symbols` search flake was filed "root cause unconfirmed; needs a controlled, scripted reproduction (N parallel `symbols(name=X)` calls immediately post-activation)", and every signal pointed at LSP warming — including the source's own comment naming *"silent workspace/symbol on a still-indexing server"*, plus three genuinely silent degradation paths (budget-exceeded → `Ok(Vec::new())` with only a `tracing::warn!`; task error and join error both `continue`d). Reading one line killed it: the tree-sitter fallback is gated on `matches.is_empty()` — the aggregate of *already-filtered* matches — so it runs whenever the final answer would be zero, regardless of which languages degraded, and non-matching LSP results never populate `matches` and so cannot suppress it. Therefore **a 0-match implies tree-sitter also found nothing, and tree-sitter never touches the LSP**: server readiness cannot produce the symptom. The surviving hypothesis is a different bug class — both paths key off one `root` from `require_project_root_for` (`src/tools/symbol/symbols.rs:225`), and a root not yet settled post-activation makes the LSP query one tree while the walker walks the same wrong tree, both correctly returning nothing. Kin to the archived shared-server active-project race. Proposal for Phase 1: when a bug names a suspected mechanism, locate the fallback/guard that would mask it and read that condition **before** costing a reproduction — an unreachable hypothesis is cheaper to eliminate than to measure.

**Evidence:** release-promotion W-8; the harness as specified would have instrumented LSP readiness, i.e. the wrong variable; correct instrument is the resolved `root` per 0-match. Kin R-57, R-50 (the view is not the set)

**Status:** archived 2026-08-17 — compacted from the index row of `docs/trackers/reconnaissance-patterns.md`. No body section was ever written for this entry; this section exists so that citations of R-59 resolve (HY-9 / proposed D12).

