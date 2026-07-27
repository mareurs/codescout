---
id: '4232733980fe92e9'
kind: tracker
status: active
title: Dependency review session log
tags:
- dependencies
- build-leanness
- session-log
---

# Dependency review session log

> Work-stream log for the dependency / build-leanness review started
> 2026-07-25 (`git2` investigation → full manifest audit). Frictions (F-N)
> and wins (W-N) from sessions touching dependency graph, feature gating,
> and build cost.
>
> Category conventions and the full Status vocabulary are pinned in
> [`docs/templates/session-log.md`](../templates/session-log.md).

---

## Index

| ID | Date | Severity | Category | Status | Title |
|----|------|---------:|----------|--------|-------|
| F-1 | 2026-07-25 | med | codescout-tool | fixed-verified | Inferred `#[cfg]` gating from grep line-adjacency; modules were ungated |
| F-2 | 2026-07-25 | med | rust-cargo | fixed-verified | `cargo tree -p X --no-default-features` retargets the flag to X, invalidating subtree closures |
| F-3 | 2026-07-25 | high | plan-drift | fixed-verified | Task 1.3 gates `pub mod client;` — 14 ungated consumers, and it excludes Task 1.0's invariant test from every build config |
| F-4 | 2026-07-25 | med | plan-drift | fixed-verified | "the four OpenAI wire structs (`:92-117`)" is five structs; two belong to the retained sparse leg |
| F-5 | 2026-07-25 | med | plan-drift | fixed-verified | Task 1.4's `any(server-stack, remote-embed)` body gate contradicts Task 1.5's `dep:rustls` placement |

## Wins Index

| ID | Date | Impact | Pattern | Counterfactual | Status |
|----|------|-------:|---------|----------------|--------|
| W-1 | 2026-07-25 | med | A/B the real manifest (patch → measure → restore) for any dep-cost claim | Review would have shipped "one-line change saves 48 crates" — the one-line version does not compile | validated |
| W-2 | 2026-07-25 | high | Re-scout a plan's cited shapes in the NEW session, and enumerate consumers for every proposed `#[cfg]` gate | Stage 1 would have dispatched with a wall of E0433 across 12 files and a load-bearing invariant "pinned" by a test that compiles in zero configurations | validated |

---

## F-1 — Inferred `#[cfg]` gating from grep line-adjacency; modules were ungated

**Observed:** 2026-07-25, dependency leanness review (`git2` question → full manifest audit).

**When:** Tracing which files use `reqwest`, to decide whether the root crate's
unconditional `reqwest` dependency could be made `optional`.

**Expected:** `grep(pattern="^(mod|pub mod) (embedder|reranker)|cfg\\(feature", path="src/retrieval/mod.rs")`
returned four lines — `1: #[cfg(feature = "server-stack")]`, `7: pub mod embedder;`,
`12: #[cfg(feature = "server-stack")]`, `14: pub mod reranker;`. I read this as
"both modules are `server-stack`-gated" and stated it to the user as fact.

**Got:** Reading `src/retrieval/mod.rs:1-17` in full shows an alphabetized `pub mod`
list. Line 1's `#[cfg]` binds to line 2 `pub mod artifact;`; line 12's binds to line 13
`pub mod qdrant;`. `embedder` (line 7) and `reranker` (line 14) are **ungated**.
Caught by `cargo check --no-default-features`: 16 errors, all `E0433 unresolved module
reqwest`, in exactly those two files.

**Probable cause:** `grep` prints matching lines with line numbers but elides the gap
between them. `#[cfg]` is a *positional* attribute binding to the next item, which grep
structurally cannot show. Filtering for two patterns at once made the interleaving look
like pairing.

**Workaround:** Read the file. Corrected the claim to the user mid-review, before it
reached the recommendation.

**Severity:** med — the recommendation would have shipped as "make `reqwest` optional,
one-line manifest change." Actual work is a module split: `embedder.rs` mixes
reqwest-dependent types (`EmbedderHttp`, `HttpDenseEmbedder`) with the reqwest-free
`EmbedOutput` / `SparseVector` / `BatchEmbedder` / `DenseEmbedder` surface that six other
modules import. The "one-line" version does not compile.

**Status:** fixed-verified — corrected in-session; verified by `cargo check
--no-default-features` failing exactly as predicted; tree restored clean
(`git status --porcelain` empty).

**Fix idea / Pointer:** R-43 (`docs/trackers/reconnaissance-patterns.md`). Rule: an
attribute-binding claim needs the file region read, never a grep hit.

---

## F-2 — `cargo tree -p X --no-default-features` retargets the flag to X, invalidating subtree closures

**Observed:** 2026-07-25, same session.

**When:** Computing which crates in the lean build are reachable *only* through the root
crate's `reqwest` edge, by unioning every other direct dep's closure and set-differencing
against the full lean tree.

**Expected:** `cargo tree --no-default-features --prefix none -p <dep>` returns `<dep>`'s
closure as resolved under the *root's* lean feature set.

**Got:** The flag applies to the **selected package** `<dep>`, not the root. Closures came
back degenerate, so the union was near-empty and the set difference reported 239 crates as
"reqwest-only" — including `clap`, `git2`, `tree-sitter-*`, `rusqlite`, and the root
`codescout` package itself.

**Probable cause:** `-p` changes which package `--no-default-features` targets. Cargo emits
no warning.

**Workaround:** Discarded the computation, said so explicitly to the user, and replaced it
with a direct A/B experiment (see W-1).

**Severity:** med — this particular output was self-evidently wrong (`git2` is not reachable
through `reqwest`), so inspection caught it. A subtler overlap would have passed as
plausible.

**Status:** fixed-verified.

**Fix idea / Pointer:** For dependency-graph deltas, A/B the real manifest and diff the
resolved trees. Do not compose per-package closures.

---

## W-1 — A/B manifest experiment caught two wrong claims before they reached the user

**Observed:** 2026-07-25, dependency leanness review.

**Pattern:** To claim "removing dep X saves N crates," patch the **real** manifest, run
`cargo tree` for each feature set, `comm -23` the sorted outputs, and restore from a
pre-taken backup (`cp Cargo.toml Cargo.lock $SCRATCH/`) with `git status --porcelain` as the
exit gate. Add `cargo check` to the loop when the claim implies the gated code compiles
without the dep. Do not reason about savings from `cargo tree -p X`.

**Counterfactual:** Both F-1 and F-2 were caught by this loop, not by review.
- Without it, the review would have asserted "gating `reqwest` behind `server-stack` is a
  one-line manifest change saving 48 crates." The loop's `cargo check --no-default-features`
  step returned 16 errors in `embedder.rs` / `reranker.rs`, proving the modules were ungated
  (F-1) and that the real change is a module split.
- The loop's first attempt (composed per-package closures) produced 239 "reqwest-only"
  crates including `git2` and `clap` — visibly absurd, discarded (F-2). The A/B replacement
  produced the defensible 274 → 226.
- It also produced the `jsonschema` figure (default 339 → 312) that no amount of reading
  `cargo tree -p jsonschema` would yield — that subtree is 103 crates, ~70 of them shared
  with the rest of the graph.

**Confirming data points:**
1. F-1 caught by the loop's compile step, not by re-reading the grep.
2. F-2 caught by inspecting the A/B alternative against the composed-closure result.
3. The feature-delta table's `remote-embed = +1 crate` row — an implausibly cheap number
   that is itself the fingerprint of the mis-gating, and only visible because features were
   measured pairwise rather than read off the manifest.

**Impact:** med — prevented one wrong recommendation and one nonsense figure in a review the
user asked for as decision input.

**Promote-when:** A second session uses patch→measure→restore for a build-config claim and it
catches a wrong assertion. At 2 datapoints, promote to CLAUDE.md as: "dependency and
feature-cost claims are measured by A/B-ing the manifest, never read off `cargo tree -p`."

**Status:** validated.

---

## F-3 — Task 1.3 gates `pub mod client;`, which has 14 ungated consumers and hosts Task 1.0's test

**Observed:** 2026-07-25, pre-dispatch reconnaissance for
`docs/plans/2026-07-25-embedding-transport-consolidation.md` Stage 1. New
session; the plan's line citations were verified in a prior session, not this one.

**When:** Before dispatching any Stage 1 task. Nothing had been edited yet.

**Expected (plan).** Task 1.3: *"Gate `pub mod reranker;`
(`src/retrieval/mod.rs:14`) and `pub mod client;` on `server-stack`. Verify
`RerankerHttp` has no non-gated caller."*

**Got (scouted reality).** Three independent contradictions:

1. **`pub mod client;` has ~14 ungated consumers.** `src/retrieval/mod.rs:3`
   declares it ungated, and `RetrievalClient::from_env()` is called from
   `src/retrieval/search.rs:3` (bare `use` at line 3, no cfg; `pub mod search;`
   is itself ungated at `mod.rs:15`), `src/retrieval/sync.rs:195` (an ungated
   inherent `impl crate::retrieval::client::RetrievalClient` in ungated
   `mod.rs:17`), `src/tools/semantic/index.rs:124,314,410`,
   `src/tools/semantic/semantic_search.rs:223`, `src/tools/memory/mod.rs:402`,
   `src/tools/config/mod.rs:330,425`, `src/tools/onboarding.rs:744`,
   `src/agent/mod.rs:1581,1742`, `src/main.rs:269,301`,
   `src/dashboard/api/index.rs:14`. Gating the module is a subtree delete, so
   `--no-default-features` breaks at every one of those sites. That is not a
   module split; it is cfg-threading through six tool modules.

2. **It contradicts the plan's own load-bearing invariant.** The invariant
   (plan §"The load-bearing invariant") is
   `!cfg(server-stack) ⟹ lite == true ⟹ dense_only == true`, and its proof
   chain steps 1–4 all require `from_env` to *exist and be reachable* under
   `not(server-stack)` — `client.rs:65-71`'s
   `#[cfg(not(feature = "server-stack"))] qdrant_code_store` bail is the first
   link. Gate `pub mod client;` and that entire chain is compiled out; the
   invariant becomes vacuous rather than proven.

3. **It excludes Task 1.0's test from every build configuration.** Task 1.0
   puts the invariant test *in `src/retrieval/client.rs`*, asserting behaviour
   under `#[cfg(not(feature = "server-stack"))]`. If `pub mod client;` is gated
   on `server-stack`, then under `not(server-stack)` the file is not compiled
   (no test), and under `server-stack` the `cfg(not(server-stack))` test is
   excluded (no test). The test compiles in **zero** configurations while the
   plan records it as what "pins this invariant before anything depends on it."

**Also:** the task's own verify step fails as written. `client.rs:6` is a bare
`use crate::retrieval::reranker::RerankerHttp;` with no cfg, and
`RetrievalClient.reranker` (`client.rs:17`) is a **non-optional field** — so
`RerankerHttp` does have a non-gated caller, which is presumably why the plan
paired the two gates in the first place.

**Probable cause.** The pairing is load-bearing for compilation (gating
`reranker` alone breaks `client.rs:6`/`:17`), so the plan reached for the
smallest change that compiles the *`server-stack`* build and did not re-check
the *lean* build's consumer set. R-43 (same work stream) is the read-side twin
of this: gating claims need the region read. This is the write-side —
gating *proposals* need the consumer set enumerated.

**Workaround / fix idea.** Invert the gate: keep `pub mod client;` **ungated**,
gate `pub mod reranker;` on `server-stack`, and make `RetrievalClient`'s
`reranker` field + the `client.rs:6` `use` cfg-conditional (the field is only
ever reached through `search_in`, which already short-circuits on `lite` at
`search.rs:84`). That preserves the invariant proof, keeps Task 1.0's test
compilable in the lean build it guards, and leaves all 14 consumers untouched.
Decision belongs to the plan owner — option (b), gating `client` and
cfg-threading 14 call sites, changes lean-build runtime behaviour
(`semantic_search` would go silently unavailable) and is a different plan.

**Severity:** high — would have cascaded. A subagent handed Task 1.3 hits
E0433 across 12 files, then either flails adding cfg gates through
`src/tools/**` (changing lean-build behaviour) or reverts. Worse, Task 1.0
ships first and lands a dead test, so the plan's declared load-bearing
invariant would be recorded as pinned while guarded by nothing.

**Status:** fixed-verified — plan edit landed 2026-07-25, before any subagent
ran. Task 1.3 now gates `reranker` only and carries an explicit
**"Do NOT gate `pub mod client;`"** block enumerating all three failure modes;
Task 1.0's test placement in `client.rs` is unchanged and now compiles in the
lean build it guards. Applied on the recommendation in this entry's
*Workaround / fix idea* rather than a separate plan-owner ruling — the
alternative (gate `client`, cfg-thread 14 call sites) is recorded in the plan as
rejected, so reversing is a plan edit, not an archaeology exercise. No code
touched.

**Fix idea / Pointer:** plan Stage 1 tasks 1.0 + 1.3; kin R-43, R-44, F-1.


## F-4 — "the four OpenAI wire structs (`:92-117`)" is five structs; two serve the retained sparse leg

**Observed:** 2026-07-25, same pre-dispatch scout as F-3.

**When:** Reading Stage 1.2 and Stage 3.1, both of which address the wire
structs by the range `:92-117` and the count "four".

**Expected (plan).** Stage 1.2 moves "the four OpenAI wire structs
(`:92-117`)" into `http.rs`; Stage 3.1 **deletes** "the four OpenAI wire
structs (`:92-117`)".

**Got (scouted reality).** `src/retrieval/embedder.rs` holds **five** structs in
that range, and only three are OpenAI/dense-side:

| struct | lines | consumers | side |
|---|---|---|---|
| `EmbedReq { inputs }` | 92-94 | `embed:277` | **sparse** (TEI-shaped `inputs`) |
| `OpenAiEmbedReq { input, model }` | 97-100 | `dense_batch:202` | dense |
| `OpenAiEmbedResp` | 103-105 | `dense_batch:210` | dense |
| `OpenAiEmbedItem` | 108-111 | via `OpenAiEmbedResp:104` | dense |
| `SparseEntry { index, value }` | 114-117 | `embed:293,303`; `embed_batch:372,386,412` | **sparse** |

**Impact is stage-dependent.** Stage 1.2 (move the whole range into
`server-stack`-gated `http.rs`) is *fine* — `EmbedderHttp::embed` /
`embed_batch`, the sparse consumers, move with them. Stage 3.1 (**delete** the
range) is wrong for two of five: Stage 2 replaces only `dense_batch` /
`dense_query` internals with `RemoteEmbedder`, and lifting the sparse leg into
the crate is explicitly **out of scope**, so `embed`/`embed_batch` still
serialize `EmbedReq` at one site and deserialize `Vec<Vec<SparseEntry>>` at
four.

**Probable cause.** The count and the line range were derived from the
OpenAi-prefixed names and a contiguous block, respectively; the block is
contiguous in the file but not in ownership. Naming hides it — `EmbedReq` is
the only sparse struct without a `Sparse` prefix.

**Workaround / fix idea.** In Stage 3.1, name the three dense structs
explicitly (`OpenAiEmbedReq` `:97-100`, `OpenAiEmbedResp` `:103-105`,
`OpenAiEmbedItem` `:108-111`) instead of citing `:92-117`, and state that
`EmbedReq` + `SparseEntry` stay with the sparse leg. Cite names, not ranges,
for any deletion.

**Severity:** med — a compile error, so the compiler catches it (R-5), but a
subagent handed "delete `:92-117`" burns a retry cycle. The bad branch is a
subagent "fixing" the resulting E0412 by deleting the sparse leg too, which is
silent feature loss in a stage whose gate does not measure sparse behaviour.

**Status:** fixed-verified — plan edit landed 2026-07-25. Stage 1.2 now says
"five wire structs" and names the two sparse-side ones; Stage 3.1 names its three
deletion targets (`OpenAiEmbedReq`, `OpenAiEmbedResp`, `OpenAiEmbedItem`)
explicitly and states that `EmbedReq` + `SparseEntry` stay, with the rule
"cite deletion targets by name, never by line range".

**Fix idea / Pointer:** plan Stage 1.2 + Stage 3.1.


## F-5 — Task 1.4's crypto-provider gate contradicts Task 1.5's `dep:rustls` placement

**Observed:** 2026-07-25, same pre-dispatch scout as F-3.

**When:** Reading Stage 1.4 and 1.5 together, then checking the manifest.

**Expected (plan).** 1.4: gate `src/lib.rs:10 install_default_crypto_provider`
*body* on `any(feature = "server-stack", feature = "remote-embed")`, leaving
callers (`agent/mod.rs:383`, `main.rs:226`) unchanged so it degrades to a
no-op. 1.5: make `reqwest` / `rustls` optional and add `dep:reqwest`,
`dep:rustls` to the **`server-stack`** feature.

**Got (scouted reality).** Two problems.

1. **The gate and the dep placement disagree.** `rustls::` appears exactly once
   in root — `src/lib.rs:14`,
   `rustls::crypto::ring::default_provider().install_default()`, inside that
   very function. So under `--no-default-features --features remote-embed`
   (no `server-stack`), 1.4 compiles the body because `remote-embed` satisfies
   the `any(...)`, while 1.5 has not linked `rustls` — unresolved crate. That
   configuration is exercised by the plan's own verification script
   (`count "--features remote-embed"`) and is the Stage 3 gate's headline number,
   though `cargo tree` will not surface it — only `cargo check` will.
   The `remote-embed` disjunct is also unnecessary: under `remote-embed` root
   delegates to `codescout_embed::RemoteEmbedder`, which installs its **own**
   provider at `crates/codescout-embed/src/remote.rs:84,96`. Gate on
   `feature = "server-stack"` alone.

2. **The caller inventory is 4, not 2 (or the "three" of Stage 3.2).**
   `src/agent/mod.rs:383` and `src/main.rs:226` are named; unnamed are
   `src/retrieval/reranker.rs:79` (in `RerankerHttp::new`) and
   `src/retrieval/embedder.rs:168` (in `EmbedderHttp::with_config`). Both
   unnamed sites sit inside code Stage 1.2 / 1.3 relocates or gates, so they
   need path fixups during the move regardless — and Stage 3.2's "delete … and
   its three call sites" would leave one behind.

**Confirming the 1.5 premise (this part holds).** `reqwest::` appears in root
only in `src/retrieval/embedder.rs` (2) and `src/retrieval/reranker.rs` (2),
both `server-stack`-gated after 1.2/1.3 — so `dep:reqwest` under `server-stack`
is sufficient. `Cargo.toml:89` (`reqwest`) and `:92` (`rustls`) are indeed
non-optional today, and `server-stack = ["dep:qdrant-client"]` is at `:194`.

**Probable cause.** 1.4 and 1.5 were written as separate tasks and reason about
different feature axes — 1.4 about *who needs TLS*, 1.5 about *who pays for
reqwest* — and the two axes were never intersected against the actual
`rustls::` usage set (one line).

**Workaround / fix idea.** Change 1.4's gate to `feature = "server-stack"`;
correct the caller list to four sites; correct Stage 3.2's "three call sites"
to four.

**Severity:** med — compile error in a real, plan-exercised feature
combination; one-word fix, but split across two tasks a subagent would execute
independently, so neither task's gate necessarily surfaces it.

**Status:** fixed-verified — plan edit landed 2026-07-25. Task 1.4's gate is now
`feature = "server-stack"` alone with the `remote-embed` disjunct removed and
the reason recorded; the caller inventory in both 1.4 and 3.2 is corrected from
two/three to the actual **four** sites.

**Fix idea / Pointer:** plan Stage 1.4, 1.5, 3.2.


## W-2 — Re-scouting a prior-session-verified plan caught three defects, one high, before dispatch

**Observed:** 2026-07-25, pre-dispatch reconnaissance on Stage 1 of
`docs/plans/2026-07-25-embedding-transport-consolidation.md`. New session; the
plan carried "verified 2026-07-25 … each link read this session" from an
earlier session in the same day.

**Pattern.** Two rules, both earned here:

1. **A plan's "verified this session" is session-scoped and does not transfer.**
   Re-scout on the first dispatch of a new session even when the plan is hours
   old and explicitly claims verification. The claim is evidence about a
   session you are no longer in.
2. **For any proposed `#[cfg]` gate on a module declaration, enumerate the
   consumer set before accepting the task.** `grep <mod>::|use .*<mod>` with
   `context_lines=2` over the workspace root, and check whether the
   configuration being gated *out* is the one the plan's tests or invariants
   live in. Gating a `pub mod` is a subtree delete; its blast radius is the
   transitive import set, and the declaration site cannot show it.

**Counterfactual.** Without this scout, Stage 1 dispatches in task order:

- **Task 1.0 lands a dead test.** It writes a `#[cfg(not(feature =
  "server-stack"))]` invariant test into `src/retrieval/client.rs`; Task 1.3
  then gates `pub mod client;` on `server-stack`, so the test compiles in zero
  configurations. `cargo test` stays green — nothing fails — and the plan's
  declared load-bearing invariant is recorded as "pinned by a test" while
  guarded by nothing. This is the expensive outcome: silent, green, and wrong.
- **Task 1.3 wall-of-errors.** E0433 at 14 `RetrievalClient` sites across
  `src/retrieval/search.rs`, `src/retrieval/sync.rs`,
  `src/tools/semantic/{index,semantic_search}.rs`, `src/tools/memory/mod.rs`,
  `src/tools/config/mod.rs`, `src/tools/onboarding.rs`, `src/agent/mod.rs`,
  `src/main.rs`, `src/dashboard/api/index.rs`. Best case one retry; realistic
  case the subagent cfg-threads `src/tools/**` and silently removes
  `semantic_search` from lean builds.
- **Task 1.4 + 1.5** produce an unresolved-`rustls` build under
  `--features remote-embed`, found only at the Stage 3 gate, two stages later.
- **Stage 3.1** deletes `EmbedReq` + `SparseEntry` and breaks the retained
  sparse leg at five sites.

Scout cost: 8 tool calls, no edits. Estimated avoided: ≥3 subagent retries, one
silent lean-build behaviour change, one dead invariant test merged green.

**Confirming data points:**
1. W-1 (this session log) — A/B'ing the real manifest caught two wrong
   dep-cost claims before they reached the user.
2. F-3 / F-4 / F-5 (this entry's scout) — three plan defects, one high, all
   caught pre-dispatch with zero code written.
3. R-43 — the read-side twin: a gating claim made from a grep hit was wrong,
   and only `cargo check` caught it.

**Impact:** high — the Task 1.0 dead-test outcome is not caught by any gate in
the plan, so this scout is the only thing standing between the invariant and a
false guarantee.

**Promote-when:** **not yet — one datapoint each, do not promote.** An earlier
draft of this entry claimed three datapoints for rule 2 by counting F-1, R-43,
and F-3 together; that is wrong. F-1 / R-43 are the *read* side (inferring
gating that already exists from a grep hit) and F-3 / R-44 are the *write* side
(proposing new gating without enumerating consumers). Same family, different
rule — so the consumer-set imperative stands at **one** datapoint, and rule 1
(session-scoped verification claims do not transfer) also at one. Promote rule 2
to the `reconnaissance` codescout memory when a second consumer-set case lands
(hit or miss, any work stream); promote rule 1 when a second
stale-verification-claim case lands outside this work stream. Until then the
R-44 ledger entry is the only record, which is the correct weight for n=1.

**Status:** validated — three defects captured pre-dispatch, no subagent ran.
## Template for new entries

<!-- Insert new F-N / W-N entries above this line via:
     artifact(action="update", id=<this id>, patch={body_edits: [{
         heading: "## Template for new entries",
         action: "insert_before",
         content: "## F-N — title\n..."}]})
     Also update the matching Index / Wins Index table row at the top.
     Templates + Status vocabulary: docs/templates/session-log.md -->
