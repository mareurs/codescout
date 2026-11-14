# Lifecycle session: state at compaction, 2026-11-14 (read this first after a compaction)

Written by the lifecycle session (SID `31731474-4768-4fd2-b6e9-e4654810de74`, account `.claude-stef`; its name drifts: `codescout-7e`, `-4a`, `-51`). Everything below was read from git or the repo on 2026-11-14 unless it says "not verified". No push anywhere ("nu fa push" is the operator's standing rule).

## Where the code is
- `experiments` (main checkout): `9b1048944`. Its latest commits are docs only (my parked-items memory, my tracker log entries, two bug files) plus a peer's bug record `4c74cc6a5`.
- `integration/identity-lifecycle-20261114` (worktree `.worktrees/integration-identity-lifecycle`, clean): `fddfe01f2` = lifecycle tools part 1 (`18e0ad3c2`, 28 files, gate green on the merged tree: lean 10277, default 10372, python 1836) plus one docs commit (plan `67870a11f0ed7a0e` done, spec `afc72d5722d1d601` active). Four `experiments` commits are NOT in integration yet (`4c74cc6a5`, `fddb14963`, `0a75ed524`, `9b1048944`, all docs).
- The two hooks `dead-artifact-ids` and `key-space` were SKIPPED on the commit `fddfe01f2` (no codescout binary in that worktree). Not verified by them.
- My own worktree `.worktrees/lifecycle-tools-part-1-20261114`, branch `feat/lifecycle-tools-part-1-20261114` and the SDD workspace are removed (branch force-deleted after `is-ancestor` of integration; commits live on in integration). The ledger's content is in `tracker-lifecycle-parked-items`.

## The agreement with the identity session (both operators)
Plan D (identity cutover tool) and lifecycle tools part 1 land on the integration branch. `experiments` gets ONE merge from integration when the operator says all work is done (the operator said so on 2026-11-14: "we will merge in experiments when all work is done"). Each line: merge integration into its branch, gate that tree, announce by message to the other session, `git merge --ff-only` from the integration worktree, announce again. Docs-only commits go on `experiments` and flow into integration by merging `experiments` into it. The final merge route: merge `experiments` into integration in its worktree, gate that tree, then `git merge --ff-only` into `experiments` in the main checkout. Ask the operator before it.

## The other session
Identity session: SID `e4a747a2-e0af-4747-9fc1-a5de0891945a`, name `codescout-5f` (was `-2a`), pid 586661, `.claude-stef`. It builds Plan D (plan `docs/superpowers/plans/2026-11-14-artifact-identity-plan-d-cutover-tool.md`, committed `755630fa0`) subagent-driven in `.worktrees/cutover-tool-20261114`, branch `feat/cutover-tool-20261114`. At 2026-11-14 about 19:30 it was at task 2 (head `7208bc037`) with uncommitted edits in `identity/grammar.rs` and `link_scan/extract.rs`. It has not yet merged integration into its branch (not verified). It announced it would land nothing on `experiments` for Plan D. Another session `codescout-e4` (`5f715d8f-…`) is unrelated to this work. Send messages by name; the SID is the identity, the name is not.
My last two messages to it: the integration branch exists; the fast-forward to `18e0ad3c2` is done. I did NOT yet tell it about `fddfe01f2` or the docs commits on `experiments`.

## Tracker and memory updates done today
- `tracker-organization-session-log` (`ecee2aab906f9df7`): F-4 (a gate-only test missed by ten green task reviews), F-5 (nested-worktree routing, Unix fixed, Windows parked), W-5 (whole-branch review value), F-3 follow-up. Committed `0a75ed524`.
- Parked items: memory `tracker-lifecycle-parked-items` (`fddb14963`).
- Bugs filed: `e620f5f67712182b` (create checks key uniqueness then writes), `24f1f923d02614a6` (Windows spelling gap of the nested-worktree routing). Both `unverified`, both `cluster/unclassified`. Commit `9b1048944`.
- Identity side trackers (`artifact-identity-session-log`, `artifact-identity-state`) belong to the identity session: not touched.

## Open, in order
1. Wait for the identity session's Plan D tasks and its announcements; when it asks to land, check `integration` is where it expects.
2. When the operator says all work is done: the final merge route above, then `git branch -d` for integration and its worktree (`scripts/worktree-remove.sh`), after the operator's word.
3. Plan L-B2 part 2 (tools that address a collection by key, `doc(move)` as a key rename) waits for identity Plan E; not planned. Plan L-E (migration) not written.
4. Parked and unfixed: see `tracker-lifecycle-parked-items` § Parked from tools part 1 (about 30 items, two bugs filed). The Windows limit is the one with a platform risk.
5. Older follow-ups from before: the `$ERR` instrument for `tests/worktree-remove.sh` (not applied); bug `3c919d925fa383fe` (one red gate run, cause unknown).

## Rules that bite (learned today)
- Per-task reviews run no suite: run `scripts/with-slot.sh cargo test --lib no_librarian_production_read_bypasses_the_read_door` when a task adds `OpenOptions`, `fs::read` or `File::open` under `src/librarian/` (F-4).
- Commit on the shared checkout by pathspec with `-m` before `--`; read `git diff --cached --name-only` first (peers stage paths there). `git branch -d` refuses a branch that `experiments` does not hold; `-D` after `is-ancestor` of integration, with the harness `@ack`.
- Find a branch owner by the `Session-Id` trailer on its commits; a guess was wrong once.
- In a worktree session `doc(...)` writes need `workspace=` pinned to the tree; an `@ack_` that the tool offers writes to the MAIN repo: do not take it for a worktree write.
- Message time labels: read `date -u`; do not estimate.
