---
id: '38a17e4acf1f1fa1'
kind: tracker
status: draft
title: Structural Debt — post-690-commit refactor stream (SD-N)
owners:
- marius
tags:
- refactoring
- structural-debt
- code-smell
- librarian
- audit-doc-refs
topic: refactoring
---

## Why this exists

The `experiments` branch carries 690 commits ahead of `master` — 647 files,
+111,476 / −6,311; in `src/` + `crates/` alone 203 files and +44,070 / −4,966.
The mass landed in one subsystem: `src/librarian/` roughly doubled, adding
`src/librarian/catalog/gc.rs` (1244), `src/librarian/tools/merge_worktree.rs`
(970), `src/librarian/catalog/graft.rs` (753),
`src/librarian/tools/link_scan/` (~1260),
`src/librarian/tools/append_entry.rs` (401),
`src/librarian/tools/constitution_check.rs` (381),
`src/librarian/catalog/worktree.rs` (185),
`src/librarian/catalog/entry_cite.rs` (130), while
`src/librarian/tools/doctor.rs` grew by 2454 lines.

That is the youngest, least-settled code on the branch. This tracker holds the
structural findings from reading it — the ones that survive a "name the
structural defect in one sentence" test. It does **not** hold bugs (those are
`docs/issues/`), tool frictions (`U-N`), or plan-vs-reality drift (`F-N`).

**`SD-N` is work-stream-scoped, not a durable taxonomy slot.** Per
`docs/TAXONOMY.md` § *Work-stream-specific prefixes*, a new project-wide prefix
must be earned. If this stream outlives itself, promote it then; until it does,
the prefix lives here and nowhere else.

## Boundary with the sibling trackers

- **`docs/trackers/legibility-backlog.md`** (`cd886c414f6751b4`) owns
  over-budget function bodies ranked by observed usage.db cost. It is written
  by `librarian(action="legibility_scan")`, not by hand. SD entries **cite** it
  and never duplicate its rows. It was last written ~2026-05-09 and is stale;
  a `write=true` scan refreshes it.
- **`docs/issues/`** owns anything with a reproduction and a wrong output.
  An SD entry that turns out to produce wrong behaviour is promoted to a bug
  file and its SD row flips to `superseded` with the bug path recorded.

## Method — the discipline this stream runs under

One transformation per commit; tests green after every single move; behaviour
preserved, period. A new parameter, branch, or output in the diff means it
stopped being a refactor.

**Baseline, recorded before the first move** (`1911af3d`):

- `cargo test --workspace` — **3818 passed / 0 failed / 50 ignored**
- `cargo clippy --all-targets -- -D warnings` — clean
- `cargo fmt` — clean
- `codescout audit-doc-refs --fail-on high` — exit 0 over 945 files

**The baseline's own caveat, which is load-bearing:** 3818 is the count for the
**default feature graph**, not for the repo. `src/dashboard` tests are filtered
out unless `--features dashboard`, and every `#[cfg(feature = "server-stack")]`
test is compiled by no lane at all. A refactor touching either moves code its
baseline does not cover, and the suite stays green throughout. Any SD entry in
those areas must state which lane actually built its tests.

## Findings

_Per-entry detail sections live below as `### SD-N — <title>`. The live table is
rendered from params at the top of the file._

## History

### 2026-08-15 — stream opened

Survey run on `1911af3d` after the backlog drive closed 14 of 15 bugs. Five
findings recorded (SD-1..SD-5). No code touched — the survey was read-only, and
the merge to `master` (a clean fast-forward, 0/690) is on the user's hold.
