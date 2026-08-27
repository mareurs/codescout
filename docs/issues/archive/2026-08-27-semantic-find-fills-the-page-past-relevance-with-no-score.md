---
kind: bug
status: fixed
tags:
- librarian
- semantic-search
- progressive-disclosure
closed: 2026-08-27
opened: 2026-08-27
owner: marius
related:
- docs/trackers/tracker-discovery-semantic-eval.md
severity: medium
---

# BUG: `artifact(find, semantic=…)` returns a full page for a query that matches nothing — no score, no starvation signal

## Summary

`semantic_find` widens its KNN `k` until it can fill the requested page, then returns rows
in KNN order with the **distance discarded**. Ordering survives; magnitude does not. So a
caller cannot tell a strong match from the least-bad remainder, and a filtered query whose
genuinely-best matches were excluded by the filter is byte-identical in shape to one that
was satisfied. It reports a confident-looking list instead of "nothing here is close".

## Symptom (Effect)

A query with no plausible match in the corpus returns a **full page** of results, no
score, no warning, `hints: {}`:

```
artifact(action="find", scope="umbrella", kind="tracker", limit=5,
         semantic="Neapolitan pizza dough hydration percentage and 00 flour proofing times")

count: 5
  720408ecd2391251  Prompt Hamsa — Audit & Self-Reflection Log
  09d45a9c7d7cf203  Skill Frictions Tracker
  061f99863f38e86d  Session Log — pi-agent-integration
  c7cd64b9230d1548  Tracker-Discovery Semantic Search Eval
  8aa6ee38a95cc309  Tracker: Fixture Expansion
hints: {}
```

There is no field in that response — not one — that differs in kind from the response to a
query that matches at cosine-near-zero.

## Reproduction

`git rev-parse HEAD` → `c93ffc40cd8c8949b5f7c79182c7583ced124308` (branch `experiments`).
Three calls, in this order. The third is the control that makes the first two legible.

**1 — nonsense query, filtered.** Returns a full page (above).

```
artifact(action="find", scope="umbrella", kind="tracker", limit=5,
         semantic="Neapolitan pizza dough hydration percentage and 00 flour proofing times")
```
Expected if the bug is present: 5 trackers, no score, `hints: {}`.

**2 — real query, filtered to the wrong kind.** The true best matches are `kind: bug`;
filtering to `tracker` removes them and the page is backfilled with unrelated trackers.

```
artifact(action="find", scope="umbrella", kind="tracker", limit=8,
         semantic="exporting ANTHROPIC_BASE_URL does not route claude code; how to record what the client sent and verify thinking capture")
```
Expected: 8 trackers, none about the subject. Reads as "nothing indexed covers this."

**3 — same query, unfiltered (the control).** Proves the corpus *does* answer it.

```
artifact(action="find", scope="umbrella", limit=10,
         semantic="exporting ANTHROPIC_BASE_URL does not route claude code; how to record what the client sent and verify thinking capture")
```
Expected: `eefaa1f6019877fe` (the routing bug) at **#1** and `75e81959cc64efa7` (the
thinking bug) at **#3** — both `kind: bug`.

Call 2 and call 3 differ by one parameter and reach opposite conclusions, and **only call 2
is the one an agent following the activation-bootstrap guide actually makes**, since that
guide prescribes `find(kind="tracker")` and `find(kind="bug", …)` as separate steps.

## Environment

- codescout `c93ffc40`, branch `experiments`, Linux, stdio MCP.
- Vector backend: Qdrant (`CODESCOUT_QDRANT_URL=http://127.0.0.1:6334`), dense embedder
  `CodeRankEmbed-Q4_K_M.gguf` via llama-server, 768-dim.
- Scope `umbrella` = `codescout-ecosystem` (codescout, prompt-engineering, claude-plugins,
  researcher, llm-proxy). ~1,160 artifacts catalogued.

## Root cause

`semantic_find`, `src/librarian/catalog/find.rs:245-278`. Measured 2026-08-27 by the three
calls above; mechanism read from the source at the lines cited.

- `src/librarian/catalog/find.rs:255` — `let target = limit + offset;`
- `src/librarian/catalog/find.rs:256` — `let mut k = (target * 5).max(100);`
- `src/librarian/catalog/find.rs:257` — `const K_CAP: usize = 2000;`
- `src/librarian/catalog/find.rs:260` — `store.knn(project_id, query, k)` returns candidate
  ids in distance order
- `src/librarian/catalog/find.rs:271` — returns as soon as `all_rows.len() >= target`, **or**
  when `k >= K_CAP`
- `src/librarian/catalog/find.rs:276` — otherwise `k = (k * 2).min(K_CAP)` and retries

The loop's exit condition is **page fullness, not relevance**. A selective filter does not
shrink the result set; it makes the function reach further down the KNN list until it has
`target` survivors. `store.knn` returns ids only, so the distance is not carried past
line 260 — `find_by_ids_filtered` hydrates `ArtifactRow`s that have no score field, and
nothing downstream can reconstruct one.

This is correct behaviour for *pagination* — the comment at `find.rs:274` calls the retry
path "Selective filter starved the page" and widens deliberately. The defect is that
starvation is invisible to the caller: the very condition the code names is the one it does
not report.

## Evidence

### Nonsense query returns a full page

Quoted in *Symptom* above. 5 of 5 slots filled, `hints: {}`.

### The same artifact ranks #1 when the query is verbatim

```
artifact(action="find", scope="umbrella", kind="tracker", limit=10,
         semantic="Observability instruments — what this proxy can see, and how to ask it")

  #1  b8215708d2ff5f9c  Observability instruments — what this proxy can see, and how to ask it
  #2  fe0cdbd8eecdebd9  PROBES — the measurement instruments in this repo
```

So retrieval and the `kind` filter both work. The artifact is embedded and reachable.

### The paraphrase query, unfiltered, answers correctly

```
  #1  eefaa1f6019877fe  [bug]  A client that doesn't set ANTHROPIC_BASE_URL bypasses the proxy silently
  #3  75e81959cc64efa7  [bug]  Headless claude -p logged empty thinking — both stated causes falsified
```

### What it cost, in this session

I ran call 2, read the weak tail as a retrieval failure, and reported to the user that
"semantic ranking doesn't find these" — a wrong conclusion published off a filtered query
run without its unfiltered control. The response gave me nothing to catch it with. That is
the user-facing shape of this bug: it does not produce an error, it produces a plausible
paragraph.

## Hypotheses tried

1. **The `kind` filter bypasses or discards semantic ranking.** — *Test:* verbatim-title
   query with `kind="tracker"`. — *Verdict:* **rejected**; target returns at #1. See
   *Evidence › verbatim*.
2. **The new artifacts were never embedded.** — *Test:* `librarian(reindex)` reported
   `embedded: 1` for the tracker; verbatim query retrieves it. — *Verdict:* **rejected**.
3. **The page is backfilled past the point of relevance, and the distance is dropped.** —
   *Test:* nonsense query returns a full page; code read at `find.rs:255-276`. —
   *Verdict:* **confirmed**.

## Fix

Shipped on `experiments` in **`e4569fcc`** — patch-id
**`4008d77b9f7ea0e790f53e58914c7ea72b4b6d51`**.

Items **(1)** and **(2)** of the plan below landed. **(3)** is deliberately not done, for
the reason the plan itself gave: a relevance floor changes semantics for paginating
callers and should wait until `distance` has supplied real data on the distributions.
That argument still holds after the fix, so it stands rather than being re-litigated.

### 1 — Return the distance ✓

`ArtifactVectorStore::knn` now returns `(id, distance)`. The **trait's return type
changed** rather than a defaulted method being added, so the compiler enumerated all
three implementors; a default would have compiled clean and left any of them silently on
the old path.

Distances ride to the tool layer in a side map, not on `ArtifactRow` — that row type is
shared with every non-semantic read path, and a query-relative number does not belong on
a record that outlives the query. **`find_by_ids_filtered` needed no change at all**; it
already preserves KNN order, so the plan's "thread the map through `find_by_ids_filtered`"
step turned out to be unnecessary.

### The polarity trap this plan did not name

**The two backends disagree about which direction is good.** Qdrant's artifacts
collection is built `Distance::Cosine` (`src/retrieval/artifact.rs`), so
`ScoredPoint.score` is a *similarity* — higher is better. sqlite-vec's `artifact_vec` is
declared `vec0(id, embedding FLOAT[768])` with no metric (`schema.sql:49`), so it
defaults to L2 and its `distance` column is *lower* is better.

The plan says "carry the KNN score" as though it were one number. Emitting either raw
under one field name makes the value's meaning invert with the backend — and since
Qdrant is the default and sqlite-vec is the documented escape hatch, the wrong reading
would have appeared **only for escape-hatch users**. Silent, backend-dependent, and the
same failure class as the bug being fixed.

The trait now contracts **lower-is-closer**, and each implementation converts (Qdrant:
`1 - score`, the standard cosine-distance identity). The **scale** stays backend-defined,
so the doc comments state plainly that values compare *within* one response and never
across backends — no caller may threshold on an absolute number.

### 2 — Report starvation ✓

`hints` now carries `semantic_starved` (the widening count, plus a hint naming the
recovery action — re-run without the filter) and `semantic_exhausted`.

Worth recording *why* the hint channel was empty: `find.rs` returned `json!({})` for
every semantic query, with a comment reasoning that count-based hints would mislead on a
KNN-bounded result. That reasoning is **correct** — `more_in_repo` compares against a
total KNN never computes — but the blanket empty map left the path with no channel at
all. The `hints: {}` in the symptom above was never "nothing to say"; it was "nothing
*can* be said".

### A loop defect found while writing the tests

Nothing checked whether the **store** had run out. With 2 artifacts and `limit: 10`,
`knn(k=100)` returns 2, which is not "enough", so `k` doubled to 2000 across five more
queries that could not return anything new.

The reporting consequence is the sharper one: that path would have made
`semantic_starved` fire on **every small-corpus query** — asserting the filter removed
the nearest matches when the truth was that the corpus has two rows. A signal that fires
when it is not true is worse than no signal. `candidates.len() < k` is the discriminator,
and it separates exhaustion from starvation cleanly.

### Tests

6 added. Each verified to **fail** with its part reverted — a three-way revert failed
exactly four, each attributable — while the no-distance control and the pre-existing
ranking test stayed green.

The load-bearing pair, per this file's own § Tests added, asserts the **signal** and not
the ranking:

- `a_filter_that_excludes_the_nearest_matches_reports_starvation` — 150 near rows removed
  by a `kind` filter, 2 far ones survive; the page is still returned (that is defensible)
  but now labelled.
- `a_short_page_from_a_small_corpus_reports_exhausted_not_starved` — the control that
  keeps the hint from firing unconditionally, which would have looked correct in every
  other test.

Gate: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test` —
**4595 passed, 0 failed**.
## Tests added

6 added, listed in § Fix. The two that matter assert the **signal**, not the ranking,
exactly as this section originally specified: asserting "the pizza query returns nothing"
would have been wrong, because returning the nearest survivors is defensible — returning
them *unlabelled* was the bug.
## Workarounds

- **Run the unfiltered control before believing a filtered semantic result.** Same
  `semantic=`, drop `kind`/`status`. If good matches appear that the filter excluded, the
  filtered page was starved.
- **Query the kinds separately.** The best answer to a "how do I…" question is often a
  `bug`, not a `tracker`; `kind="tracker"` silently forecloses that.
- **Prefer verbatim phrasing** from the target document when you have it — it retrieves at
  #1 even through a filter.
- **Never conclude "nothing is indexed about X" from one filtered semantic query.**

## Resume

Nothing outstanding. Items (1) and (2) shipped in `e4569fcc`; item (3) — the optional
relevance floor — stands deliberately undone on the argument this file already made, and
is now better supported: `distance` exists, so anyone proposing a floor can measure the
distribution first instead of guessing a threshold.

The one thing worth doing before that: re-run the three Reproduction calls against a
large real corpus and compare the pizza query's distances to the verbatim query's. If
they do not separate cleanly, the embedder's distance distribution is the next thing to
characterise, not the loop — as this file predicted.
## References

- `src/librarian/catalog/find.rs:245-278` — `semantic_find`.
- `src/librarian/tools/find.rs` — the tool layer that shapes the response items.
- `docs/trackers/tracker-discovery-semantic-eval.md` — a parked eval measuring which
  discovery primitive surfaces the right artifacts; related, but it evaluates *ranking
  quality*, where this bug is about the *absence of a quality signal*. Its measurement
  would be easier to run once scores are returned.
- `docs/trackers/prompt-surface-measurement-session-log.md` `W-26` — the general form:
  a comparison establishes that two groups differ, never which knob names the group. Call 2
  vs call 3 here is exactly that trap with one parameter.
- `llm-proxy:docs/trackers/observability-instruments.md` — the artifact whose
  discoverability surfaced this.
