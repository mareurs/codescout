# Artifact identity redesign — where the work stands (2026-11-13, after C1 landed)

Read `docs/research/2026-11-13-artifact-identity-plan-c1-execution-state.md` (doc id `9bcc00c179ecbdce`) for the task table, rulings and deferred findings of Plan C1. Its "Where the work is" section predates the landing; this memory is current.

## One-paragraph state

Spec v5 (`docs/superpowers/specs/2026-11-11-repo-relative-artifact-key-design.md`): committed files are the only store, the catalog is a derived index, the key is the lowercase filename stem, one closed citation grammar. Plans A, B1, B2 and C1 are merged on `experiments` (C1 fast-forwarded at `7db510676`, 2026-11-13; the user chose "merge locally"; worktree and branch removed). Plan C1 covered dual-write of entries and cites, the prose-path cites, `rekey_prefix` with grammar-citer refusal, and the CLI write lock. Gate on the merged tree: nextest 9919/9919 lean and 10013/10013 default, python 1836, fmt and clippy clean. Whole-branch Opus review: "Yes after fixes"; the one Important finding (I-1, digit-leading key in `rekey_prefix`) was fixed in `b146e18a3` and re-reviewed. Next plans: C2 (events, observations, links), C3 (content-keyed vector cache; needs a persistent derived-file location first), D (cutover tool), E (read/write flip, deletion tail).

## Still to do after the landing (controller, from the main checkout)

DONE 2026-11-13 (commits 9338195fc, 87737a242): DC and WIN recipes pass a section; `KNOWN_SECTIONLESS_LEDGER_RECIPES` is empty (the gate tests take an explicit list); PV and A recipes use `doc(`; the 11 sidecar prompts are aligned (`sidecar_shape_drift` 0); no "worktree-local file" left in the guides; bugs filed: `a806db1ec4ea162b` (N-2, truncate in place) and `f0d4a3093cbb9b63` (M-2).

Still open: optional bug files for N-1, N-3, N-4, N-5, N-6 (listed in the research doc).

Plan D scout written 2026-11-14: `docs/research/2026-11-14-artifact-identity-plan-d-scout.md` (id `489339eef5f9e93b`). Findings D-1..D-8: the shadow diff needs an `--old-root` option; the dual-write library takes an old-catalog write transaction (bodies not read); events, observations and links have no export target until C2; the rename step needs a citation-rewriting rename (N-5); the citation rewrite must read file text, not `entry_cite.dst_ref`; the build reads a commit, so the tool has a write phase, an operator commit, then a build phase. Three questions wait for the operator (C2 before D or split D; who commits; file-only variant or scratch copy). Next: the operator's answers, then the Plan D plan, then C2 and C3.

## Rules that bite when resuming

- A full gate runs only on an UNTOUCHED tree (no implementer holds an open round); see session-log F-16.
- `TOOL_SURFACE_CHAR_BUDGET` is exactly 57_664 with zero headroom: any advertised description or schema edit is character-neutral or smaller, or re-measured with a log entry.
- SDD ledgers are git-ignored inside the worktree and die with it; the research doc holds the committed copy of rulings.
- Never merge into `experiments` or push without the user's answer. Landing route: merge `experiments` into the branch in its worktree, gate that tree, check peers' staged files, `git merge --ff-only` in the shared checkout, `scripts/worktree-remove.sh`, `git branch -d`.
- Session log id `abff63149e3c24e8`; B1/B2 rulings docs `8721253ab393c824`, `4c82d96fc9952e98`.
