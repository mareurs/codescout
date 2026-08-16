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

