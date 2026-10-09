---
id: b1ffbe9b27336a49
kind: adr
status: active
title: ADR-2026-10-08 — A read keeps the handle it reads, and one real file has one handle
owners:
- marius
tags:
- design-principle
- progressive-disclosure
- tool-contract
- read_file
- read_markdown
- output-buffer
topic: tool-contracts
---

# ADR-2026-10-08 — A read keeps the handle it reads, and one real file has one handle

## Status

Accepted — active. Decided 2026-10-07 by the operator after a live probe of the rebuilt binary showed
that `read_file` of a buffer gave a new handle on every call. It extends
`docs/adrs/2026-10-05-a-result-keeps-the-one-handle-its-tool-gave-it.md` from "one handle per result" to
"one handle per thing that was read". Amended 2026-10-09: the pool that holds those handles has a hard byte
budget (see Decision, *Added 2026-10-09*).

## Context

The operator's rule: when one handle holds the whole buffer, that handle is the one every later call
refers to. No call makes another buffer from it.

Measured on the live binary, 2026-10-07, with the pool at its production capacity of 50
(`OutputBuffer::new(50)`, `src/server.rs:447`):

| Call | Result |
|---|---|
| `read_file("@file_X", start_line=1, end_line=1)` on one 3,000-byte wide line, twice | `@file_159df630`, then `@file_159df698` |
| `read_file("@tool_X", json_path="$.symbols[0].body")`, twice | `@file_159df6a8`, then `@file_159df721`; a range beside `json_path` was ignored |
| `read_file("@cmd_X")` (3,000 lines, no range) | no new handle; `next` named `@cmd_X` |
| `read_file("@cmd_X", start_line=1, end_line=2500)` | new `@file_159e1852` |
| `read_file("docs/RELEASE.md")` whole, twice, file unchanged | `@file_159e1949`, then `@file_159e1a97` |
| the same 1-line real file, `start_line=1, end_line=1`, twice | `@file_159e75ca`, then `@file_159e7b33` |

The no-range read of a buffer already followed the rule. The range read did not. `store_file` and
`store` mint on every call and evict the least recently used entry at capacity. Only `store_tool`
deduplicates, by content hash. Each `next` of a paged buffer is a ranged read, so a long buffer flushed
the pool.

The comment on the slice store said it kept the response small, so that `call_content` would not wrap
it in a `@tool_*` (BUG-026). That reason was false under the current code. The page was already sized
with `response_fits`, `response_room` and `buffer_page`, and the stored copy never made the response
smaller.

## Decision

**R1. A read of a buffer ref returns that ref and makes no buffer.** The response and its `next` name the
source ref and count in the source's own lines. A buffer-read response carries no `file_id`.

**`json_path` on a `@tool_*` ref pages on the same ref.** `start_line` and `end_line` beside `json_path`
slice the value's own lines. `next` quotes the whole `json_path` while the page keeps at least 2,000
bytes of room (cap `read_file.json_path_route_room`). Above that it is plain prose with concrete
numbers. It never carries a placeholder. The whole-buffer `next` ranges to the last line, so following
`next` always reaches the end.

**R3. One real file, by resolved path, has one live `@file_*` handle.** A whole read, a line range, a
markdown section and a read through `read_markdown` all return that handle, in the file's line numbers.
`OutputBuffer::store_file_read` deduplicates by resolved path. A file that changed keeps its handle,
updated in place. The entry is stamped with the file's mtime taken before the read, capped at the clock.
A read that started before a newer one cannot overwrite it. A holder of the handle is told once, by the
first `run_command` that reads it, that the file was refreshed.

**Unchanged by this decision:**
- the `grep` tool's own `@tool_*` result, and `run_command` over a buffer that makes a new `@cmd_*`: each
  answers a new question and is a new artifact;
- the `@tool_*` content-hash deduplication and the `call_content` backstop;
- the filtered view of a `memory` topic: it stays a snapshot handle, deduplicated only when name and bytes
  are identical.

**Added 2026-10-08.** A `json_path` or `toml_key` value of a real file that is larger than one page is paged
on the file's own path: `read_file("<path>", json_path=…, start_line=N, end_line=M)`. It makes no handle. The
line numbers are the value's own, because the value is re-serialized and has no lines in the file. This was
the last place where one real file had a second handle, and `store_file_excerpt` is deleted.

**Added 2026-10-09. The pool has a hard byte budget.** `BufferInner.total_bytes` counts `stdout` plus
`stderr` of every entry and is changed only by four helpers (`admit_locked`, `remove_locked`,
`demote_locked`, `retain_file_text_locked`), all under the pool's one lock, with saturating arithmetic.
The ceiling is checked, not debug-asserted: if evicting everything evictable leaves no room, the counter is
logged and recounted from the entries, and an entry that still finds none keeps no text and records the cut.
The rulings (the first round, then the review round of the same day):

- **The ceiling is 128 MiB (`MAX_POOL_BYTES`), default and maximum. Chosen, not measured.**
  `OutputBuffer::with_byte_limit` can only lower it: it clamps to [1 MiB, 128 MiB] and logs a
  `tracing::warn!` when it clamps. The 1 MiB floor (`MIN_POOL_BYTES`, also chosen) is kept. The server
  builds its pool in `server::server_output_buffer`, which is where a setting would be read.
- **One entry holds at most HALF the limit (`entry_cap`), ruling R-A.** An entry of the whole limit left
  no other handle holding text, and the next small store evicted it: review probe S2 kept 1,048,574 B of
  `seq 1 300000` at 1 MiB, a following `seq 1 5000` evicted it, and the caller then got "buffer reference
  not found ... Buffer refs expire when the session resets", the wrong cause. That refusal (`REF_GONE`, at
  every buffer-not-found site) now also names the pool dropping handles to stay under its byte limit, and
  the route: re-run the command.
- **Eviction order.** A store that would pass the limit first DEMOTES least-recently-used whole-file
  `@file_*` entries: the text is dropped and the id, path, stamp and text hash stay. Only for a store whose
  text exists nowhere else are least-recently-used `@cmd_*`, `@tool_*` and snapshot entries then evicted.
  **The text of a whole file never evicts anything** (review probes RP1, RP2, S1): a path read of a new or
  demoted file keeps its text only if it fits after demoting other files, and is otherwise minted or kept
  demoted. The 50-entry cap stays and is applied first. A read of a demoted handle takes the text from disk
  and does not put it back, so a read never grows the pool. A resident entry refreshed by a read keeps the
  new text only if it fits the room left; otherwise it is demoted. A read displaces no other entry.
- **The disk is touched with the pool unlocked.** A read of a file-backed handle takes the path, stamp and
  state under the lock, stats, reads and hashes the file without it, and applies the result under the lock
  only if the entry is unchanged, else starts over (review probe RP8: a 300 MB demoted file held the lock
  8.9 s in a debug build and blocked another thread's 2-byte store 9.8 s). Every `@file_*` entry carries the
  SHA-256 of its whole text, computed outside the lock, so demotion hashes nothing.
- **An entry bigger than its share is never stored as is.** A `@cmd_*` or snapshot entry is stored as a
  marked prefix of WHOLE lines that fits the share, with the existing `Truncation` and `truncation_marker`,
  so line N of the handle is line N of the output for every kept line and every later read carries
  `buffer_truncated`. One line longer than the share keeps NOTHING (review probe RP4a: a 2 MB single line
  gave 0 of 1 lines kept); the ruling allows it, because part of a line would make the count false.
  `stderr` is cut on its own record (`stderr_truncated`, read through `.err`) and keeps at least half the
  share when both streams are over. A `@tool_*` too large to hold as JSON is stored in its line-addressable
  form, the line space its readers number: whole when that form fits the share (it used to be cut, losing
  the final `}`), else as a marked prefix. Its envelope names only line and `grep` routes and counts the
  bytes held, and a `json_path` read of it is refused naming the cut and the line route (review probe RPT).
  A snapshot dedups by the hash of the whole view, so a cut view read twice is one handle (probe RP3), and
  the `memory` response says the view is cut. The responses that can mint a cut entry (`run_command`'s
  summary, its tee capture, `call_content`'s envelope, `memory`) carry `output_truncated` with the kept and
  omitted line counts; the tee capture reports the handle's own record, since the capture keeps the line
  that empties its budget however long and the pool can cut it again. A whole file bigger than the share is
  minted demoted.
- **Reads that report a cut.** `read_file`, `run_command` and now the `grep` tool (review probe RPG) carry
  `buffer_truncated` on a cut handle. `truncation_notice` no longer goes through `get()`: it does not bump
  the entry to most recently used and does not refresh a file. A bare handle named in a `run_command` now
  also carries that handle's `.err` cut notice.
- **R3 and the stamp rules are unchanged.** A demoted entry keeps its one handle, its stamp, the
  stale-write rule and the once-only refresh notice; its change test is the text hash.

## Alternatives considered

- **Keep the slice handle** so a slice stays greppable. Rejected: the reason was false, and the source
  ref is already greppable.
- **Deduplicate only** (the first of two commits), leaving a range or section on its own snapshot handle.
  Rejected by the operator: a changed file still made a second handle.
- **Use the whole-file handle only below a size limit.** Offered, not chosen. The operator kept one handle
  for every size.
- **A byte budget on the pool**, or a lazy whole-file entry that keeps the path, stamp and length and reads
  the text on access. Built together on 2026-10-09: the budget, with the lazy entry as what a whole file
  becomes when it is demoted (see Decision, *Added 2026-10-09*). Of the three answers to an entry larger
  than the whole budget, the operator chose the third for `@cmd_*` and `@tool_*` (a marked prefix) and a
  lazy entry for a whole file; the store stays infallible.

## Consequences

- Easier: a caller holds one ref for a buffer and one for a file. The pool no longer fills with copies of
  one text. By code reading (not measured), fifty repeated whole reads of one 736 KB file could keep fifty copies before. They keep one now.
- Harder, **retention**: a range or section read of a file now keeps the whole file. A probe of 12 files of
  5 MB read by range kept 62,914,560 bytes, against 288,948 bytes before (217 times). The pool capped
  entries, not bytes, so 50 large files could fill memory. Bounded since 2026-10-09 at 128 MiB (see
  Decision): the same case scaled to a 1 MiB pool (twelve ~205 KB files, 2.4 MB) keeps at most 1 MiB
  (`twelve_range_reads_of_large_files_stay_within_the_byte_limit`).
- Harder, **the budget's own costs** (2026-10-09, by code reading and the unit tests, not measured live):
  two entries of half the limit fill the pool; while `@cmd_*` and `@tool_*` fill the pool, every whole file
  is demoted by the next store, even one just read, because demotion comes before any eviction, and a
  file read into a full pool is minted demoted; each read of a demoted handle reads and hashes the whole
  file (outside the lock). A `@tool_*` too large for JSON cannot be read by `json_path`. `run_command`
  still holds a command's whole output in memory before the pool sees it (`wait_with_output`), and a
  whole-file `read_file` still has no size limit: the budget bounds what the pool RETAINS, not what a call
  reads.
- Known limit, **a same-mtime rewrite is noticed only by a demoted entry** (review probe RP7): a resident
  entry compares mtimes and serves the text it holds (v1), a demoted one reads and hashes the file and
  serves v2 with a refresh notice. The two kinds of entry can therefore answer differently for one file.
- Harder, **line numbers move**: a range or section handle now follows the file, as a whole-file handle
  already did. After an edit above the range, a line number a holder noted earlier names other text.
- Harder, **the refresh notice reaches only `run_command`**. A `read_file` of the handle does not report it.
- Residual: two writes inside one millisecond have equal mtimes and cannot be ordered. Two readers that
  both stat a future mtime are not ordered by the capped stamp. The handle is still never served stale,
  because the file's mtime stays past the stamp and the next read re-reads it.
- Residual: `\r\n` is dropped, and a trailing newline is lost when `json_path` pages are joined. Line
  addressing uses `str::lines()` and did so before.
- Harder, **a value of a real file is recomputed on every page**: the file is re-read and re-parsed each time.
  A reviewer measured 0.9 s per page for a 17.8 MB JSON file in a debug build and 78-112 ms in release, and
  9 to 10 s per page with 1.3 GB peak memory for a 181 MB file. There is no size guard. If the file changes
  between two pages, the pages mix two versions: after ten lines were inserted at the top, page 2 started ten
  lines before where page 1 ended, and only `total_lines` moved. Plain real-file range pages behave the same
  way. A `@tool_*` value cannot change, so it does not.

## Change scenarios absorbed

- A new tool pages a buffer and mints a handle for the page: the guard
  `no_read_of_an_existing_buffer_mints_a_handle` enumerates every source kind and arm at a full pool of 50.
- Two tools read one file: `every_over_budget_read_of_one_file_names_its_one_handle` follows whole, range and
  section reads through `read_file` and `read_markdown`.
- A file changes between two reads, or two readers race: the stale-write and future-mtime tests.

## Revisit when

- Done 2026-10-09: a byte budget on the pool (see Decision). Reopen it if 128 MiB proves wrong (no
  measurement backs it), if the half-limit share per entry proves too small or too large, or if
  re-reading demoted files from disk becomes a cost worth a cache.
- Done 2026-10-08: `read_file` with `json_path` or `toml_key` on a real file pages the value on the file's own
  path (see Decision). Reopen it if the per-page re-parse becomes a problem; a cache would need an owner for
  its staleness.
- `peer knowledge` returns a large remote buffer whole, so the local `call_content` makes a `@tool_*`. The
  remote handle cannot be addressed locally. The tool is opt-in.
- A tool ships a second handle. The `file_id` backstop (Phase B of
  `docs/adrs/2026-10-07-one-measure-of-the-delivered-response.md`) now concerns real-file arms only.

## Built

Eighteen commits on `experiments`, merged from two branches. Patch-ids (`git patch-id --stable`):

| SHA | patch-id | Change |
|---|---|---|
| `e62f1444` | `8898abac7ab0669fc3348a18aa0e4e1b7812da7b` | a read of a buffer keeps its handle and mints none |
| `0d36088a` | `fe22f783e450f6c4ed1ad603f05d762d0c0bdd9a` | pin the `json_path` range slice and the overlong-path route |
| `5ef5642b` | `c6ae3ea0d847774ef4029ebf5790552088496230` | follow the `json_path` page hint on a ref with no `stdout` key |
| `9ae8a9c9` | `c062933ed5e2ee489c049a36699a6f929122da33` | every buffer-read `next` runs as written and reaches the end |
| `67db5d5c` | `0d7f71c567e2e90e8d963bf9a9b960ab799d4168` | the `json_path` page hint names its search's scope |
| `0499c606` | `47ec549869d29d08685fd6138ab54dc4c6db76e4` | one real file keeps one live handle: `store_file` deduplicates |
| `6ebd0615` | `18502743dece9cd27f87551e9c33f81bf45171fc` | a range or section of a real file names the file's one handle |
| `2aa12097` | `0175e5a692fed67deadd7394a4c647043299b088` | a late store of an older read never regresses a file's handle |
| `2ad3d950` | `f8b9ca620264cced16064263b25397ef7a12412c` | a holder of the handle is told once when a path read changed it |
| `637af393` | `a984b0b4ea106bf94a8d17db4f30bdc6bf261b3b` | a multi-heading error routes every section |
| `2f011371` | `85709c3282ef70c629ef8286c2d8de4bc0cd2ca5` | re-home the review's killers |
| `a7c672ec` | `bdf8cbc4b0eea700372e48090510a5c8b9908123` | a buffered markdown range page delivers its handle, count and `next` |
| `62310561` | `2bcd5d856a8c65fb66910b49e3f95f799cb0b6d6` | a write inside the read window is repaired by the next handle read |
| `c5967b51` | `94ff010e8ee83bd6ab102a11c5f073bd932ed73c` | kill the two survivors of the tip mutation run |
| `12aaf8b4` | `11d127ca3da0ff3c53d9380948da206380fcd1c4` | cap every whole-file stamp at the clock |
| `d669fa67` | `864f984d4c602c5fbe07da280baf844b4c8367bb` | a range page's text counts the lines it shows |
| `ea164515` | `322d8417f10b4f422ec6d51915ff9bb68ca93fa6` | pin the largest-field route at the echo clip |
| `ee13088b` | `c51f094c99d9befde0350d52398e5b5ccb38b373` | a second `read_hook` pins "mtime before read" |

A second round, merged on 2026-10-08 on top of the peer's `840fba50`, added the real-file value paging and the
fix of the `run_command` cut-off hint. Patch-ids:

| SHA | patch-id | Change |
|---|---|---|
| `2cd90b71` | `a6efa912705b8d25cd2148659560248233029a80` | a cut-off buffer query that is not a slice pages its own output |
| `2c4967b5` | `f6faa8b326c7a4fc2c942706ebdc3801beb32aa8` | tests: a non-slice hint's room, a piped slice |
| `97970e3d` | `bcae102dd42620f6be891ba1e022015b8eb20b48` | non-slice paging follows what each page showed; quoting it cannot model |
| `9319d9fa` | `1d9de6f500ba8522712b979900a81b296fb9ed8c` | tests that kill two survivors of the paging fix |
| `0374c4c5` | `3d06d55bcd5d290419a8e6742fd7f3a6e5b57aa7` | the adversarial table builds its regex once |
| `9e621dc0` | `cc044046bce35954f3b7f16c76533256fd677581` | a `json_path`/`toml_key` value of a real file is paged on its path |
| `327541ba` | `213d338dbeb72164cb4965576e7824a5a3fa0fe4` | tests: the key is off line 1 |
| `9304793d` | `217c61bf5441a06b6bc67eb17d2ffaecce102410` | siblings and breadcrumb judged together; a real file's handle routes to its path |
| `a9ae2ed8` | `8efaa9127b856be6cc888b08b589785b1960618b` | an empty `siblings` list is not droppable |
| `5eb486bd` | `6a531492e85a46a96ed0333fa56dbcea580c7e98` | page-loop cap in a test |
| `19ab7993` | `df13954a4fa7659ba119ef18af2a383c1f2ecd77` | hint tests are dash-safe; review killers; the `@file_` refusal names a route only if the file type accepts it |

A third round, on branch `fix/buffer-pool-byte-budget` (2026-10-09), added the byte budget. Patch-ids:

| SHA | patch-id | Change |
|---|---|---|
| `267d318e` | `1c006d08245fdaa27bccee38880bc8d08a7cf9dc` | a hard byte budget on the buffer pool |
| `46135fc2` | `971162b1564208bcf2c0463978f766f536bee236` | tests that kill the survivors of its mutation run |
| `bf1d104a` | `d3649bdf6ff717881904c2589efded0e1dce753f` | `MAX_POOL_BYTES` classified as a probed result cap (`output_buffer.pool_bytes`) |
| `e4f3f78f` | `975e3cc7b0c578f157a3f4e2f9875103c120940a` | the review round: R-A, R-B and eleven review findings |
| `0718021b` | `d4781948592629813679032f6429276cf64f45a6` | tests that kill the four survivors of the review round's mutation run |

Mutation runs through `scripts/mutation-probe.sh`, one mutant at a time, each verdict with a `test result:`
line. First round: 23 mutants left 5 survivors (one equivalent edge, three edges with no case at exactly
the limit, one assertion monotone under shrinking); after the repair 25 of 25 were killed. Two blind
reviews then found mutants that survived the whole `tools::` suite, among them a `<= max + 1` pool edge
(no case stored one byte past a full pool), four hash mutants, the server's pool built at the floor, and an
oversized snapshot whose record was dropped or stored empty. The review round re-ran 53 mutants (the earlier
set adapted to the new code, reconstructions of the reviewers' mutants from their descriptions, and one per
fix): 49 were killed and 4 survived (a tool result of exactly one share that was plain text, so its line
form was itself; a read's refresh that may demote other files; a file one byte over the share; a store hit
that does not record its hash). After `0718021b` all four were killed. Every `<=` at the limit edge and at
the share edge is killed, including `<= max + 1`. Two pieces of code no test reaches: the recount on
accounting drift and the no-text fallback in `admit_locked` (only drift or a caller bug can reach them),
and the clamp's `tracing::warn!`.

## Evidence

Measured on the merged tip and on baseline `38265405` by an independent reviewer, in-process through
`ReadFile.call` and `call_content`, pool of 50. A live probe on the rebuilt binary on 2026-10-08 agreed (a
file read twice gave one handle; a wide buffer line read twice gave none; a `json_path` value paged with a
working `Next:`; an edited file kept its handle and told its holder once). The calls are in the bug record
(`docs/issues/archive/2026-10-08-a-read-of-a-buffer-made-another-buffer-and-one-real-file-had-many-handles.md`):

| Case | Baseline | Merged |
|---|---|---|
| clamped wide line of a buffer, twice | 1 new handle per call | 0 |
| `json_path` value of 12,871 bytes, twice | 1 new handle per call, no `next`, range ignored | 0; 2 pages with `Next:`; the rejoined value matches |
| 1,000-line buffer range, followed to the end | 12 new handles | 0; the pages rejoin exactly |
| 456-line markdown file, 11 different reads | 8 handles | 1 handle |
| wide line of a real file, twice | 1 new handle per call | 1 handle for both |

Three independent reviews (one per branch, then one of the merged tip) found, and the fixers repaired before landing: a `json_path` `next` that held a
placeholder; a paging chain that stopped at line 3,046 of 20,000; a stale-write race that left the one
handle on old text; a lost refresh notice; a multi-heading error with no route; a future mtime that hid an
edit; a line count that included the cut marker. Mutation counts were reported by the fixing agents. The
last round killed 25 of 25. The final review applied 20 new mutants: 13 were caught, 4 were killed by new
tests or a new hook, and 3 are equivalent by a named invariant. Confidence: high on R1 and R3, medium on the
retention cost, which the operator accepted knowing it has no upper bound.
