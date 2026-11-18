# Effort-steering archive — 2026-11-18

Research is **paused**. This is a preservation branch, not a product integration or a new experimental result. No model run was launched for this archival operation.

## Start here

- **[Where the files are](FILE-LOCATIONS.md)**: exact external paths, capsule sizes and SHA-256 values, original-file recovery map, and relocation instructions.
- [Experiment index](../../docs/trackers/effort-steering/README.md): historical synthesis, caveats and roadmap. Dates inside it remain historical.
- [Custody manifest](custody-manifest.json): original source, SHA-256, byte count, mode, storage destination and tar member for each preserved input.
- [Verification](verification.json): custody comparison and recursive credential-pattern scan. This scan is limited to its named patterns; it is not proof that every possible secret is absent.
- [Supplemental custody](supplemental-custody-manifest.json): historical data and full plans/specifications found outside the consolidated tracker in the final sweep; their original paths are retained on this branch.
- [Post-retirement verification](post-retirement-primary-verification.json): retained primary payloads checked after main cleanup, without requiring their retired original paths.
- [Compatibility context](compatibility-context.tar.gz): original issue records and the previous memory pointer. Product bug records stay in main.
- [Catalog snapshot](catalog-snapshot.json): scoped artifacts, augmentations, events, observations, reservations and incoming/outgoing links exported in a read-only SQLite transaction before cleanup.
- [Main recipes](main-sdd-recipes.tar.gz), [research recipes](research-sdd-recipes.tar.gz), [experimental code](research-tools.tar.gz), and [uncommitted tracked patch](research-uncommitted.patch). These are inert snapshots, not installed modules on this branch.

The branch starts at the published experiments commit `034e4783f9ba65d1b8da735263c8bfed61e2ae58`. It does not publish the main checkout's unrelated local commits or the research worktree's unpublished Git history.

## State checked on 2026-11-18

The generic mechanism has **not** demonstrated a quality improvement over ordinary low or xhigh. Earlier small repair comparisons and the closed paid pilot do not establish equivalence or success. Infrastructure qualification, mutation checks and successful scripted controls are not functional model outcomes.

The planned newer four-repository L/R/P/X comparison was prepared but not completed. The registered public runtime image is missing. The research source binding scan matched 597 of 597 registered files at the pre-archive audit; that does not establish current runtime readiness. Global instructions changed since the frozen input. Resume requires a fresh audit and a deliberate decision; old ACTIVE/NEXT instructions are not authorization.

Historic test/gate reports retain their original dates and exclusions. Archival verification does not retest the experimental mechanism or turn WIP into approved product code. Cost accounting retains unknown root/reviewer/embedding costs and the unresolved reservation recorded in the experiment documents.

## External custody

Eight large evidence capsules are stored locally at `/home/marius/archives/codescout/effort-steering-20261118/capsules/`, under their original relative paths: seven from the consolidated tracker and one from historical research data. Each omitted capsule has a sibling `*.external.json` at its original archive location. The old in-tree blind-challenger directory is additionally preserved as the external `legacy-resident.tar.gz` capsule (over 32 MiB compressed), with a complete member manifest and [pointer](legacy-resident.external.json). Exact sizes and hashes are in the primary and supplemental custody manifests.

This location is on the same machine. Publishing this branch does **not** upload or provide an independent backup of those capsule bytes. Do not prune this directory as a disposable cache. Other external experiment scratch roots, Docker images, and old worktrees were not deleted by this operation; the manifest states what this archive actually covers.

`research-history.bundle` in the external root preserves the Git history needed to recover research HEAD `d7236b843a231b63f39e191ee6d85b27619d75a3`. It was checked with `git bundle verify`; its prerequisite is `32d0c713de272738ae7ccba9e1e4fccb004600fe`, reachable from the published base. Its size and SHA-256 are recorded in the manifest. The external root also retains the main staged-index snapshot from before cleanup.

## Recovery and limits

[Scratch cleanup receipt](scratch-cleanup-20261118.json) records the later removal of 67 Rust build caches, three redundant imported rootfs exports, three stopped containers and six superseded image tags. Sources, logs, results, manifests and the external custody archive were retained. Original compiled-cache bytes cannot now be reverified; rebuilding and fresh qualification are required before any future run. Research remains paused.

Use a separate worktree or clone for recovery. Restore experimental code from `research-tools.tar.gz`; the custody manifest and `research-status.z` identify the retained source and WIP paths, and `research-uncommitted.patch` preserves the tracked delta against the recorded research HEAD. The archive branch's product sources are its published base; they are not a tested integration of this experimental code.

Tar members were read back and hashed without extracting or retargeting them. Absolute paths, symlink targets and sealed capsule contents were preserved. Moving a capsule does not make its old runtime paths portable. Do not rewrite sealed evidence to make old registrations appear runnable. Transient `__pycache__` directories and `.pyc` files were excluded; they are not experiment evidence.

Main cleanup retains only a short archive index and compatibility pointers for older document paths. Product bug records and unrelated sessions' work remain in the main checkout. No model efficacy claim, old history rewrite, or merge into a production branch is part of this archive.

## Main checkout retirement completed

[Completion receipt](retirement-complete.json) records the local cleanup commit, external custody hashes and final checks. [The cleanup patch](main-cleanup.patch) preserves the documentation-only main change without publishing its unrelated ancestor commits. [Final main verification](main-final-verification.json) confirms that the task paths are committed, the non-task staged entries were preserved and the remaining tracker documents are archived pointers.

The current filename-key gate caught redundant aliases. An isolated index replay reproduced 19 new key problems; consolidating 26 redundant pointers reduced them to zero, and the real index and commit hooks then passed. [The mapping](pointer-consolidation.json) records retained and retired aliases. Historical filenames and artifact identifiers remain unchanged in this storage branch as evidence; they are not a live catalog migration.
