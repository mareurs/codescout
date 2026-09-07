---
id: bf5e57977f5b6af9
kind: bug
status: fixed
title: 'BUG: `BackfillReport::missing_file` counts every read failure and reports it as "no longer on disk"'
owners:
- marius
tags:
- cluster/record-asserts-an-unchecked-completion
topic: backfill error classification
closed: 2026-09-07
opened: 2026-09-07
owner: marius
related: []
severity: low
---

# BUG: `BackfillReport::missing_file` counts every read failure and reports it as "no longer on disk"

## Summary

`backfill_chunk_vectors` increments `missing_file` on `Err(_)` from `read_to_string`, but the
field's doc comment states a specific cause — *"Artifacts whose file is no longer on disk"* — and
names a remedy that depends on it. A file that exists and is simply not valid UTF-8 is reported
as missing, and the remedy the field points to will never fire for it.

## Symptom (Effect)

```json
{"artifacts": 2807, "embedded": 61613, "skipped_empty": 0, "missing_file": 1}
```

The one artifact that stayed without chunk rows afterwards:

```
id               kind     status   abs_path
d79ff49fb590df72 unknown  unknown  .../lang-pal-engine/data/transcriptions/AUC-57-DA-Clarification-Note-2026-05-12.md
```

It is present, readable, and 20133 bytes:

```
$ file <path>
Microsoft Word 2007+

$ python3 -c "open(p,'rb').read().decode('utf-8')"
UnicodeDecodeError at byte 16: invalid continuation byte
context: b'PK\x03\x04\x14\x00\x06\x00\x08\x00\x00\x00!\x00...[Conte'
```

A `.docx` (ZIP magic `PK\x03\x04`) carrying a `.md` extension. Not missing — not text.

## Reproduction

```
git rev-parse HEAD          # 4b30601c at filing
```

1. Place any non-UTF-8 file with a `.md` extension where the librarian classifier will pick it up.
2. `librarian(action="reindex")` so it gains an `artifact` row.
3. `codescout backfill-chunks --json` → `missing_file` includes it, while the file is on disk.

Observed live 2026-09-07 with the artifact above.

## Environment

Linux (`ripper`), `experiments` @ `4b30601c`, shared catalog at `~/.local/share/librarian/catalog.db`.

## Root cause

`src/librarian/indexer.rs`, inside `backfill_chunk_vectors`' page loop:

```rust
let content = match std::fs::read_to_string(&abs_path) {
    Ok(c) => c,
    Err(_) => {
        report.missing_file += 1;
        continue;
    }
};
```

`Err(_)` discards the `io::ErrorKind`. `read_to_string` fails on `NotFound`, on
`PermissionDenied`, and on `InvalidData` (non-UTF-8) — three causes with three different
remedies, folded into a counter whose name asserts the first.

The field's own doc names a remedy conditioned on that assertion:

```rust
/// Artifacts whose file is no longer on disk. A catalog-vs-filesystem drift
/// this run declines to repair — `librarian(action="reindex")` owns removal.
pub missing_file: usize,
```

For this artifact that remedy is inert forever: `reindex` removes rows whose file is **gone**,
and this file is present. So the row keeps no chunk rows, `NOT EXISTS` re-selects it on every
run, and the catalog-wide `vectorless` count sits permanently at 1 with the report pointing at a
repair that cannot apply.

**The struct's own header argues against exactly this**, which is what makes it a clean instance
rather than an oversight:

> Four counts rather than three, because a file that is *gone* and a file that is *empty* are
> different outcomes with different remedies, and folding them into one number would make the
> report say "skipped" about two unrelated things.

The reasoning was applied to `skipped_empty` and not to the arm beside it.

measured 2026-09-07: the report above, `file(1)`, and the `UnicodeDecodeError` offset.

## Evidence

### Doc and code arrived together, so this is a wrong statement rather than drift

```
$ git log -S 'Artifacts whose file is no longer on disk' -- src/librarian/indexer.rs
488192e8  2026-09-02  feat(librarian): a resumable chunk backfill that escapes the indexer's absorbing state
$ git log -S 'report.missing_file += 1' -- src/librarian/indexer.rs
488192e8  2026-09-02  feat(librarian): a resumable chunk backfill that escapes the indexer's absorbing state
```

Same commit. This is deliberately **not** filed under `cluster/doc-contradicted-by-code`, whose
claim requires the statement to have been true when written and excludes an authoring error.

## Hypotheses tried

1. **Hypothesis:** the file really is absent and the catalog path is stale.
   **Test:** `ls -la`, `head -c 1`, `file`.
   **Verdict:** rejected — present, readable, 20133 bytes, `Microsoft Word 2007+`.
2. **Hypothesis:** it is a permissions failure.
   **Test:** `head -c 1 <path>` as the running user.
   **Verdict:** rejected — readable. The failure is `InvalidData`.

## Fix

Match on `ErrorKind` and split the counters:

```rust
Err(e) => {
    match e.kind() {
        std::io::ErrorKind::NotFound => report.missing_file += 1,
        std::io::ErrorKind::InvalidData => report.unreadable_encoding += 1,
        _ => report.unreadable_other += 1,
    }
    continue;
}
```

Each arm then carries a remedy that applies: `reindex` removal for the first, an extension/
classifier exclusion for the second, an operator action for the third. Consider also whether the
classifier should admit a `.md` file whose bytes are a ZIP container at all — that is the upstream
half, and closing it would empty this arm rather than only labelling it.

- **SHA (experiments):** `45eac50e`
- **patch-id:** `0f70f33bbc19f0a95ecb08a1966592e63a9b05d0`

> **The struct quoted in § *Root cause* is the PRE-FIX shape** and no longer matches the tree.
> `BackfillReport` now has six fields, not four. Left as quoted because a § *Root cause* is a
> record of what was wrong — but marked, because a quoted struct reads as *evidence* rather than
> prose, so a reader trusts it harder than it deserves and it decays just as fast. Generalised by
> sessionId `59112612` while working an unrelated bug whose own fix plan was one field stale:
> **a measurement of an interface, recorded and then outlived by the interface.**

**Applied.** `Err(_)` becomes `match e.kind()` into three counters — `missing_file`
(`NotFound`, the only arm whose documented `reindex` remedy actually applies),
`unreadable_encoding` (`InvalidData` — a binary payload behind a text extension, which `reindex`
can never clear because the file is present), and `unreadable_other` (permissions, I/O, a
directory — deliberately not folded into either, because both of those name a remedy and this one
means "go look").

**Fixture note, because it is the reason this nearly shipped untested.** The first version wrote
`b"PK\x03\x04\x14\x00\x06\x00\x08\x00\x00\x00!\x00\xdf\xa4"` — the real file's magic bytes,
truncated. That literal is **valid UTF-8**: `0xDF` opens a two-byte sequence and `0xA4` is a legal
continuation, decoding to U+07E4. `read_to_string` succeeded, the artifact embedded normally, and
the test passed while asserting nothing. Invalidity begins at `0xD2`, which opens a sequence `l`
(0x6C) cannot continue — byte 16, matching the real artifact. The literal now carries `\xd2l`, an
annotation on the fixture line saying what breaks if it is shortened, and an
`assert!(read_to_string(&binary).is_err())` guard so a future truncation reds immediately rather
than going quiet. The mutation run caught this, not review.

**Discrimination:** the test asserts `unreadable_encoding == 1` **and** `missing_file == 1` in the
same run, with one genuinely-deleted file present. A single `Err(_)` arm reports `missing_file:
2`, so either assertion alone would still pass — it is the pair that separates them.

## Tests added

None yet. A test writing `b"PK\x03\x04..."` to a `.md` fixture and asserting `missing_file == 0`
while an encoding counter is `1` reds on today's code and does not depend on prose.

## Workarounds

Treat a non-zero `missing_file` as "could not read", not "absent", and check the path before
acting on it. If the count will not clear after a `reindex`, the cause is not absence.

## Resume

Edit the `read_to_string` arm in `backfill_chunk_vectors` (`src/librarian/indexer.rs`, added by
`488192e8`) to match on `e.kind()`, add the two counters to `BackfillReport`, and update the
`missing_file` doc comment to state what it now counts. Then add the fixture test in
§ *Tests added* and confirm it reds first.

## References

- `src/librarian/indexer.rs` — `BackfillReport` and the `Err(_)` arm, both from `488192e8`
- `docs/issues/2026-09-07-backfill-chunks-walks-the-whole-catalog-not-the-project.md` — why an artifact in an unrelated repo showed up in this run at all
