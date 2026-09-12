---
id: '8ace06843584dd1e'
kind: bug
status: taken
title: 'BUG: the tree-sitter fallback is gated on matches.is_empty(), so a warm LSP returns strictly fewer symbols than a cold one'
tags:
- cluster/selector-narrower-than-its-population
---

# BUG: the tree-sitter fallback is gated on `matches.is_empty()`, so a warm LSP returns strictly fewer symbols than a cold one

## Summary

`search_project_symbols` (`src/tools/symbol/symbols.rs`) serves project-scope `symbols`
searches from LSP `workspace/symbol` first, and falls back to a tree-sitter walk **only
when the LSP produced nothing at all** (`if matches.is_empty()`). The fallback's
granularity is the whole call — not the file, and not the language.

**Corrected 2026-09-12 after measuring both arms; this section previously scoped the
defect too narrowly.** It said the unreachable files were those "the LSP does not index",
which reads as a fixture-crate edge case. § Reproduction probe B shows it is wider on two
axes:

- **`matches` is not per-language**, so one hit in *any* language suppresses the fallback
  for *every* language. A single **Python** match made twelve Rust symbols unreachable,
  **eight of them under `src/`** — files rust-analyzer unambiguously owns. This is the
  common case, not an edge.
- **The emptiness the gate reads has two upstream sources, not one** — an LSP that
  returned nothing, and an LSP that timed out for one language (§ Root cause 3).

The observable consequence is inverted from what a reader expects: a **cold or broken**
rust-analyzer returns MORE results than a warm, working one — measured at `13` against
`1` on one unchanged tree — silently, with no marker on either answer.

## Symptom (Effect)

Measured 2026-09-12 on this checkout. `pub trait Tool` exists at
`tests/fixtures/edit-eval-rust/src/bug054_repro.rs:1`.

```
symbols(symbol="Tool")   -> 2 matches     (warm rust-analyzer)
symbols(symbol="Tool")   -> 3 matches     (same call, earlier, cold rust-analyzer)
```

Neither answer is marked. The 2-result answer is the one a caller gets under normal
operation, and it is the incomplete one.

## Reproduction

1. `symbols(symbol="Tool")` against this repo with rust-analyzer warm — 2 matches, and
   `tests/fixtures/edit-eval-rust/src/bug054_repro.rs` is absent.
2. Confirm the symbol is really there: `grep -rn "Tool" tests/fixtures/edit-eval-rust/src/`
   -> `bug054_repro.rs:1:pub trait Tool`.
3. Confirm the file is reachable by the walk — it is not gitignored, and
   `symbols(path="tests/fixtures/edit-eval-rust/src/bug054_repro.rs")` lists its `Tool`.

The cold-LSP arm was observed by a peer session before the warm arm, which is how the
discrepancy surfaced at all — the two readings were minutes apart on one unchanged tree.

**Probe B — the same query cold and warm, five minutes apart, one unchanged tree.** Run
2026-09-12 against `2ece66cf` on the release binary built 09:41:16, whose identity was
confirmed by inode (`executing == on-disk`, no `(deleted)` marker) BEFORE the run — this
repo has stale-binary readings on record and a probe against an unidentified build
measures nothing.

```
symbols(name="parse", exact=true)

  COLD — LSP not started, so matches.is_empty() held and the fallback ran
    -> 13 matches / 12 files
       tests/fixtures/edit-eval-rust/src/replace_generic.rs
       tests/fixtures/nav-eval-rust/src/shadowing.rs
       tests/fixtures/nav-eval-rust/src/generics.rs        (x2)
       scripts/probe_entry_attribution.py
       + 8 hits under src/

  <symbol_at("src/librarian/frontmatter.rs", line=80) — starts rust-analyzer>

  WARM
    -> 1 match / 1 file
       scripts/probe_entry_attribution.py
```

Reproduced twice at the warm end. No cold restart is needed to get both arms — warming
the LSP between two identical calls is enough, which makes this cheap to re-run.

**Two things probe B shows that probe A does not, and both widen the bug:**

- **The single surviving hit is PYTHON.** `matches` is not per-language, so one match
  from *any* language makes it non-empty and suppresses the fallback for *every*
  language. The twelve that vanished include **eight under `src/`** — files
  rust-analyzer is supposed to own. § Summary's framing (fixture-crate symbols go
  missing) understates it: cold `13` against warm `1` is most of the answer, not an
  edge case.
- **`PER_LANG_BUDGET` (`:620`) inherits the same defect.** Its comment reads *"the
  tree-sitter fallback below still runs if every language produces nothing"* — *every*
  is the operative word. A language whose `workspace/symbol` exceeds the 8 s budget
  yields an empty vec, so if Kotlin times out while Rust returns one hit, Kotlin is
  silently uncovered. The `tracing::warn!` fires where no agent reads it.

**Constraint this puts on any fix:** a candidate must not tree-sitter a root the LSP
already covers. `matches` is a flat `push` with no dedupe, so an overlapping gap set
emits DUPLICATE symbols rather than merely wasting time — a correctness regression, not
a cost one.

*Attribution, as apportioned by the parties themselves: the original cold/warm delta is
`b0b9bc40`'s observation; the `matches.is_empty()` root-cause read and one verified nested
manifest are `f3c594ce`'s, who filed this; the nested-root population, the controlled
inversion above and its cross-language scope are `b80a27d4`'s.*


**Probe B re-run against the fix — 2026-09-12, `920eb443` in the binary. The inversion is
NOT closed, and the fix is partial by a mechanism its own design did not anticipate.**

Binary identity confirmed before the run: pid `2194140`, executing inode equal to on-disk
(`189466740`), no `(deleted)` marker, built 10:46:06 and started 10:46:28, with
`920eb443` an ancestor of HEAD.

```
symbols(name="parse", exact=true)
                     BEFORE 920eb443      AFTER 920eb443
  COLD                 13 / 12 files       13 / 12 files     <- preserved exactly
  WARM                  1 /  1 file         5 /  4 files     <- improved, not fixed
```

The four recovered warm are exactly the nested-root fixture hits the fix targets:
`edit-eval-rust/replace_generic.rs`, `nav-eval-rust/generics.rs` (x2),
`nav-eval-rust/shadowing.rs`. So the mechanism shipped does what it was built to do.

**What it does not do, and this is the finding.** The eight `src/` hits cold mode returns
are still absent warm. They are not under any nested root, so they are not in the gap set
— the fix cannot reach them by construction. Stable across a second run `45` seconds
later, so it is not indexing lag.

**The design premise is false in practice.** § Fix option 4 rests on *"a directory
carrying its own manifest is the one the outer project does not build, therefore the
LSP's coverage is everything else"*. The converse does not hold: rust-analyzer was
**alive** during the warm run — it answered `symbol_at` on `src/librarian/frontmatter.rs`
with both a definition and a hover — and still returned nothing matching for these eight
from `workspace/symbol`. So LSP-covered is strictly SMALLER than not-in-a-nested-root,
and `nested_roots` is a lower bound on the gap rather than the gap itself.

Control, so this is not read as *"`src/` is unreachable"*:
`symbols(name="ArtifactBackend")` and `symbols(name="files_needing_fallback")` both return
their `src/` definitions. Those names match nothing else, so the LSP produced nothing at
all, `matches.is_empty()` held, and the cold arm covered the tree. **`src/` is reachable
exactly when no other language happens to match** — which is the original defect surviving
in a narrower form.

**Consequence for status: the bug stays open.** What shipped is the nested-root half. The
per-language half named in § Root cause 3 is untouched, and is now measured rather than
predicted — one Python hit still suppresses whole-tree coverage for Rust. A fix that
closes it has to make emptiness a **per-language** question, because `matches.is_empty()`
is computed across every language at once.

**Probe B re-run against BOTH fixes — 2026-09-12, `e14e5609`. Warm did not move, and the
reason is a third mechanism neither fix addresses.**

Binary identity: pid `4031602`, executing inode equal to on-disk (`190112719`), no
`(deleted)` marker, built 16:35:45, started 16:36:04, `e14e5609` an ancestor of HEAD.
**Coldness established by process absence** — `pgrep -c rust-analyzer` returned `0` before
the cold run — not inferred from the output.

```
symbols(name="parse", exact=true)
              pre-fix    after 920eb443    after e14e5609
  COLD          13            13                13        <- preserved throughout
  WARM           1             5                 5        <- unchanged by the second fix
```

Indexing lag is excluded: the warm figure was re-read with rust-analyzer at `4:40`
uptime and was still `5`.

**What the second fix NOT moving the number tells us**, which is worth more than the
number. `e14e5609` adds every file of an *unanswered* language to the gap. Warm stayed at
`5`, so Rust was classified **covered** — `lang_outcome` returned `Some`, meaning
rust-analyzer answered inside the budget and did not error. The per-language conflation
was real and is fixed, and it is **not** what loses these eight symbols.

**The measurement that locates the third mechanism.** The same query *without* `exact`
returns `25` matches over `16` files, and among them are `src/` **type** symbols that only
the LSP can have supplied — `Parsed` (`src/bin/sync_project.rs`), `ParseWarning`
(`src/librarian/tools/audit_doc_refs/mod.rs`), `SparseEntry` and `SparseVector`
(`src/retrieval/embedder.rs`). None of the eight `src/` items named exactly `parse` appears,
and **every one of those eight is a function**.

The split is therefore by SYMBOL KIND — not by file, not by directory, not by server
liveness — which is precisely why neither shipped fix reaches it.

**Hypothesis, explicitly NOT measured here.** rust-analyzer's `workspace/symbol` is
documented to search *types* for a bare query and to need a `#` suffix before it returns
functions; `client.workspace_symbols(&pattern)` passes `pattern_lower` raw. If that is the
mechanism, Rust **function** search has been served by the tree-sitter fallback all along,
and is therefore silently degraded on every query where any other language matches. That
is a larger claim than this file has evidence for, and confirming it needs the actual
`workspace/symbol` response, which no probe here reads. Recorded as a direction, not a
finding.

**A method correction, because it cost two earlier readings.** Several intermediate probes
today were interpreted with a "range format" tell — `Function 91-132` read as tree-sitter,
bare `Function 91` as LSP. **That tell is confounded by MATCH COUNT, not by arm:** a result
set small enough for the focus / auto-inline path renders without a range, a larger one
renders with it. It never identified the serving arm. The endpoint measurements above are
unaffected because coldness came from `pgrep`, but nobody re-deriving this should reach for
the format.

**Status stays open.** Two of three mechanisms are fixed and measured; the third is located
and unexplained.
## Environment

Linux, branch `experiments`, rust-analyzer 1.97.1. Not feature-gated.

## Root cause

Two independent narrowings compose, and only the second is documented at its site.

**1. The LSP's population is the cargo workspace, not the walk.**
`tests/fixtures/edit-eval-rust/` is its own cargo project (own `Cargo.toml` + `Cargo.lock`),
and the outer manifest declares `members = [".", "crates/codescout-embed"]`. It is not a
member, so `workspace/symbol` never indexes it. Meanwhile the accepted-files walk in the
same function **does** accept it (`ignore::WalkBuilder` + `ast::detect_language`), so the
file is inside the population the tool presents itself as searching.

**2. The fallback is all-or-nothing.** After the LSP arm, the tree-sitter walk runs under
`if matches.is_empty()`. That predicate asks "did we find anything?", never "did we search
everywhere?". One hit from an indexed crate therefore suppresses the only mechanism that
covers the unindexed ones.

**3. The gate has TWO upstream sources of emptiness, and only one is the LSP being
absent.** `PER_LANG_BUDGET` (`:620`) gives each language 8 s, and on timeout yields an
empty vec for that language with a `tracing::warn!`. Its comment reads *"the tree-sitter
fallback below still runs if every language produces nothing"* — ***every*** is the
operative word, and it is doing load-bearing work nobody reading quickly will notice. A
Kotlin timeout alongside one Rust hit produces a silent partial: Kotlin uncovered, the
fallback suppressed, and the only record in a log no agent reads. Same predicate, second
victim. (Found 2026-09-12 while measuring probe B; not previously filed.)

The `in_walk` filter directly above is the tell that the two populations were already
known to differ: it exists to drop LSP hits the walk did *not* accept (gitignored build
output). Nothing does the converse — admit walk-visible files the LSP did not return.


**The third mechanism, measured at the PROTOCOL on 2026-09-12 — and it is not a codescout
defect.**

Every earlier reading was confounded: a query the LSP answers with nothing falls through to
the tree-sitter arm, and the two arms' output is not distinguishable from outside. So this
was taken by driving rust-analyzer over stdio directly, with no codescout in the path
(`scripts/probe-ra-ws-symbol.py`, 90s index wait, toolchain `1.97.1`).

```
workspace/symbol "parse"     ->  11 symbols,   6 in-tree,  0 named exactly `parse`
                                 kinds: Package(4) x5, Enum(10) x1, Struct(23) x5
                                 NO functions at all
workspace/symbol "classify"  ->  73 symbols,  73 in-tree,  6 named exactly `classify`
                                 kinds: Module(2) x3, Function(12) x70
workspace/symbol "parse#"    -> 128 symbols, 128 in-tree,  all Function(12)
```

**rust-analyzer genuinely does not return the eight `parse` functions for a bare `parse`
query.** It returns six in-tree symbols, all of them types. The eight exist, are `pub fn`,
and are in the workspace. `classify` proves this is not a blanket "types only" rule —
that query returns functions, including the six named exactly `classify`. What separates
the two queries is not established here and is a rust-analyzer question, not a codescout
one.

**So the finding reframes the bug rather than extending it.** The tree-sitter pass is not
a fallback for files the LSP cannot index. It is **load-bearing for correctness on queries
the LSP answers INCOMPLETELY** — and `matches.is_empty()` suppresses it precisely when the
LSP returned *something*, which is exactly the case where that something may be a subset.
The predicate is not merely too coarse; it is anti-correlated with the need.

**This partially rehabilitates the rejected per-file union (§ Fix option 1).** It was
rejected because `lsp_seen` is query-scoped and so cannot decide coverage. That objection
stands. What has changed is the requirement: if an LSP answer can be an arbitrary subset of
the true answer, then **no** coverage predicate computed from the response can be correct,
and the only sound shape is to run tree-sitter over the accepted files regardless and
deduplicate against the LSP's results. That is the expensive option, and it is now the only
one not known to be wrong. Pricing it is the open work.

**Do NOT prescribe the `#` suffix as the fix without more work.** The `parse#` run returned
exactly `128` symbols — a cap — whose names do not contain the query at all
(`a_backslash_path_survives_the_delete_payload_round_trip`, `a_blank_imperative_is_refused`,
…). Whatever `#` does there, it is not "the same query, including functions", and shipping
it would trade a silent subset for a different silent subset.
## Evidence

- `src/tools/symbol/symbols.rs`, `search_project_symbols` — `if matches.is_empty()` gating
  the tree-sitter walk; `in_walk` handling only the opposite direction.
- `Cargo.toml` — `members = [".", "crates/codescout-embed"]`.
- `tests/fixtures/edit-eval-rust/Cargo.toml` — separate project.
- `tests/fixtures/edit-eval-rust/src/bug054_repro.rs:1` — the unreachable `pub trait Tool`.

### Four calls on one unchanged tree, all consistent with the gate and with nothing else

Contributed by the peer session that first hit the symptom (sessionId
`b0b9bc40-5358-4a44-b342-a2a71dc50fad`), against server pid 218699 confirmed to be the
current on-disk binary by walking its own process tree. Reproduced here because it is a
better evidence set than the one this file was opened with — rows 3 and 4 are positive
controls that the summary's two rows cannot supply:

| call | result |
|---|---|
| `symbols(symbol="Tool")` | 2 — fixture absent |
| `symbols(path=".../bug054_repro.rs")` | file overview shows `Interface 1-3 Tool` |
| `symbols(symbol="Tool", path="tests/fixtures/edit-eval-rust")` | **FINDS it** |
| `symbols(symbol="ReadMarkdown")` | **finds the fixture, project-wide, exact mode** |

Rows 3 and 4 are the load-bearing ones, and they are what make this a mechanism rather
than a story about a flaky index. Both produce an **empty LSP result** — row 3 because the
scope contains no workspace member, row 4 because no in-workspace symbol is named
`ReadMarkdown` — so `matches.is_empty()` fires and the fallback walks. The fixture is
found in exactly the cases where the LSP contributes nothing, and lost in exactly the
cases where it contributes something. Row 4 also independently kills the "exact mode is a
different backend" reading: exact mode reaches the fixture fine, when the LSP is empty.

**The failure is invisible precisely when the system is healthy.** The tool's coverage is
*inversely* related to how well the LSP is working — a warm, correct index silently
returns a subset of what a broken one returns. That is the property that makes this worth
fixing rather than documenting: every mechanism a reader would use to gain confidence in
the answer (LSP up, index warm, no errors logged) is a mechanism that makes the answer
smaller.

**A duplicate was filed on the symptom and withdrawn.**
`08d1ef6387a9e569` ("project-wide exact symbol lookup drops a colliding member", tagged
`cluster/capped-result-presented-as-complete`) was opened ~20 minutes before this file,
uncommitted, and deleted by its author in favour of this one — same observation, wrong
root-cause class. Recorded so a reader who saw it in a `find` before it went does not
spend time reconciling two records.
## Hypotheses tried

1. **Hypothesis:** exact and substring lookups are served by different backends, which
   would explain one finding the file and the other not. **Test:** read
   `search_project_symbols` — both modes go through one `name_ok` predicate on one LSP
   path. **Verdict:** rejected. The mode is not the variable; LSP warmth is.
2. **Hypothesis:** the fixture is gitignored or language-undetectable, so the walk never
   accepts it. **Test:** `symbols(path=...)` on the file returns its symbols, and it is
   not in `.gitignore`. **Verdict:** rejected — the walk accepts it; only the LSP does not.

## Fix

Not fixed, and this is a design decision rather than a cleanup, which is why it is filed
rather than patched in passing:

**— PARTIALLY SHIPPED 2026-09-12 as option 4 below.** Fix SHA: `920eb443`. Patch-id:
`11c0cd7f4926900d18e7e18f3a0273f0439224d4`. **It does not close the inversion** — the
post-fix re-run of § Reproduction probe B measures cold `13` against warm `5`, up from
warm `1`. Read that section before building on this one: option 4's premise (LSP coverage
is everything outside a nested root) is measured FALSE, so what shipped is a lower bound
on the gap. The options are kept as written, including the rejected one, because a reader
who meets only the outcome would reasonably retry the per-file union — it is the obvious
shape and its defect is not visible from the code.

- **Per-file union** — ~~run the tree-sitter walk over `accepted_files` the LSP did not
  return symbols *for*~~. **REJECTED 2026-09-12, at the bytes.**
  `LspClient::workspace_symbols(&self, query: &str)` (`src/lsp/client.rs:1024`) is
  **pattern-scoped**: it returns the symbols matching the query, never an inventory of
  what it indexed. So "files the LSP returned a symbol for" is a function of the QUERY,
  and a fully-indexed file containing no match is byte-for-byte indistinguishable from
  one the LSP never saw. On any selective search the gap set is nearly all of
  `accepted_files`, so this parses the corpus on every warm call.
  One sub-claim survives and is worth keeping, because the original costing was also
  wrong in the other direction: the *directory walk* is **not** an added cost. It is
  built unconditionally at `:583-608` to feed the `in_walk` filter, so it is paid today
  regardless. The cost was always `ast::extract_symbols()` per file, and the gap set is
  what decides how many — which is exactly what this option gets wrong.
  (Raised by `f3c594ce` against a draft of this shape by `b80a27d4`.)
- **Walk only the gap** — compute the set of accepted files belonging to no LSP-indexed
  project and tree-sitter those. Cheaper, needs a way to ask which roots the LSP covers,
  which `workspace/symbol` does not expose.
- **Declare the scope instead of widening it** — keep today's behaviour and have the
  response say which files were LSP-served vs walked, per
  `docs/adrs/2026-08-27-negative-results-name-their-scope.md`. Cheapest and honest, but
  leaves the result set LSP-state-dependent.

- **Detect nested project roots during the walk you already do** — a directory under
  `root` carrying its own manifest for the detected language (a `Cargo.toml` with a
  `[package]`, a `package.json`, …) that the outer manifest does not declare a member.
  Files under such a root are the gap, computable from the walk alone with no LSP
  cooperation, and bounded by how many nested projects exist rather than by the query —
  which is precisely where the per-file union fails.
  **Measured 2026-09-12: five such roots** —
  `tests/fixtures/{call_graph/rust, edit-eval-rust, lsp-mux/rust, nav-eval-rust,
  rust-library}` — holding **48 of 436** `.rs` files, ~11% of the corpus.
  `crates/codescout-embed` is a sixth nested manifest but IS a declared member, and
  excluding it is load-bearing rather than tidy: see § Reproduction's duplicate-symbol
  constraint.
  Honest limits, both named by its author: it is a **proxy** for "the LSP does not index
  this" rather than the thing itself, and it is language-aware in a way this function is
  not today. It fails safe — a mis-detected nested root costs extra parses, never a
  wrong answer.

  **Current candidate, with one revision.** An earlier draft of this bullet said to
  intersect the nested-root set with the files the LSP returned a symbol for.
  `f3c594ce` then pointed out that set is **query-scoped** — the same property that
  killed the per-file union above — so it can establish that a root IS covered and never
  that it is not. The asymmetry is real: a false "covered" costs a silently missing file,
  a false "uncovered" costs a visible duplicate.
  The answer is to **refuse the trade rather than pick a side.** Decide coverage
  *structurally*, from manifest membership, which is query-independent; prevent
  duplicates *mechanically*, by deduping at the push site on `(file, name, line)`.
  Coverage then never rests on a query-scoped signal, and a duplicate cannot survive even
  if one is produced — `lsp_seen` drops out of the design entirely.
  (Proposed by `f3c594ce`, who also raised the asymmetry; root count and this resolution
  by `b80a27d4`.)

Note the third option alone does not remove the inversion (cold > warm); it only stops it
being silent.

## Tests added

Six, all in `src/tools/symbol/symbols.rs`'s `fallback_gap_tests`, over
`files_needing_fallback` and `is_project_manifest`.

- `everything_is_covered_when_the_lsp_produced_nothing` — the pre-fix arm, which must
  keep behaving identically. A fix that narrowed this case would turn a working cold
  path into a broken one.
- `only_nested_roots_are_covered_when_the_lsp_answered` — the fix itself. `nested_roots`
  being non-empty is the load-bearing detail: pass `&[]` and it silently becomes a test
  that the warm arm returns nothing, which a deleted fallback also satisfies.
- `a_file_the_lsp_already_answered_for_is_not_reparsed` — the duplicate guard for a
  partially covered root.
- `lsp_seen_does_not_shrink_the_cold_arm` — the pair that makes the cold branch a real
  branch rather than an unreachable one.
- `the_gap_is_sorted_so_a_capped_search_is_deterministic` — asserted against an
  independently sorted expectation, not `is_sorted()`, which a one-element result and a
  luckily-ordered `HashSet` both satisfy.
- `manifests_are_recognised_and_source_files_are_not` — carries its own negative half,
  without which `fn is_project_manifest(_) -> bool { true }` passes and every directory
  becomes a nested root.

**Mutation-tested at three SITES rather than once for the feature**, per § *Testing
Discipline*. Each killed only its own test, which is what makes them three measurements
rather than one: removing `.sort()` killed the determinism test alone (and printed a
genuinely scrambled `HashSet` order, so it is not passing by luck); removing the
`lsp_seen` filter killed the reparse test alone; disabling the cold arm killed the three
tests that depend on it.

**Not yet done, and it is the measurement this file turns on:** re-running § Reproduction
probe B against a binary containing the fix, to confirm cold `13` / warm `1` becomes
`13` / `13`. That needs `cargo rb` + `/mcp`; until then the fix is verified by unit test
and not on the wire.
## Workarounds

Scope the call to the file or directory (`symbols(path=...)`), which takes the
path-restricted branch and does not consult `workspace/symbol` at all. Do not treat a
project-wide `symbols` count over a multi-cargo-project tree as complete.

## Resume

**Claimed 2026-09-12 by `b80a27d4`**, after confirming with its filer that it was free —
`open`, never claimed, filed-and-parked deliberately. Asked rather than inferred from the
fact that they moved to another bug.

**Unblocked 2026-09-12.** `b0b9bc40`'s `group_by_file_ranked` refactor landed cleanly —
`cargo build --release --bin codescout --features local-embed` succeeds. Verified by
building, not by asking.

**Probe B re-run against the rebuilt binary (release build 19:26, pid 1000730, exe
inode confirmed live, no `(deleted)` marker), same tree, same query:**

```
symbols(name="parse", exact=true)

  COLD  -> 13 matches / 12 files   (unchanged from the original probe B)
  WARM  -> 5 matches / 4 files    (the currently-landed fix: was 1/1 before it)
```

**This is the currently-open fix's own measurement, not a new bug.** The 5 that
survive warm are exactly the four fixture files under `tests/fixtures/*-eval-rust/`
— nested roots outside the LSP's own cargo workspace, which `files_needing_fallback`
correctly re-parses. The 8 that are still missing are exactly the `src/` hits from
the cold list, including `frontmatter.rs:80`'s `parse` — the function this very probe
just warmed via `symbol_at` moments earlier, and it still does not come back from
`workspace/symbol`. Those files ARE inside the LSP's own workspace, so the
manifest-based coverage predicate correctly calls them "covered" and skips
re-parsing them — and that predicate cannot be patched to fix this, because the gap
isn't coverage, it's rust-analyzer's own `workspace/symbol` answer being incomplete
for queries like a bare `parse` (the third mechanism, measured at the protocol
earlier). Coverage-based fallback structurally cannot reach this remainder.

**Next action, revised by this measurement.** Option 4 (structural coverage +
mechanical dedupe) is confirmed correct for what it targets and confirmed
insufficient alone — it cannot close the `src/` gap because that gap is inside its
own definition of "covered." Closing it needs the root-cause shape already on
record: run tree-sitter over the accepted files regardless of LSP coverage and
dedupe against whatever the LSP returned, rather than deciding not to run it. Pricing
that — the always-run-and-dedupe cost, not the nested-roots-only fix already
landed — is the open work, and it changes the scope of this bug from "the fallback
under-triggers" to "the fallback's own trigger condition cannot see the failure
mode it exists for."

One thing NOT owed any more: the inversion is now an observed measurement rather than an
inference — § Reproduction probe B, both arms, reproduced twice (2026-09-12 pre-fix,
2026-09-12 post-fix).
## References

- `src/tools/symbol/symbols.rs` — `search_project_symbols`
- `docs/adrs/2026-08-27-negative-results-name-their-scope.md`
- Surfaced 2026-09-12 by a peer session's measurement during unrelated `symbols` work; the
  mechanism was read out of the source and confirmed against the manifests here.
