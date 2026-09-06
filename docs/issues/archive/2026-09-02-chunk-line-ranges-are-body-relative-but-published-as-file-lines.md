---
kind: bug
status: fixed
tags:
- cluster/unclassified
closed: 2026-09-06
opened: 2026-09-02
owner: marius
related: []
severity: high
unverified: The original Resume's prediction that scripts/run-artifact-bench.py would show a non-zero hits@5 after this fix was NOT re-run at archive time, and the suite's number has since moved for unrelated reasons (63fae4ea, 6f032dbd), so a reading taken now would not isolate this change. The consumer enumeration that discharged the previous caveat covers src/**/*.rs only -- not raw SQL built elsewhere, nor out-of-tree readers of the catalog file.
---

# BUG: chunk line ranges are body-relative but published as file lines, so every span points N lines short

## Summary

> **FIXED at `36afd405` (patch-id `6ba7ae81ba07d8fde8870fc6162c6330093159b8`, `experiments`).**
> The description below is the state as filed on 2026-09-02, kept verbatim as the premise
> § *Fix* discharges. It does not describe current behaviour.

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

**Fixed — shape (a), store file-relative. `36afd405` on `experiments`, patch-id
`6ba7ae81ba07d8fde8870fc6162c6330093159b8`.**

(a) was preferred above and (a) is what shipped, so the stored column's name and its meaning
now agree and no consumer has to re-apply an offset.

- **`embed_queue_items` takes the WHOLE document** rather than a pre-stripped body, and derives
  the body *and* its line offset from **one** `frontmatter::parse`. A caller cannot pair a body
  from one parse with an offset from another, because it never sees the offset — the
  representable-state fix, not a discipline fix.
- **`build_chunks` gains an explicit `line_offset`** (`src/librarian/catalog/chunk.rs:71-135`),
  applied to every returned range. There is deliberately **no** 3-argument form meaning zero,
  because an implicit zero is exactly how the defect shipped. `build_single_chunk` took the
  same parameter (`chunk.rs:265`).
- **The offset is applied AFTER the entry-token lookup**, and the doc comment pins why:
  `entry_tokens_by_line` is computed over `body`, so its keys are body-relative. Folding the
  offset in before the lookup leaves every range correct and slides every token onto the wrong
  chunk — measured under mutation 2026-09-03, the preamble inheriting `W-2` while the real
  `W-2` chunk read `None`.
- **The re-chunk cost nothing**: `content_hash` is computed over `content` only, so
  `replace_chunks` keeps each chunk's id and vector and re-syncs only its position. The
  migration off body-relative ranges required no re-embedding.

The commit also aligned a fallback the two callers disagreed on: a document whose frontmatter
fails to parse is now chunked whole at offset 0 in both paths. `index_repo_sync` previously
fell back to an **empty** body, so a file with a malformed opening block was chunked into
nothing and silently never became searchable — a second, unrelated silent-loss path closed in
passing.

### The `unverified:` caveat, discharged 2026-09-06

The caveat read: *"every consumer of `artifact_chunk.start_line` / `end_line` was not
enumerated, only the one this session shipped."* Enumerated now, and the population is
smaller than the caveat implied:

- `rows_by_chunk_ids` (`chunk.rs:503`) is the only read accessor that returns `ChunkRow`s to a
  caller, and it has **exactly one production call site** — `src/librarian/catalog/find.rs:365`,
  inside `semantic_find`. That is the consumer the fix shipped against.
- `chunks_for` (`chunk.rs:471`) is called only by `replace_chunks` (`chunk.rs:340`), which
  compares stored rows against freshly built ones, and by tests. It publishes nothing.
- The other 11 files touching `artifact_chunk` (`indexer.rs`, `tools/delete.rs`, `tools/mv.rs`,
  `tools/reindex.rs`, `artifact_store.rs`, …) address rows by id for vector bookkeeping and
  never read a line range.

**Scope of that check:** `grep artifact_chunk` over `src/**/*.rs` plus `references()` on both
accessors. It does not cover a consumer that reaches the column through raw SQL it builds
elsewhere, and it says nothing about out-of-tree readers of the catalog file.
## Tests added

Four, all in `src/librarian/catalog/chunk.rs`, and they discharge this file's own requirement
that *"the regression test must compare a published range against a **file**"*:

- `a_line_offset_shifts_every_range_and_moves_no_entry_token` (`chunk.rs:705`) — the load-bearing
  one. Asserts both halves of the ordering constraint at once: at offset 0 ranges stay
  body-relative, at offset 4 every range shifts by exactly 4, **and** no `entry_token` moves.
  A fix that folded the offset in before the token lookup passes the range half and reds here,
  which is the mutation the doc comment describes.
- `a_single_chunks_range_is_file_relative_like_every_other_chunk_row` (`chunk.rs:866`) — the
  `build_single_chunk` path, which is a separate writing site and therefore a separate guarded
  site rather than a second sample of the same one.
- `an_unchanged_chunk_gets_its_line_range_resynced_when_content_above_it_shifts`
  (`chunk.rs:970`) — byte-identical content at a shifted `start_line`, so it catches a
  `replace_chunks` that preserves the vector and forgets the position.
- `build_chunks_carries_line_ranges_and_entry_tokens` (`chunk.rs:554`) — its own comment
  records that the brief's proposed `w.start_line <= 5 && w.end_line >= 7` was **one-sided in
  both directions** and could not see `start_line.saturating_sub(1)`, so it asserts equality
  on the pair instead.

Both gate lanes compile and run all four — this is catalog code, not backend code, so the
`server-stack` blind spot that affects the sibling Qdrant bug does not apply here.
## Workarounds

Add the artifact's frontmatter line count to any range read from `matched`. There is no
field in the response carrying it, so the caller must fetch the file to compute it — which
defeats the purpose of the range.

## Resume

N/A — fixed and archived.

**One thing the original Resume promised that was NOT re-run, said plainly rather than left to
look discharged.** It predicted that after the fix,
`python3 scripts/run-artifact-bench.py --suite scripts/tc-suites/artifact-entries.json` would
show *"a non-zero hits@5 for the first time"*. That prediction is untested here: the benchmark
was not re-run as part of this archive, and the number in
`docs/trackers/retrieval-benchmark.md` has since moved for unrelated reasons (chunk grain
became the default at `63fae4ea`, and Qdrant went per-project at `6f032dbd`), so a reading
taken now would not isolate this fix anyway. Whoever next runs that suite should treat the
non-zero as expected, not as confirmation of this specific change.

**Why this file sat open for three days after `36afd405` closed it.** Nothing connected the
two. The commit did not name the file, the file recorded no SHA, and
`librarian(action="doctor")`'s `non_terminal_status_with_fix_anchor` only fires on an open bug
that *records* an anchor — so it read 0, correctly, about a question nobody had asked. The
signal that did exist was in the source: `36afd405` left doc comments citing this file by path
at `src/librarian/catalog/chunk.rs:60` and `src/librarian/entry_token.rs:132`. **A live bug
file cited from a source doc comment is a cheap tell that its fix already shipped**, and it is
wired to nothing. Same mechanism, same evening, same sweep: the sibling
`docs/issues/archive/2026-09-03-editing-an-artifact-removes-it-from-qdrant-backed-semantic-search.md`.
## References

- `docs/superpowers/plans/2026-09-02-artifact-chunk-grain-retrieval.md` § Task 10, Task 12
- `docs/trackers/retrieval-benchmark.md` — baseline `hits@5 0/12` recorded 2026-09-02
- `scripts/run-artifact-bench.py` — the instrument; its `search_live` positive control
  passes here, which is exactly why the 0/12 needed chasing rather than accepting
