---
title: experiments → master promotion plan
kind: plan
status: active
tags: [release, promotion]
created_at: 2026-04-21
---

# experiments → master Promotion Plan

265 commits ahead of master as of 2026-04-21. Plan executes in dependency order.
Cross-reference: `docs/trackers/experiments-to-master.md` for readiness rationale.

## Rules

- Run `cargo fmt && cargo clippy -- -D warnings && cargo test` after each phase.
- Features with an experimental doc need four graduation steps (see template below).
- After all cherry-picks in a phase, rebase `experiments` on `master` once.
- Never force-push `master`.

### Graduation template (features with experimental doc)

```bash
git checkout master
git cherry-pick --no-commit <sha>
# 1. git mv docs/manual/src/experimental/<feature>.md docs/manual/src/<chapter>/<feature>.md
# 2. Remove `> ⚠ Experimental` callout from the moved file
# 3. Add entry to docs/manual/src/SUMMARY.md
# 4. Remove entry from docs/manual/src/experimental/index.md
git commit -m "feat(...): ..."
```

### Standard template (no experimental doc)

```bash
git checkout master
git cherry-pick <sha>          # or range: <oldest>^..<newest>
git push
git checkout experiments
git rebase master
```

---

## Phase 1 — Foundation (must land first)

- [ ] **Cargo workspace conversion**
  - Commit: `e33532e`
  - Cmd: `git cherry-pick e33532e`
  - No experimental doc.

- [ ] **jemalloc global allocator**
  - Commit: `88efd2c`
  - Cmd: `git cherry-pick 88efd2c`
  - No experimental doc.

- [ ] **Verify + rebase**
  - `cargo test && cargo clippy -- -D warnings`
  - `git checkout experiments && git rebase master`

---

## Phase 2 — Bug Fixes

- [ ] **BUG-035/037/039 — path disambiguation, ANSI stripping, attr walk-back**
  - Commit: `102b4cf`
  - Cmd: `git cherry-pick 102b4cf`

- [ ] **BUG-040 — atomic_write preserves Unix exec bit**
  - Commit: `98faa30`
  - Cmd: `git cherry-pick 98faa30`

- [ ] **BUG-041 — retry on stale LSP positions**
  - Commit: `bfeeed1`
  - Cmd: `git cherry-pick bfeeed1`

- [ ] **BUG-042/043 — body-only new_body guard + section EOF wipe guard**
  - Commit: `fef3aa8`
  - Cmd: `git cherry-pick fef3aa8`

- [ ] **BUG-044 — sibling-preservation on nested symbol edits**
  - Commits: `be345cd` (fix) + `e391913` (regression tests)
  - Cmd: `git cherry-pick be345cd e391913`

- [ ] **BUG-036 — stale start_line validation tightening**
  - Commit: `6de19d9`
  - Cmd: `git cherry-pick 6de19d9`

- [ ] **Cancel handling — suppress cancel response + cancel-aware dispatch**
  - Commits: `b5121f2` + `04fce16`
  - Cmd: `git cherry-pick b5121f2 04fce16`

- [ ] **Verify + rebase**
  - `cargo test && cargo clippy -- -D warnings`
  - `git checkout experiments && git rebase master`

---

## Phase 3 — codescout-embed Extraction

Multi-commit refactor. Cherry-pick as a range — all are pure moves, no logic change.

- [ ] **Extract codescout-embed crate**
  - Commits: `abb9253` (scaffold) → `ed42e7a` (migrate callsites)
  - Range cmd: `git cherry-pick abb9253^..ed42e7a`
  - No experimental doc. Pure refactor.

- [ ] **Verify + rebase**
  - `cargo test && cargo clippy -- -D warnings`
  - `git checkout experiments && git rebase master`

---

## Phase 4 — Core Feature Improvements

### Three-level Guidance Taxonomy

- [ ] **Hint / Warning / MustFollow levels**
  - Commits: `1ca5566` (errors) + `8d5acb1` (read_markdown) + `072b61b` (docs)
  - Cmd: `git cherry-pick 1ca5566 8d5acb1 072b61b`
  - No experimental doc (internal tool improvement).

### Prompt Efficiency Overhaul

- [ ] **D1–D5 prompt refactors + ONBOARDING_VERSION=6**
  - Commits: `394f47c d3ab411 c2b5781 b4625a0 5b5b02b 17a0bea bf80963`
  - Note: Pick in that order — each D-commit builds on the previous.

### MCP Resources + Progress Notifications

- [ ] **Resource registry + doc:// + memory:// + project://summary**
  - Commits: `164854d 9adf313 ee7df97 0fe192b 2b9d8e8 22a4665 beb04b1 ba8ac5a`
  - Experimental doc: `docs/manual/src/experimental/mcp-resources.md`
  - Graduate to: `docs/manual/src/reference/mcp-resources.md` (or appropriate chapter)

- [ ] **Progress notifications (2 Hz throttle + ProgressSink)**
  - Commits: `f050c82 b29ee82`
  - Bundled into mcp-resources graduation above.

### read_markdown Improvements

- [ ] **Adaptive tiers + @file_* refs + MustFollow overflow**
  - Commits: `5be8e50 a73b6e7 63d60df 8d5acb1 a73b6e7 8820f3a`
  - Experimental doc: `docs/manual/src/experimental/read-markdown-improvements.md`
  - Graduate to: `docs/manual/src/tools/read-markdown.md` (or tool reference chapter)
  - ONBOARDING_VERSION=7 included in `8820f3a`.

### list_symbols Progressive Directory

- [ ] **Three-mode dispatch (flat / class_overview / directory_map)**
  - Commits: `bce2042 063dd20 ad82901 9c1c97e 3c1ebca b1d220a e081c6d`
  - Experimental doc: `docs/manual/src/experimental/list-symbols-progressive-dir.md`
  - Graduate to: `docs/manual/src/tools/list-symbols.md`

### Bash / Shell Support

- [ ] **tree-sitter-bash + AST chunker + LSP config**
  - Commits: `fb439ad 8281775 2739921 f0e1f4d 7d21a59 c1567cb 725be72`
  - Experimental doc: `docs/manual/src/experimental/bash-language-support.md`
  - Graduate to: `docs/manual/src/language-support/bash.md`

### Write Serialization

- [ ] **Cross-process write lock**
  - Commits: `6960923 d6e6650 61899f2 f252639 48357f8 66cc946 c924bf8 ed8961a 5fcbef8`
  - Experimental doc: `docs/manual/src/experimental/cross-process-write-serialization.md`
  - Graduate to: `docs/manual/src/reference/write-serialization.md`

### Per-file Diversity Cap (semantic_search)

- [ ] **File diversity rerank**
  - Commit: `a64197a` (codescout) + `ba07b05` (codescout-embed side — already in embed extraction)
  - Experimental doc: `docs/manual/src/experimental/file-diversity-rerank.md`
  - Graduate to: `docs/manual/src/tools/semantic-search.md`

- [ ] **Verify phase 4 + rebase**
  - `cargo test && cargo clippy -- -D warnings`
  - `git checkout experiments && git rebase master`

---

## Phase 5 — Needs Review

- [ ] **list_dir: disable gitignore filtering**
  - Commit: `cced68e`
  - ⚠ Review: confirm that showing gitignored files is the intended behaviour for all use cases. If yes, cherry-pick. If no, revert on experiments first.
  - No experimental doc.

---

## Phase 6 — Experimental (stabilise before promoting)

These clusters stay on `experiments` until intentionally graduated. Check off when ready to begin promotion sequence.

- [ ] **Global config (two-layer global + project merge)**
  - Commits: `a5ae941 c69c4f4 4f1dd7d eceb4dd c69c4f4 a5047fc f4b48ed dc17100 9dbd9ab eac732a bbc7736`
  - Prerequisite: write user-facing config docs first.
  - No experimental doc yet — create before graduating.

- [ ] **Index scope guard / preflight + elicitation**
  - Commits: `2a2a1f8 774e873 a1a2772 668414e e090182 fa3df7a 9ffcb7f ffcbaca 3b14983 3eeb994 26ff292 37e06d2`
  - Experimental doc: `docs/manual/src/experimental/index-scope-guard.md`
  - Prerequisite: global config must be on master first (uses `security.max_index_bytes`).

- [ ] **LSP multiplexer — Rust rollout**
  - Commits: `d8c5d80 568f64d 8df29ca 991192d b01aabd 80415b1 699629e 5efcc54 321a6de`
  - Experimental doc: `docs/manual/src/experimental/mux-rust.md`
  - Prerequisite: soak period; confirm no rust-analyzer state corruption under concurrent load.

- [ ] **Metadata-enriched chunks**
  - Commits: `44b9189 0d3c472 10a58ad 37dcb77 c1af41a f8ff23f df4cca2 57e2a48 bfc77bc 22a4616 5e815f5 40c1b99 4aad791 e4781a6`
  - Experimental doc: `docs/manual/src/experimental/metadata-enriched-chunks.md`
  - Prerequisite: codescout-embed extraction on master. Re-run embedding benchmark after graduating.

- [ ] **Asymmetric query prefix (CodeRankEmbed)**
  - Commit: `7c00eaa`
  - Experimental doc: `docs/manual/src/experimental/asymmetric-query-prefix.md`
  - Prerequisite: confirm model family before enabling globally.

- [ ] **librarian-mcp (full crate)**
  - Commits: `5660add` → `8d1e0ed` (~80 commits, entire crate)
  - Experimental doc: `docs/manual/src/experimental/librarian-mcp.md`
  - Prerequisite: cargo workspace + codescout-embed on master first.
  - Note: promote as a single block — internal commits are not independently useful.

---

## Post-promotion checklist

- [ ] Run full release cycle from `master` (bump version, tag, publish, push)
- [ ] Rebase `experiments` on new `master` tag
- [ ] Drop now-superseded original commits from `experiments` (`git rebase -i master`)
- [ ] Update `docs/trackers/experiments-to-master.md` — mark promoted clusters
