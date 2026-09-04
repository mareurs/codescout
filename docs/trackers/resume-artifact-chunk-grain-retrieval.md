---
id: '9ba8a7a553b7a097'
kind: tracker
status: active
title: Resume queue — Artifact Chunk-Grain Retrieval (AC-N)
owners:
- marius
tags:
- resume-queue
- retrieval
- chunk-grain
- embedding
- benchmark
topic: artifact chunk-grain retrieval
entry_high_water_AC: 3
entry_prefix: AC
---

# Resume queue — Artifact Chunk-Grain Retrieval (AC-N)

Work left in the artifact chunk-grain retrieval stream after the 2026-09-04 session that
measured (a) title-in-embedding and root-caused the chunk-freeze defect. Shipped state is in
`docs/trackers/retrieval-benchmark.md` § *2026-09-04 (dawn)*; this queue holds only what is
**not** done.

Opened at the 2026-09-04 vacation stop, so a laptop session can resume without the catalog —
every fact needed to restart is prose in git. Roster context:
`docs/trackers/resume-vacation-wrapup-2026-09-04.md`. Index of all queues holding work:
`docs/trackers/resume-queue-index.md`.

## How to use this queue

Entries are independent; none blocks another. Each states what to run, what a correct result
looks like, and — where it matters — what a **wrong** result would look like, since every item
here is a measurement and a measurement's characteristic failure is a plausible number rather
than an error.

**Before running anything that reads the catalog**, check the server is not on a replaced
binary: `readlink /proc/<mcp-pid>/exe` must not end in `(deleted)`. Measured 2026-09-04:
**11 of 14** running codescout servers were on deleted inodes after a rebuild, and the source
tree confirms the wrong thing — the process runs the image it loaded.

## Provenance

Stream shipped in `f4e30856`, `f796c857`, `afb4ee54`, `998f64d3`, `f35591b7`, `596aef9c`,
`1e1fb026`, `2e796b61`, `919da0cb`, `bf97825a`, `d5897c73`, `edc0087f`, `24c55642`
(all on `origin/experiments`). Session `ffb95976-dc89-4cca-87aa-c026544faf2f`.

## AC-1 — Decompose the (a) gain from the stale-vector repair it shipped with

**Valid:** conditional — a reindex isolates the title prefix from the re-embed

**Rests on:** `docs/trackers/retrieval-benchmark.md` § *2026-09-04 (dawn)*, whose "Confound"
paragraph states this openly rather than claiming a clean attribution.

**Status:** open — the measurement exists and its attribution does not.

**What was measured.** One `librarian(reindex, reembed=true, scope="project")` moved the
artifact bench from `hits@5 5/12 → 7/12` and MRR `0.3125 → 0.3958`, with `--limit 50` class
counts **byte-identical** (`hit=9 preamble=1 wrong_file=2`) while MRR rose `0.3264 → 0.4078`.
Frozen membership with moved order is the signature of a ranking gain on an unchanged corpus.

**Why it is not attributed.** That run shipped **two** changes at once: (a) `embed_queue_items`
prepending `TOKEN — title` to mid-entry chunks (`919da0cb`), and a wholesale re-embed that
repaired whatever stale vectors the chunk-freeze defect (`921a192357e54bad`) had accumulated.
The bench cannot separate them.

**The experiment.** Build with the title prefix disabled — revert only the `match &r.entry_token`
arm in `embed_queue_items` (`src/librarian/indexer.rs`), leaving the scope terminator in place —
then `reembed=true`, then the bench at `--limit 5` and `--limit 50` with **no file edit between
the two pairs**. Cost ≈ 12 min build + 7 min reindex + 2 min bench.

**What a correct result looks like.** If the gain is (a): the prefix-disabled run returns to
roughly the 5/12 · 0.3125 baseline. If the gain was stale-vector repair: it stays near
7/12 · 0.3958, because the re-embed happened either way.

**What a WRONG result looks like, and it is the likely one.** Any bench run whose MCP server
predates the build measures the old image and reports "(a) changed nothing" — a plausible
number, no error. Reconnect `/mcp` after `cargo rb` and verify
`find src crates tests Cargo.* -newer target/release/codescout` returns **empty** before
believing any figure.

**Do not skip the terminator.** Reverting the scope terminator along with the prefix would
measure a configuration that never shipped: without it, thousands of trailing-section chunks
inherit an earlier entry's token and would be prefixed with the *wrong* title.

## AC-2 — Reconcile `7695ad877b44e96a` against the root cause found under it

**Valid:** conditional — `7695ad877b44e96a` is closed or re-scoped

**Status:** open — the bug is still `open` and its own Fix section is superseded.

`docs/issues/2026-09-02-chunk-line-ranges-are-body-relative-but-published-as-file-lines.md`
records the *symptom* (a per-file constant offset between a chunk's published `start_line` and
its own entry heading). `921a192357e54bad` establishes the *cause*: stale chunk rows, not
offset arithmetic — `frontmatter::body_line_offset` computes the prefix's line count after an
`ends_with` guard and cannot be short by a constant.

**What is owed.** Decide whether `7695ad877b44e96a` closes as *superseded by* `921a192357e54bad`
or narrows to a residue neither the freeze nor the arithmetic explains, and record which. Its
hypothesis 3 — the forced re-walk that "ruled staleness out" — is **invalid** and already
annotated as such: `force=true` never reaches `replace_chunks`, so it rebuilt zero chunks.

**Do not close it silently on the strength of the new file.** The two describe different
populations, and a closure that does not name the residue leaves the next reader unable to tell
a fixed defect from an unmeasured one.

## AC-3 — AE-11 and AE-12 are mis-specified in a way the pre-flight cannot catch

**Valid:** conditional — the two cases are re-pointed or removed

**Status:** open — they inflate the denominator without testing retrieval.

`scripts/run-artifact-bench.py` gained a `defines_entry(path, token)` pre-flight so a stale
suite case reports `unscorable` rather than reading as a retrieval miss. It catches a case whose
`expect_entry` is no longer defined at `expect_path`. It **cannot** catch AE-11 and AE-12, whose
targets are byte-sized evidence stubs: the entry is defined, so the pre-flight passes, but the
chunk is too small to carry retrievable signal and no ranking change can ever score them.

**What is owed.** Re-point both at entries with real prose, or drop them and say so in the
suite. Either way the denominator moves, so **record the change beside the next bench figure** —
a suite edit and a retrieval change are indistinguishable in the score alone.

## Template for new entries

```
## AC-N — <claim-shaped title>

**Valid:** dated YYYY-MM-DD | invariant | conditional — <event>

**Status:** open | done | superseded

<what is owed, what a correct result looks like, what a wrong one looks like>
```

## History

### 2026-09-04 — opened at the vacation stop

Opened with AC-1..AC-3 so the stream resumes from a laptop. AC-1 is the item the session
explicitly deferred rather than guessed at; AC-2 and AC-3 were carried in prose across a
context compaction and had no committed home before this file.

