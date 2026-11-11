---
kind: tracker
status: active
title: "Evidence — edit acknowledgements do not confirm what is now in the file"
tags:
  - frictions
  - edit_code
  - edit_file
---

# Evidence — edit acknowledgements do not confirm what is now in the file

Ledger entry: FR-10 in [index.md](index.md). Window: 2026-10-12 to 2026-11-10 UTC. Corpus: `.codescout/usage.db`. Sources: the `refactor` miner (class 4) and the `editfile` miner (`edit_stale_match`).

## Verification re-reads

An edit that succeeded and has a path, followed within 2 calls by `read_file` or `symbols` on the same path.

- 923 of 10,685 such edits (9%), across 188 principals and 108 sessions. The first run found 940.
- Re-read by `read_file` with `force=true`: 494 (53%); with a `start_line`: 506; by `read_file`: 540; by `symbols`: 383.
- Examples: row 182119 (`edit_code` insert, then `read_file` lines 350–410 with `force=true`), row 181333, row 186786.
- Verbatim acknowledgement: `{"status": "ok", "wrote_to": "/home/marius/work/claude/codescout", "rel_path": "src/librarian/tools/artifact.rs"}`.

**The obvious fix is not supported by the data.** The median `output_json` length is 158 bytes for edits that were re-read and 148 bytes for the rest. Acknowledgement size does not predict a re-read.

## Stale-match failures

`edit_stale_match` on `edit_file`: 147 rows, 49 sessions (the lead's all-tool count is 163). For 143 rows with a path:

- 73 came after the agent's own successful edit to that file and after its last `read_file` of it.
- 41 had no `read_file` of that path at all.
- 29 came after a read that followed the last edit.
- Only 8 had the old text as the agent's own earlier `new_string`.
- Only 17 of 142 were whitespace-only misses detectable from the message. Example: row 296573, `old_string` indent 33 spaces against 31 in the file.

## What this does and does not show

It shows that agents often edit twice without re-reading and that one in eleven edits is followed by a re-read. It does not show that the agents lost work, and it does not show that a richer acknowledgement would remove the re-reads.

## Idea (hypothesis, weakly supported)

Echo the new line span and its first and last lines in the acknowledgement. Test it with a before/after count of the re-read rate. The size result above is a reason to expect little.

## Not verified

- The byte-level cause of the stale matches. Writes by `edit_code` and path-spelling differences are not counted.
- Whether the re-reads are habit, policy (the gate mandates verification) or need.
