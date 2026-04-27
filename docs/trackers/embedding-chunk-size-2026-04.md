---
id: null
kind: null
status: done
title: null
owners: []
tags: []
topic: null
time_scope: null
---
# Issue: Indexer sends oversized chunks to embedding server

**Date:** 2026-04-19  
**Project:** opencode (20-project monorepo)  
**Embedding server:** llama-server on port 43300, model `nomic-embed-code.Q4_K_M.gguf`

## Symptom

`index_project` runs, stays at `done: 0, total: 0` for a long time, then fails with:

```
HTTP 500 Internal Server Error from embedding server:
{"error":{"code":500,"message":"input (8553 tokens) is too large to process.
increase the physical batch size (current batch size: 4096)","type":"server_error"}}
```

## Root cause

The indexer produces chunks larger than the server's physical batch size (4096 tokens).
`nomic-embed-code` supports up to 8192-token context, but the llama-server default
`--ubatch-size` is 4096, which caps how many tokens can be processed in a single forward pass.

## Workarounds tried (none worked)

- Added `chunk_size = 3000` and `chunk_overlap = 200` to `[embeddings]` in `project.toml`
  → keys appear to be ignored by the indexer (no effect)

## Fix options

1. **Server side:** Launch llama-server with `--ubatch-size 8192` (or match to model max context).
2. **Indexer side:** Respect `chunk_size`/`chunk_overlap` from `project.toml` when chunking,
   or add a hard cap that stays below 4096 tokens per chunk by default.
3. **Indexer side:** Catch the 500 token-too-large error and retry with a smaller chunk
   instead of aborting the whole index run.

## Additional context

- The `ignored_paths` config (`node_modules`, `dist`, `target`) also did not take effect —
  the indexer scanned all 77k files (including node_modules) before the exclusions were applied.
  This caused the initial long `0/0` hang before the token error surfaced.
- The `done/total: 0/0` status during file discovery gives no progress signal, making it
  hard to distinguish "still scanning" from "stuck/hung".
