---
status: open
opened: 2026-09-02
closed:
severity: high
owner: marius
related: []
tags:
- cluster/unclassified
kind: bug
unverified: 'The blast radius beyond `matched` is not established — every consumer of `artifact_chunk.start_line` / `end_line` was not enumerated, only the one this session shipped.'
---

# BUG: chunk line ranges are body-relative but published as file lines, so every span points N lines short

## Summary

`artifact_chunk.start_line` / `end_line` are computed against the **frontmatter-stripped
body**, and exported to callers as if they were **file** lines. Every artifact with
frontmatter — which is essentially all of them — reports a span short by exactly the
frontmatter length. On a tracker that means the range lands inside the *previous* entry.

## Symptom (Effect)

Measured 2026-09-02 against the live catalog, `artifact-entries` suite case `AE-1`:

```
query        : "choosing where a gate lives by how fast it reports"
expect       : docs/trackers/bug-fix-session-log.md, entry W-81
rank-1 hit   : docs/trackers/bug-fix-session-log.md   ← correct file, correct rank
matched.start_line : 7793
W-81 heading is at file line 7808
entry enclosing file line 7793 : W-80        ← the PREVIOUS entry
frontmatter closes at file line 15
7808 - 7793 = 15                             ← exactly the frontmatter length
```

Retrieval is not wrong. Ranking is not wrong. The **coordinate space** of the published
number is wrong, and it is wrong by a constant that differs per artifact.

Suite result: **hits@5 0/12, MRR 0.0, `search_live: true`** — identical to the pre-change
baseline, for an entirely different reason.

## Reproduction

```bash
cargo build --release --bin codescout    # server-stack build; the lean one has no vectors
./target/release/codescout artifact find --semantic \
  "choosing where a gate lives by how fast it reports" --limit 5 --json \
  | python3 -c "import json,sys; d=json.load(sys.stdin); \
      print(d['items'][0]['abs_path'], d['items'][0]['matched']['start_line'])"
# -> docs/trackers/bug-fix-session-log.md 7793 ; W-81 is at file line 7808
```

## Environment

`experiments` at `488192e8`. Live Qdrant backend (release build default); the lean debug
build resolves to sqlite-vec, whose `artifact_vec_v2` is empty on this machine, so the
benchmark must be run with `--bin target/release/codescout`.

## Root cause

`embed_queue_items` (`src/librarian/indexer.rs`) strips frontmatter before chunking, and
`build_chunks` (`src/librarian/catalog/chunk.rs:26-47`) records `raw.start_line` /
`raw.end_line` straight from the splitter — 1-indexed **within the string it was handed**.
That string is the body. Nothing adds the frontmatter offset back before the rows are
stored, and `SemanticHit::chunk` carries them unchanged into
`items[].matched.{start_line,end_line}` (`src/librarian/tools/find.rs`).

**Why it survived every guard in the change that introduced it.** `entry_token` is computed
by `entry_tokens_by_line(body)` and looked up as `tokens.get(raw.start_line)` — both
body-relative, so they agree with each other and the token is CORRECT. The defect exists
only in the numbers that leave the process, which is precisely the surface no in-tree test
compares against a file. The Task 10 tests assert `matched.start_line == expected.start_line`
from the same `ChunkRow` they seeded, so they are satisfied by any coordinate space at all.

**measured 2026-09-02:** offsets confirmed by hand on `bug-fix-session-log.md` (frontmatter
15 lines, offset 15). Mechanism read at the bytes in `chunk.rs:26-47` and `indexer.rs`
`embed_queue_items` the same day.

## Impact

- `matched.start_line` / `end_line` are wrong for every artifact carrying frontmatter.
- The docstring shipped alongside them tells callers to read the full span with
  `artifact(get, id=…, start_line=…, end_line=…)` — which fetches the wrong span, silently.
- On an entry ledger the range typically lands in the **preceding entry**, so a caller
  following it reads a plausible, adjacent, wrong section. No error, no empty result.
- The stored rows are wrong, so this is not fixable by an export-time patch alone unless
  the offset is recoverable per artifact at read time.


### 2026-09-04 — still open, and now quantified: the fix is PARTIAL, not absent

Measured over every chunk in this repo's `docs/` carrying an `entry_token`
(n = 3,729), comparing the published `start_line` against the file line of the
heading that defines that token:

| delta | count | reading |
|---|---|---|
| `+0` | 1,305 | correct — chunk starts at its heading |
| large `+N` | ~2,000 | correct — sub-chunks of an entry longer than 2,048 chars |
| **negative** | **378** | **the defect: the chunk starts BEFORE the heading it is labelled with** |

The negatives are a **per-file constant**, and in the worst files **not one chunk
lands on its heading**:

| file | delta | chunks | on-heading |
|---|---|---|---|
| `docs/trackers/bug-fix-session-log.md` | −2 | 218 | **0** |
| `docs/trackers/open-issue-work-queue.md` | −1 | 71 | **0** |
| `docs/trackers/tool-usage-patterns.md` | −5 | 27 | 6 |

So the offset **is** applied and is short by a small per-file amount — not omitted.
That is why it stopped being visible: the original symptom was short by the whole
frontmatter (7793 vs a true 7808, 15 lines), which a reader notices. Short by 1–2
lands in the tail of the previous entry and reads as a slightly generous span.

**Three things this rules out, each measured rather than reasoned:**

- **Not staleness.** `librarian(action="reindex", force=true)` over all 1,471
  artifacts left it at 378 (up from 315 — *worse*, because more files had been
  edited meanwhile). `replace_chunks` does re-sync positions on a content-hash
  match exactly as its doc comment claims; the value it re-syncs **to** is wrong.
- **Not the frontmatter's height in any simple form.** `bug-fix-session-log.md`
  and `open-issue-work-queue.md` both have 13-line frontmatter and take
  **different** offsets (−2 and −1); `tool-usage-patterns.md` and
  `reconnaissance-patterns.md` both have 3 YAML sequence items and take −5 and
  **0**. Whatever `body_line_offset` under-counts, it is not "lines" or "sequence
  items" as such. (Hypothesis not yet tested: `parse()` may normalise the body so
  `doc.ends_with(body)` holds at a slightly different split point than the raw
  frontmatter boundary.)
- **Not the entry token.** The token is correct in every case checked — looked up
  in body coordinates *before* the shift, exactly as `build_chunks`' comment
  prescribes. Only the published line is wrong.

**That asymmetry is a free detector, and it is already in the response.**
`matched.entry_token` (computed at chunk-build time) and
`entry_at(matched.start_line)` (re-derived from the file) disagree **iff** this bug
is present. `scripts/run-artifact-bench.py` now compares them and warns; it fired
on the first run, naming `AE-1:bug-fix-session-log.md@7996 indexed=W-81
on-disk=W-80`.

**Who it costs.** Any consumer resolving *"which entry is this?"* from the line
range gets the **previous** entry — including the recovery action the tool's own
hint recommends, `doc(action="get", id=…, start_line=…, end_line=…)`. It also
re-scored the retrieval benchmark: `AE-1` moved `hit` → `wrong_entry` with the
retrieval result **byte-identical** (same rank 1, same line 7996), which reads as
a retrieval regression and is not one.

**A design that would retire the class rather than fix the arithmetic** (raised by
the user, 2026-09-04): if each chunk carried its entry's **header text**, its
**position within that entry** ("part 3 of 7"), and a **stable handle** for the
whole entry, then no consumer would resolve an entry from a line number and the
range would become advisory. `RawChunk::metadata` already exists for the first of
those — *"searchable header prepended before embedding"* — and is explicitly `None`
on the markdown path.
## Fix

**Not implemented — two shapes, and they differ in what has to be re-run.**

- **(a) Store file-relative.** Pass the frontmatter line count into `build_chunks` and add
  it to every `start_line` / `end_line`. One meaning per stored row, every consumer correct
  by construction. Requires re-chunking the corpus, because existing rows are wrong.
- **(b) Offset at export.** Leave rows body-relative and add the offset in `find.rs` when
  building `matched`. No migration, but the stored column keeps a meaning its name does not
  state, and the next consumer inherits the same trap.

(a) is preferred. Either way the regression test must compare a published range against a
**file** — the property no existing test checks.

SHA and patch-id to be recorded here at fix time.

## Tests added

None yet. The guard that would have caught this is a test asserting that the entry token at
`matched.start_line`, read from the FILE on disk, equals the token the hit reports — i.e.
the benchmark's own scoring rule, run as a unit test over one seeded artifact WITH
frontmatter. Every current test seeds a body and compares against the same body.

## Workarounds

Add the artifact's frontmatter line count to any range read from `matched`. There is no
field in the response carrying it, so the caller must fetch the file to compute it — which
defeats the purpose of the range.

## Resume

Decide (a) vs (b). If (a): thread the offset through `embed_queue_items` ->
`build_chunks`, then re-chunk (the `backfill-chunks` CLI added in `488192e8` will not do it
— it skips artifacts that already have chunk rows, so the corpus needs a forced re-chunk
or a targeted DELETE + backfill). Then re-run
`python3 scripts/run-artifact-bench.py --suite scripts/tc-suites/artifact-entries.json
--bin target/release/codescout` and expect a non-zero hits@5 for the first time.

## References

- `docs/superpowers/plans/2026-09-02-artifact-chunk-grain-retrieval.md` § Task 10, Task 12
- `docs/trackers/retrieval-benchmark.md` — baseline `hits@5 0/12` recorded 2026-09-02
- `scripts/run-artifact-bench.py` — the instrument; its `search_live` positive control
  passes here, which is exactly why the 0/12 needed chasing rather than accepting
