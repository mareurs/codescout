---
id: 2d8806456e6e1ba3
kind: bug
status: fixed
title: a narrowed link_scan marks correct cites edges stale — the prune is bounded by a citation's source while resolution is bounded by both ends
owners:
- marius
tags:
- cluster/selector-narrower-than-its-population
closed: 2026-09-24
opened: 2026-09-21
owner: marius
related: []
severity: high
unverified: 'CLEARED 2026-09-24 on the rebuilt binary — `link_scan(limit=10, write=false)` reports `edges_stale = 0`, was 53; the false-dangling half is filed as 82b69559ddb18983. Was — The live reproduction has not been re-run against a rebuilt binary: after `cargo rb` + `/mcp`, `librarian(action="link_scan", scope="project", write=false, limit=10)` should report edges_stale at or near the pairs with both ends in the window, not 53. The predicate and its call-site wiring are pinned by tests; the live number is not observed.'
---

> **Do not run `librarian(action="link_scan", write=true)` with a narrowed scan on this
> checkout while this is open.** Every figure below came from `write=false`. The destructive
> form was never executed, deliberately — see § *Symptom* on what that means for the claim.

# BUG: a narrowed link_scan marks correct `cites` edges stale — the prune is bounded by a citation's SOURCE while resolution is bounded by BOTH ends

## Summary

`librarian(action="link_scan")` applies two different bounds to one delta. The prune
predicate protects an edge whose **source** was not scanned; the `desired` set it is
diffed against is built from a citation resolver whose corpus is **only** the scanned
artifacts. A citation whose *destination* falls outside the scan window therefore
contributes no `desired` pair while its *source* sits in the prunable set — so a correct,
prose-supported edge is classified stale and, under `write=true`, deleted. Narrowing the
scan (`limit`, or a tighter `scope`) makes this **worse**, not safer: the smaller the
window, the more destinations fall outside it.

## Symptom (Effect)

Two report-only runs against the same catalog, minutes apart, disagree about which edges
should be deleted — and the *narrower* one wants to delete more.

| run | `artifacts_scanned` | `scan_truncated` | `edges_desired` | `edges_unchanged` | `edges_stale` |
|---|---|---|---|---|---|
| `limit=10` | 10 | `true` | 10 | **0** | **18** |
| default `limit` | 1770 | `false` | 2930 | **2736** | **2** |

The capped run's 18 stale pairs all carry `"dst": null` — the reporter cannot name the
destination artifact, because it is not in the scanned corpus. Enumerated, not totalled
(source → destination, with the destination's path resolved from the catalog afterwards):

From `docs/trackers/issue-clusters/IC-24-value-correct-in-a-frame-its-name-does-not-state.md`
(`3f62fb253da43649`) — six:

- `2dd9d90bc83f9f49` → `docs/trackers/bug-fix-session-log.md`
- `3cbaf75df04686dd` → `docs/trackers/embedder-stack-ops-session-log.md`
- `48671109181877b9` → `docs/trackers/issue-clusters/IC-18-selector-narrower-than-its-population.md`
- `58e59d66022a8b14` → `docs/trackers/issue-clusters/IC-20-floor-published-under-the-name-of-a-total.md`
- `59ebeebb6ed05c89` → `docs/trackers/prompt-hamsa-audit-log.md`
- `d9d291b44775e50d` → `docs/issues/2026-09-06-a-push-publishes-commits-their-author-was-withholding.md`

From `docs/trackers/issue-clusters/IC-6-addressing-without-an-escape-hatch.md`
(`bc284a781fc9f5f2`) — twelve:

- `02ddb62d44b9ba34` → `docs/trackers/issue-clusters/IC-14-guard-narrower-than-its-name.md`
- `194c5945591dfc7e` → `docs/issues/archive/2026-09-12-every-ref-in-a-fenced-block-is-attributed-to-the-blocks-first-line.md`
- `2a70a91ae2dd3f69` → `docs/trackers/issue-clusters/IC-21-instrument-omits-the-dimension-that-grows.md`
- `2e49fd615738a623` → `docs/trackers/issue-clusters/IC-23-attribute-derived-at-container-granularity.md`
- `391d93763bd1d9fd` → `docs/trackers/issue-clusters/IC-2-gate-keyed-on-unobservable-event.md`
- `3922c2a0fd0dfcfc` → `docs/trackers/observer-blindness.md`
- `48671109181877b9` → `docs/trackers/issue-clusters/IC-18-selector-narrower-than-its-population.md`
- `5696563f06b2c222` → `docs/trackers/reconnaissance-patterns.md`
- `58e59d66022a8b14` → `docs/trackers/issue-clusters/IC-20-floor-published-under-the-name-of-a-total.md`
- `ac8fbe339e66ade3` → `docs/trackers/2026-08-16-iron-law-gate-firing-audit.md`
- `efdaea802f0b1719` → `docs/trackers/issue-clusters/IC-5-repro-env-diverges-from-gate-env.md`
- `f2ecdd76a6189efb` → `docs/trackers/tool-usage-patterns.md`

Every one of those fourteen distinct destinations is a live, in-repo artifact that exists
on disk (checked file-by-file, 2026-09-21).

**What is MEASURED and what is INFERRED, stated separately because the difference is the
whole weight of the record.**

- **Measured:** the two `write=false` responses above, including the 18-element
  `edges_stale` array and its `"dst": null` fields; that the full run's own `edges_stale`
  holds exactly two pairs and neither is among the 18; that all fourteen destination files
  exist.
- **Inferred, by derivation rather than observation:** that the full scan classifies all 18
  as `unchanged`. The response publishes `edges_unchanged` as a count only, never as an
  array, so this is not read off the output. It follows from the code: an existing edge
  falls into `unchanged` iff it is in `desired`, and into `stale` iff it is not in `desired`
  **and** its source is prunable (`src/librarian/tools/link_scan/diff.rs:50-61`). In the
  full run `scan_truncated=false` and `unreadable=0`, so every scanned source — including
  both of the two sources above — is prunable. Had any of the 18 been absent from the full
  run's `desired`, it would have appeared in that run's `edges_stale`. It did not. So all 18
  are in `desired` there.
- **Never observed at all:** a `write=true` run deleting these edges. `edges_pruned` is `0`
  in both runs because both were report-only. The destructive step is read from
  `diff::apply` (`src/librarian/tools/link_scan/diff.rs:67-86`), which deletes every pair in
  `d.stale`; it was not executed, and this record does not claim it was.

## Reproduction

```
workspace(action="activate", path="/home/marius/work/claude/codescout")
librarian(action="link_scan", scope="project", write=false, limit=10)   # edges_stale: 18
librarian(action="link_scan", scope="project", write=false)             # edges_stale: 2
```

Read `counts` and `edges_stale` from each. **Do not add `write=true`** — that is the defect,
the catalog is machine-local and gitignored, and there is no undo.

**Instant and tree, because on a shared checkout a figure needs both.** `git rev-parse HEAD`
read `27e0247d8995d4c77f76bdfefbbb5475be36e75c` at 2026-09-21T15:43:14Z and
`0f11fc40ee6bc33498332dcdbab66f9107c86c07` at 15:51:49Z — peers were committing throughout.
The `limit=10` reading was taken twice, once in each window, and returned `edges_stale: 18`
both times (`citations` moved 110 → 119 between them as peer files landed; the stale figure
did not). The full-`limit` reading was taken once, inside the first window.

**The tree SHA is context here rather than the determinant, and saying so is part of the
measurement.** `cites` edges live in the machine-local catalog DB, which is not in git, so
they reflect whatever the last write-mode scan derived — not the commit. The corpus
fingerprint that matters is `artifacts_scanned=1770` with `scan_truncated=false`.

## Environment

Linux, `experiments`, project `codescout`, MCP stdio, catalog under
`~/.local/share/librarian/`. Shared checkout — fourteen linked worktrees present and
several peer sessions live during the measurement.

## Root cause

Three bounds, applied to one diff, and only two of them agree.

1. **`existing` is unbounded.** `src/librarian/tools/link_scan/mod.rs:790` calls
   `links::by_rel(&cat, diff::CITES_REL)`, whose own doc comment reads *"All links with the
   given rel, across the whole catalog"* (`src/librarian/catalog/links.rs:37-41`). No scope,
   no limit.
2. **`prunable` is SOURCE-bounded to the scan.** `src/librarian/tools/link_scan/mod.rs:786-789`
   builds it from the ids of artifacts actually extracted this run. The module doc states the
   intent explicitly: *"only edges whose `src_id` is in `prunable_src` … may be pruned. A
   scoped run must not delete edges owned by unscanned artifacts"*
   (`src/librarian/tools/link_scan/diff.rs:4-9`). That half works.
3. **`desired` is bounded at BOTH ends by the scan.** The resolver's inputs are built only
   from the scanned rows: `resolve::DefinitionIndex::build` over `extracts`
   (`src/librarian/tools/link_scan/mod.rs:398-403`), and `resolve::Corpus`'s `ids`,
   `by_rel_path` and `by_stem` filled from `for row in &rows`
   (`src/librarian/tools/link_scan/mod.rs:411-431`), where `rows` is the `limit`-truncated
   find (`src/librarian/tools/link_scan/mod.rs:354-365`).

The prune predicate is then `!desired.contains(pair) && prunable_src.contains(pair.0)`
(`src/librarian/tools/link_scan/diff.rs:57-61`). A citation whose destination is outside the
window fails step 3 — it resolves to nothing, so no `desired` pair — while its source
satisfies step 2. Both conditions hold, and the edge is stale.

In mechanism-language: **the guard is keyed on `pair.0` while the thing that determines
membership of `desired` is keyed on `pair.1`.** The unscanned-destination case is exactly
the one the module comment's reasoning covers and its predicate does not.

*Measured 2026-09-21 by the two `write=false` runs above; the deletion step is read from
`src/librarian/tools/link_scan/diff.rs:67-86` and was deliberately not measured.*

## Evidence

### The same response reports the citation as unresolved and the edge as deletable

In the `limit=10` run, `dangling[].raw` for source `3f62fb253da43649` holds
`d9d291b44775e50d`, and `edges_stale` holds the pair `(3f62fb253da43649,
d9d291b44775e50d)`. For source `bc284a781fc9f5f2`, `dangling[].raw` holds
`194c5945591dfc7e` and `edges_stale` holds that pair too. Token-form raws behave the same
way: `IC-18` and `IC-20` are reported dangling from both sources, and their definers —
`48671109181877b9` and `58e59d66022a8b14` — are stale destinations for both. One citation,
two findings: *"this citation is broken"* and *"delete the edge it made"*, in the same
payload, from a scan that simply did not look at the target.

### A destination outside the window can be reported in NO findings array while still driving its edge's deletion

`docs/trackers/issue-clusters/IC-24-value-correct-in-a-frame-its-name-does-not-state.md`
cites the bare token `A-38` (*"with `A-38` alone supplying ten rows"*).
`docs/trackers/prompt-hamsa-audit-log.md` defines `## A-38`, and the full scan derives that
edge (`59ebeebb6ed05c89` is not in the full run's `edges_stale`). In the `limit=10` run the
string `A-38` appears **nowhere in the entire response** — not in `dangling` (36 entries,
`truncated.dangling=false`), not in `ambiguous`, `cross_repo`, `malformed_qualifier`,
`cross_repo_file_qualified` or `rests_on_unresolvable` — yet the edge is listed stale.

The cause is the prefix gate, which narrows with the corpus:
*"`known_prefixes` is the dangling gate: every alpha prefix with ≥1 definition anywhere in
the corpus, plus every prefix a ledger DECLARES via `entry_prefix`"*
(`src/librarian/tools/link_scan/resolve.rs:69-82`). With the hamsa audit log unscanned,
prefix `A` has no definer and no declarant in the 10-artifact corpus, so `A-38` is silenced
as prose-acronym noise — correct behaviour for that gate's purpose, and it means **the
report a caller reads before deciding to pass `write=true` does not enumerate every citation
the prune is based on.**

Four of IC-24's stale destinations are reached through a third route again:
`bug-fix-session-log:F-171`, `:F-168`, `:F-170` and `embedder-stack-ops-session-log:F-7` are
reported in `cross_repo` — a qualified token whose qualifier stem is missing from the capped
`by_stem` map is classified as belonging to another repository, when both ledgers are in this
one.

### The tool asserts idempotence on the call that has just destroyed something

On every `write=true` call, `src/librarian/tools/link_scan/mod.rs:999` returns:

```
edges written as rel="cites" (scanner-owned). Re-run any time — idempotent.
```

and the module doc says the same: *"Scanner-derived edges are idempotent (INSERT OR IGNORE
on the (src,dst,rel) PK) and regenerate on every run"*
(`src/librarian/tools/link_scan/mod.rs:5-11`).

That is **true of the INSERT half** — `diff::apply` inserts through `links::insert_with`, and
the composite PK makes a duplicate a no-op — and **false of the DELETE half**, which is not
idempotent under a changed scan window: run at `limit=10` it removes edges, run at the
default `limit` it re-adds them, and the two runs do not commute with a third at another
`limit`. The sentence arrives at exactly the moment a caller has just run the destructive
form, and it invites the repeat. `write=false` returns a different and accurate hint
(*"report only — pass write=true to materialize/prune the cites edges above"*), so the false
claim is reachable only on the call where it costs something.

### The sibling table already carries the fix, and its comment already names this hazard

`entry_cite` has an `origin` column (`src/librarian/catalog/entry_cite.rs:11`), `ORIGIN_SCAN`
(`:47`), and `prune_scan_rows` whose deletion is scoped to *both* `origin='scan'` and the
scanned sources — with a doc comment that reads: *"a global prune would delete rows
belonging to artifacts this pass never looked at and could not re-derive — silently dropping
edges a wider earlier scan had correctly materialized. This mirrors the `prunable` set the
artifact-grain path already builds from the rows it actually extracted"*
(`src/librarian/catalog/entry_cite.rs:61-79`). The reasoning is right and it stops one step
short: it reasons about the rows a pass *never looked at* on the source axis only.

`links` has **no** `origin` column — `LinkRow` is `src_id`, `dst_id`, `rel`, `created_at`
(`src/librarian/catalog/links.rs:8-13`) — and `doc(action="link")` accepts any `rel` string
with no validation against `CITES_REL` (`src/librarian/tools/link.rs:9-13`, inserting through
`links::insert`). So a hand-written `rel="cites"` edge is byte-indistinguishable from scanner
output and is prunable on the same terms. **Whether any hand-written `cites` rows exist in
this catalog today is NOT established** — nothing in the schema records provenance, so the
question is not answerable from the data, and no damage to such a row is being claimed.

### The unit tests are monotone under this defect

`diff_partitions_add_stale_unchanged` (`src/librarian/tools/link_scan/diff.rs:102-115`) and
`unreadable_src_is_never_pruned` (`:118-124`) both vary the **source** axis only — the first
carries the comment *"b→z NOT stale (src outside scanned set)"*. Neither constructs a pair
whose source is scanned and whose destination is not, which is the only shape that exhibits
this. A green suite here is silence about the case, not coverage of it.

## Hypotheses tried

1. **Hypothesis** — the 18 stale pairs are genuinely stale and the full scan protects them
   only by excluding them from `prunable`.
   **Test** — read `counts` for the full run: `scan_truncated=false`, `unreadable=0`, so
   `prunable` is every scanned id; both sources are project artifacts inside the 1770.
   **Verdict** — rejected. With both sources prunable, absence from `desired` would have put
   them in the full run's `edges_stale`, which holds two unrelated pairs.
2. **Hypothesis** — the capped run's `"dst": null` means the destination artifact does not
   exist, i.e. the edges really are dead.
   **Test** — resolved all fourteen distinct `dst_id`s against the catalog and checked each
   path on disk.
   **Verdict** — rejected. All fourteen exist and are live; `null` is a reporting artifact of
   the destination being outside the scan window.
3. **Hypothesis** — the report at least enumerates every citation whose non-resolution drives
   a prune, so a careful reader could audit the delta before writing.
   **Test** — traced `59ebeebb6ed05c89` back to the bare token `A-38` and searched the whole
   `limit=10` response for it.
   **Verdict** — rejected. `A-38` appears in no findings array (see § *Evidence*), because
   the prefix gate narrows with the corpus.
4. **Hypothesis** — this is a re-file of one of the archived `link_scan` cap bugs.
   **Test** — a title-contains query over bugs including the archive returned 10, and a
   semantic search for link_scan prune/cap defects returned 25.
   **Verdict** — rejected; see § *References* for the scope caveat on that search.

## Fix

**FIXED 2026-09-24 at `ea151a39`, by the narrow-the-prune repair this section preferred.**
`diff::diff` now takes the resolver corpus's ids and prunes only a pair whose DESTINATION is
resolvable, mirroring the source bound (`src/librarian/tools/link_scan/diff.rs`). Its failure mode
is the safe one: a genuinely dead edge to an unscanned destination survives until a scan wide
enough to see both ends. The write-mode hint at `src/librarian/tools/link_scan/mod.rs` no longer
claims idempotence.

Not taken: widening the resolution corpus to the whole catalog (it would make `limit` stop
bounding the work), and refusing a prune outright when `scan_truncated=true` (a reasonable
further tightening, not needed once the prune cannot reach an unscanned destination).

Re-measured before the fix, 2026-09-24, `write=false`: `limit=10` -> `edges_stale: 53`; full
scan -> `2`. The post-fix live reading is owed after a rebuild — see `unverified:`.

## Tests added

`diff::tests::an_edge_to_an_unscanned_destination_is_never_pruned` (red first; a both-ends-scanned
pair keeps it from passing under "prune nothing") and
`an_unsupported_edge_between_two_scanned_artifacts_is_reported_stale` in
`src/librarian/tools/link_scan/mod.rs`, a call-level wiring pin — nothing in the module asserted
that anything is ever pruned. Mutation via `scripts/mutation-probe.sh`, 2/2 KILLED.

## Fix provenance

- **SHA:** `ea151a39` (`experiments`)
- **patch-id:** `cadee09dc3e8d041e721b9e8574886c0f9211adc`

## Workarounds

- **Only ever run `write=true` with no `limit` and the widest scope you mean**, and check
  `counts.scan_truncated` is `false` in a `write=false` run first. If it is `true`, the prune
  is unsafe.
- `write=false` is the default and is entirely safe; every figure in this record came from it.
- If edges have already been lost, a single unbounded `librarian(action="link_scan",
  write=true)` re-derives every edge whose citation is still in prose — which is why this is a
  transient rather than permanent loss for scanner-derived edges, and a **permanent** one for
  any hand-written `cites` row, since nothing can re-derive those.

## Resume

N/A for the mechanism — fixed at `ea151a39`. The live re-measurement after a rebuild is in `unverified:`.

## References

- `src/librarian/tools/link_scan/diff.rs` — `diff` (`:40-63`), the prune predicate
  (`:57-61`), `apply` (`:67-86`), the source-bound module doc (`:4-9`), both existing tests
  (`:102-124`).
- `src/librarian/tools/link_scan/mod.rs` — the idempotence claim (`:5-11`, `:999`), the
  `limit` truncation (`:343`, `:354-365`), `DefinitionIndex::build` (`:398-403`), `Corpus`
  (`:411-431`), `prunable` + `existing` + the diff call (`:786-791`).
- `src/librarian/tools/link_scan/resolve.rs:69-82` — `known_prefixes`, the dangling gate that
  narrows with the corpus.
- `src/librarian/catalog/links.rs` — `LinkRow` with no `origin` (`:8-13`), `by_rel`'s
  whole-catalog scope (`:37-41`).
- `src/librarian/catalog/entry_cite.rs` — `origin` (`:11`), `ORIGIN_SCAN` (`:47`),
  `prune_scan_rows` and its doc comment naming this hazard on the source axis (`:61-79`).
- `src/librarian/tools/link.rs:9-13` — `doc(action="link")` accepts any `rel` unvalidated.
- **Ledger checked, and the check was not exhaustive.** A bugs query filtering titles for
  `link_scan`, with the archive included, returned 10 rows, all terminal and all about other
  defects: `43a4abe4f4397663` (findings-array truncation unreachable), `e891b7c6a5b1dbe7`
  (dangling count prefix-gated), `2104fc471db2f769` (`extract()` and HTML comments),
  `fa09a4a6346ff296` (`extract()` and bare `---`), `c1769965d558de3d`, `c2a65e6e1814524b`,
  `95ba3264eeb92117`, `801e5b4c13198406`, `755f0f0ed41d19c3`, `505f5d2f2175fe5a`. A semantic
  search for link_scan prune/cap defects returned 25 hits and `hints.cap_suppressed=95` —
  **so that sweep saw a fraction of its own population and the absence of a prior record is a
  floor, not a proof.** None of the rows read covers a prune deleting correct edges.
- Cluster: `cluster/selector-narrower-than-its-population` (`IC-18`,
  `docs/trackers/issue-clusters/IC-18-selector-narrower-than-its-population.md`). **The fit,
  and the one place it is imperfect, recorded rather than smoothed.** `IC-18`'s claim names
  *"a resolution path"* as a selector and states the failure exactly: *"because the excluded
  members were never examined there is no count to report and nothing to mark. A zero reads
  as 'not present' rather than 'not looked at'."* That is this mechanism verbatim — the
  citation resolver's corpus is narrower than the population of citation destinations, and
  an unexamined destination returns a well-formed *unresolved*. Where it strains: `IC-18`'s
  `**Blind party:**` is *"the reader of the result"*, and its kept-apart-from-`IC-13` test
  reasons about a **reported** partial answer. Here the false zero is consumed **internally**
  and drives a `DELETE`, so the injured party is the catalog rather than a reader, and no
  current `IC` claim states a destructive consequence. `IC-13` was considered and rejected on
  its own stated boundary — it excludes a marker the caller *can* see, and `artifacts_scanned`
  plus `scan_truncated=true` are both in the response. `IC-14` was considered and rejected
  because `prunable_src`'s name is honest about covering the source axis; nothing gets through
  a guard here, a second bound is simply narrower. `IC-24` was considered and rejected on
  *"the value is exactly right and exactly recoverable"* — the capped `edges_stale` is not
  recoverable from the capped report.
- `IC-18`'s `**Members:**` gains this file's stem in the same commit, per the roster gate
  (`docs/trackers/issue-clusters.md`, `1b5a080fe2efcb6b`).
