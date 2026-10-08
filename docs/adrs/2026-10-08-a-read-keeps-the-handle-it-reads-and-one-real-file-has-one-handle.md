---
id: '9c493e95a48b29ad'
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
"one handle per thing that was read".

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
- `read_file` with `json_path` or `toml_key` on a real file, and the filtered view of a `memory` topic:
  each stays a snapshot handle, deduplicated only when name and bytes are identical.

## Alternatives considered

- **Keep the slice handle** so a slice stays greppable. Rejected: the reason was false, and the source
  ref is already greppable.
- **Deduplicate only** (the first of two commits), leaving a range or section on its own snapshot handle.
  Rejected by the operator: a changed file still made a second handle.
- **Use the whole-file handle only below a size limit.** Offered, not chosen. The operator kept one handle
  for every size.
- **A byte budget on the pool**, or a lazy whole-file entry that keeps the path, stamp and length and reads
  the text on access. Not built (see Revisit when).

## Consequences

- Easier: a caller holds one ref for a buffer and one for a file. The pool no longer fills with copies of
  one text. By code reading (not measured), fifty repeated whole reads of one 736 KB file could keep fifty copies before. They keep one now.
- Harder, **retention**: a range or section read of a file now keeps the whole file. A probe of 12 files of
  5 MB read by range kept 62,914,560 bytes, against 288,948 bytes before (217 times). The pool caps entries,
  not bytes, so 50 large files can fill memory.
- Harder, **line numbers move**: a range or section handle now follows the file, as a whole-file handle
  already did. After an edit above the range, a line number a holder noted earlier names other text.
- Harder, **the refresh notice reaches only `run_command`**. A `read_file` of the handle does not report it.
- Residual: two writes inside one millisecond have equal mtimes and cannot be ordered. Two readers that
  both stat a future mtime are not ordered by the capped stamp. The handle is still never served stale,
  because the file's mtime stays past the stamp and the next read re-reads it.
- Residual: `\r\n` is dropped, and a trailing newline is lost when `json_path` pages are joined. Line
  addressing uses `str::lines()` and did so before.

## Change scenarios absorbed

- A new tool pages a buffer and mints a handle for the page: the guard
  `no_read_of_an_existing_buffer_mints_a_handle` enumerates every source kind and arm at a full pool of 50.
- Two tools read one file: `every_over_budget_read_of_one_file_names_its_one_handle` follows whole, range and
  section reads through `read_file` and `read_markdown`.
- A file changes between two reads, or two readers race: the stale-write and future-mtime tests.

## Revisit when

- Pool memory becomes a problem. Two designs are written down, neither built. (1) `total_bytes` and
  `max_bytes` in `BufferInner`, with eviction in a loop. It needs a ruling on an entry larger than the
  whole budget: admit it alone and flush the rest, make the store fallible, or truncate it and break
  "line N of the handle is line N of the file". (2) A lazy whole-file entry: retention for real files
  drops to about zero, and each handle access reads the disk.
- `read_file` with `json_path` or `toml_key` on a real file should page the value on the same path, as
  `@tool_*` now does. The value is re-serialized, so it has no line numbers in the file.
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
