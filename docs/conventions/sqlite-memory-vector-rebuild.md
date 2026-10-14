---
id: '16ece9f8bd10ad0d'
kind: guide
status: active
title: Rebuild SQLite semantic memory vectors on a copy
owners:
- marius
tags:
- memory
- sqlite-vec
- albert-r02
actual_model_verification: genuine CLI plus reopened recall passed on isolated Xenova/all-MiniLM-L6-v2 snapshot 751bff37182d3f1213fa05d7196b954e230abad9; Albert's own snapshot/corpus acceptance still required
verified_source_commit: 36080b79daed5ba712f14dd75a935b3fd68188f6
verified_source_patch_id: 892b29741757566939c72750ff52112f20467b89
---

# Rebuild SQLite semantic memory vectors on a copy

Use `codescout rebuild-memory-vectors` when an existing SQLite semantic-memory database contains vectors from a different embedding model or dimension. The command reads every `memory_item` row, preserves every original SQL value and literal ID, and replaces only `memory_vec`. It also recomputes all vectors when the requested dimension already matches: width does not identify a model.

The initial CLI accepts only `local-dir:<snapshot>`. The existing local loader assumes **AllMiniLML6V2Q** (384 dimensions), including its tokenizer/pooling convention; it does not authenticate a model's identity from the directory name. Use the verified complete snapshot and file hashes supplied for the intended profile. The same snapshot and embedding convention must serve future writes and queries.

## Before starting

Stop every writer to the selected database: MCP servers, background indexing and other agents using that store. Inventory every active memory database and its configured directory, including external directories and other server profiles. An explicit database repair covers one file and all namespaces/buckets in that file. It does not repair another SQLite database, Qdrant, code vectors, or librarian/catalog vectors.

Keep the original model/profile configuration and topic files. Record the source binary's full Git SHA, the model snapshot's hashes, the absolute source database path, and the record count reported below. Do not infer that every memory index has been covered from one file's success.

Use a coherent SQLite backup into a fresh trial directory. SQLite's backup API and the CLI's `VACUUM INTO` capture WAL content; copying an active main database file alone can omit committed rows. See [SQLite backup API](https://www.sqlite.org/backup.html) and [VACUUM INTO](https://www.sqlite.org/lang_vacuum.html). A standalone backup must have its own filename and no unrelated old WAL/SHM sidecars.

## Prepare an isolated copy

Set these to your selected paths; keep the source and trial separate:

```sh
SOURCE_DB='/absolute/source/project.memories.db'
TRIAL_DB='/absolute/fresh-trial/project.memories.db'
MODEL_DIR='/absolute/verified/all-MiniLM-L6-v2-Q/snapshot'
ROLLBACK_DB='/absolute/fresh-trial/original-before-rebuild.db'
CODESCOUT='/absolute/path/to/the/tested/codescout'
```

Create the trial parent directory first. This Python 3 standard-library copy refuses an existing destination and uses SQLite's online backup API. It copies the whole database, including the virtual-table backing pages; it does not load a model or inspect memory content:

```sh
python3 - "$SOURCE_DB" "$TRIAL_DB" <<'PY'
import os
from contextlib import closing
from pathlib import Path
import sqlite3
import sys

source = Path(sys.argv[1]).resolve(strict=True)
destination = Path(sys.argv[2]).absolute()
if source == destination:
    raise SystemExit("source and trial must differ")
fd = os.open(destination, os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o600)
os.close(fd)
# On failure leave the reserved destination for diagnosis; choose a new trial name.
with closing(sqlite3.connect(source.as_uri() + "?mode=ro", uri=True)) as src:
    with closing(sqlite3.connect(destination)) as dst:
        src.backup(dst)
        # sqlite3 backup preserves a WAL header; normalize only this fresh trial.
        mode = dst.execute("PRAGMA journal_mode=DELETE").fetchone()[0]
        if mode.lower() != "delete":
            raise SystemExit("trial could not become a standalone non-WAL copy")
with destination.open("rb") as copied:
    os.fsync(copied.fileno())
print("coherent trial copy prepared")
PY
```

## Validate and rebuild

Dry-run requires the explicit arguments but does not load the snapshot, write the source or create a backup. It refuses WAL before opening SQLite: a SQL read-only connection can create or change shared-memory files. Use the coherent non-WAL trial prepared above; normalize only the fresh trial, never a daily source merely to make preview pass:

```sh
"$CODESCOUT" rebuild-memory-vectors \
  --db "$TRIAL_DB" --model "local-dir:$MODEL_DIR" --dimensions 384 \
  --backup "$ROLLBACK_DB" --dry-run
```

Read the JSON report. `records` counts the actual rows in this file; `previous_dimensions` comes from the baked vec0 schema, including an empty table. `requested_dimensions` is your target. `published_dimensions` is null and `vectors_written` is zero during dry-run. A malformed row/schema is a hard error and is never silently skipped.

Run without dry-run only on the prepared trial:

```sh
"$CODESCOUT" rebuild-memory-vectors \
  --db "$TRIAL_DB" --model "local-dir:$MODEL_DIR" --dimensions 384 \
  --backup "$ROLLBACK_DB"
```

Success requires exit zero, `vectors_written == records`, `published_dimensions == 384`, and the reported fresh backup. The command validates every vector's finite values, nonzero signal and width, exact literal ID coverage, and unchanged raw memory records before commit. Backup paths must differ from the source and its SQLite sidecars, even through parent aliases. It pins source/backup identities, verifies coherent original vector bytes and rechecks the sealed backup before commit, including an empty rebuild. Backend error text is suppressed because it can echo private documents. An existing backup is refused. A zero-record database succeeds with an empty 384-dimensional table and a verified backup.

A failed embedding writes no index changes. A publication failure rolls the original vector table back. Retry starts all embeddings again; there is no partial resume state. If a backup destination was reserved before failure it remains for diagnosis and may be incomplete: use a fresh destination on the next attempt. A concurrent writer causes refusal; retain that writer's changes and stop it before retrying.

Memory use includes all replacement vectors for this one database. Confirm adequate RAM for a larger corpus.

## Accept in a fresh process

Re-run dry-run with a new unused backup filename and reconcile the record count and `previous_dimensions == 384`. Start a fresh CodeScout process against the isolated trial store using the same verified local snapshot and 384-dimensional profile. Confirm that configuration selects SQLite and the trial directory, with auto-indexing disabled until accepted. The process must use the original stored project namespace and correct database basename; verify list/read sees the original population before judging recall.

Check list/read and meaningful memory recall for known topics, including old anchored memories and multiple buckets. Restart that process and repeat recall. Compare the original and trial `memory_item` values (including IDs, raw JSON and extra columns) and the separately retained topic files. Keep the checks' real results; a healthy count alone is not relevance or whole-corpus acceptance.

Use [the native client acceptance guide](albert-client-copy-acceptance.md) for the separately verified Codex/Claude/Pi recipes. Repeat the inventory and procedure for every active SQLite memory index. Test Qdrant, code/catalog stores, Claude Code plugin loading and Codex integration separately. The command itself does not change profiles or install plugins.

## Roll back on a separate closed copy

Close the trial's processes. Preserve its current database and sidecars for diagnosis. Make a fresh coherent copy of the verified rollback database into another empty directory using the same backup script; never overwrite an open database or combine restored data with sidecars from a different file.

Use the original compatible query model/profile to test the restored old index. If the historical model is unknown, this restores the original physical rows and vectors but does not prove compatible recall. Another model producing 768 values does not establish compatibility.

Keep the daily database untouched until the full trial acceptance and the inventory of remaining indexes are complete.

## Developer verification

Measured on 2026-10-14: the genuine-model test below passed with Xenova/all-MiniLM-L6-v2 revision 751bff37182d3f1213fa05d7196b954e230abad9 (quantized ONNX SHA256 afdb6f1a0e45b715d0bb9b11772f032c399babd23bfc31fed1c170afc848bdb1). This covers actual CLI inference and fresh-store recall on two synthetic records, alongside the separate 192-record deterministic reconstruction tests. It does not accept Albert's actual corpus or a different snapshot.

The deterministic tests use real SQLite/vec0 and substitute only document embeddings. They cover 192 literal IDs/raw records, reopened KNN, failure/retry, wrong/non-finite vectors, empty/same-width schemas, missing/orphan vectors, WAL-aware backup, external writers, alternate accepted UUID spellings, publication count/ID refusal, and rollback after actual post-DDL insert/coverage failures.

An optional full CLI/model/reopened-recall test requires the verified offline snapshot; it is ignored by the default gate:

```sh
CODESCOUT_TEST_REBUILD_MODEL_DIR='/absolute/verified/snapshot' \
  ./scripts/with-slot.sh cargo test --features local-embed \
  --test rebuild_memory_vectors real_local_snapshot_cli_rebuilds_and_reopened_store_recalls \
  -- --ignored --exact
```

An ignored test is not a passing inference test. Actual-machine model and integration acceptance must be reported separately.
