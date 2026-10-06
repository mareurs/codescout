# Publish Hold Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** An author records "withheld" for their session, and the pre-push guard refuses that session's commits even under `CODESCOUT_PUSH_ACK=all`.

**Architecture:** A hold is a ref `refs/holds/<session-id>` in the shared `.git`, pointing at a blob. `scripts/hold-publish.sh` writes and deletes it. `scripts/pre-push-foreign-session-guard.sh` checks it for every commit in the pushed range before the ack test. The guard's session-liveness function moves to a sourced file so both scripts share it.

**Tech Stack:** bash, git 2.56, python3 (already used by the guard). Tests are bash suites that build throwaway repos.

**Spec:** `docs/superpowers/specs/2026-10-06-publish-hold-design.md`

## Global Constraints

- Ref name is exactly `refs/holds/<session-id>`. The ref points at a blob, never a commit.
- Blob lines, in order: `reason: <text>`, `set-at: <UTC ISO-8601, e.g. 2026-10-06T00:00:00Z>`, `head: <sha at hold time>`.
- `CODESCOUT_PUSH_ACK` (including `all`) never clears a held commit.
- The guard stays silent and exits 0 when `CLAUDE_CODE_SESSION_ID` is empty. Do not change this.
- A hold on a session the guard cannot look up still refuses. It fails closed.
- Trailerless commits are not matched by a hold. The existing note stays.
- No shell script may contain a personal home path (`tests/committed_paths.rs` fails the build). Tests use `mktemp -d` and a temporary `HOME`.
- Every test case asserts the refusal and the silence it must keep (the suite's own rule). Every absence assertion needs a paired positive assertion.
- Commit by pathspec with the separate-call sequence in `docs/conventions/shared-checkout-commit-sequence.md`. Do not push.

## Review Focus

- A commit with two `Session-Id` values (a hand-written second trailer): a hold on the second sid still refuses. Pinned in Task 3.
- A hold whose session has no commit in the pushed range: the push is silent and allowed. Pinned in Task 3.
- A session id that is not a valid ref component (`..`, spaces, `/`): `set` refuses it and creates no ref. Pinned in Task 2.
- The guard runs inside a linked `git worktree`: it still sees the hold. Pinned in Task 3.
- The `resolve-sids.sh` file is missing next to the guard: the guard still refuses and prints the bare sid. Pinned in Task 1.

---

### Task 1: Move `resolve_sids` to a sourced file

**Files:**
- Create: `scripts/resolve-sids.sh` (defines only `resolve_sids`)
- Modify: `scripts/pre-push-foreign-session-guard.sh` (the `resolve_sids() { ... }` definition, with its comment block, around lines 451-505)
- Test: `tests/pre-push-foreign-session-guard.sh`

**Interfaces:**
- Produces: `resolve_sids <sid>...` prints one line per sid: `<sid>\t<LIVE|gone|?>\t<uds:socket or empty>`. Behaviour is unchanged from today.

- [ ] **Step 1: Record the baseline.** Run `bash tests/pre-push-foreign-session-guard.sh`. Write the final PASS and FAIL counts in the commit message later. Expected: FAIL 0.
- [ ] **Step 2: Write the failing test** `a guard without resolve-sids.sh beside it still refuses and prints the bare sid`. Copy the guard alone into `mktemp -d`, set `GUARD` to the copy, build the "another session's commit" repo from the existing section, and run it. Assert: exit 1, output has `$BOB`, output hasnt `LIVE`. It must fail now only if the guard cannot run without the new file, so also assert the original guard still passes. Expected today: PASS (the function is inline), so this step records the intended contract. Re-run it after Step 4.
- [ ] **Step 3: Create `scripts/resolve-sids.sh`.** Move the `resolve_sids` function and its comment block verbatim. Add a header comment naming the two callers.
- [ ] **Step 4: In the guard, replace the definition** with: source `"$(dirname "${BASH_SOURCE[0]}")/resolve-sids.sh"` when readable, otherwise define `resolve_sids() { printf '%s\t?\t\n' "$@"; }`. The fallback keeps the guard's "degrades, never a wrong answer" contract.
- [ ] **Step 5: Verify the shim runs the repo copy.** Read `install_shim` in `scripts/install-hooks.sh` and confirm the shim executes `scripts/pre-push-foreign-session-guard.sh` from the repo, not a copy. If it copies, stop and report. Run `bash tests/install-hooks-check-population.sh`. Expected: unchanged result.
- [ ] **Step 6: Run both suites.** `bash tests/pre-push-foreign-session-guard.sh`. Expected: PASS count = baseline + the new assertions, FAIL 0.
- [ ] **Step 7: Commit** the three files by pathspec. Subject: `refactor(scripts): resolve_sids moves to a sourced file so hold-publish can share it`.

### Task 2: `scripts/hold-publish.sh` and its suite

**Files:**
- Create: `scripts/hold-publish.sh`, `tests/hold-publish.sh`
- Modify: `.github/workflows/ci.yml` (add `- run: bash tests/hold-publish.sh` to the `push-guard-tests` job, beside the guard suite)

**Interfaces:**
- Consumes: `resolve_sids` from Task 1.
- Produces, as the CLI contract the guard and docs rely on:
  - `hold-publish.sh set [reason...]` exit 0. Exit 2 with a message on stderr when `CLAUDE_CODE_SESSION_ID` is empty or is not `[A-Za-z0-9-]+`. A second `set` replaces `reason:` and `head:` and keeps the first `set-at:`.
  - `hold-publish.sh release [sid]` exit 0 always. No argument means the caller's own sid. For another sid it prints a line containing `another session` and that sid. For an absent hold it prints `no hold`.
  - `hold-publish.sh list` prints one row per `refs/holds/*`: `<sid>\t<LIVE|gone|?>\t<age>\t<reason>`. It prints nothing when there are none.
  - Exported helper for the guard: none. The guard reads the ref and blob directly.

- [ ] **Step 1: Write the suite** `tests/hold-publish.sh`, using the same `ok`/`no`/`eq`/`has`/`hasnt` helpers and throwaway-repo idiom as `tests/pre-push-foreign-session-guard.sh`. Cases, each as its own named assertion:
  - `set without a session id exits 2 and creates no ref`.
  - `set with an invalid session id (../x, a b, a/b) exits 2 and creates no ref`.
  - `set creates refs/holds/<sid> as a blob` with `git cat-file -t` equal `blob`, and the blob has the three lines in order with the given reason.
  - `a second set keeps the first set-at and replaces reason` (sleep 1.1 between the two calls).
  - `release with no argument removes the caller's own hold`.
  - `release of another sid removes it and says another session` with the sid in the output.
  - `release of an absent hold exits 0 and says no hold`.
  - `list is empty with no holds`.
  - `list marks a session with no registry row as ?` and `a row whose pid is dead as gone`. Point `HOME` at a temp dir holding `.claude-test/sessions/x.json` with `sessionId`, `pid` 999999, a nonexistent `messagingSocketPath` and `procStart` `1`.
- [ ] **Step 2: Run it and confirm it fails** because the script is missing. Run: `bash tests/hold-publish.sh`. Expected: FATAL not executable.
- [ ] **Step 3: Implement `scripts/hold-publish.sh`.** It acts on the repo of the current directory. It writes the blob with `git hash-object -w --stdin` and the ref with `git update-ref`. It sources `resolve-sids.sh` for `list`, with the same missing-file fallback as the guard.
- [ ] **Step 4: Run the suite.** Expected: all PASS, FAIL 0.
- [ ] **Step 5: Add the CI line and commit** the three files by pathspec. Subject: `feat(scripts): hold-publish records a per-session publish hold in refs/holds`.

### Task 3: The guard refuses held sessions' commits

**Files:**
- Modify: `scripts/pre-push-foreign-session-guard.sh` (the per-commit loop near `while IFS=$'\x1f' read -r sha sid subject`; a new block after the outer `done`, before the untrailered note)
- Test: `tests/pre-push-foreign-session-guard.sh` (new section `== a publish hold ==`)

**Interfaces:**
- Consumes: `refs/holds/<sid>` and its blob from Task 2; `resolve_sids` from Task 1.
- Produces: a refusal on stderr, exit 1, containing the marker `PUBLISH HOLD`, then for each held sid: the sid, its `reason:` text, its age, its state (`LIVE`, `gone`, `?`), and the literal command `scripts/hold-publish.sh release <sid>`. It also prints either a `git push origin <sha>:<branch>` line for the prefix below the oldest held commit, or the sentence `nothing below the held commit is unpublished`.

- [ ] **Step 1: Write the failing cases** in the new section. Each builds a repo with `new_repo`/`commit`, creates the hold with `git update-ref refs/holds/$BOB "$(printf 'reason: waiting\nset-at: 2026-10-06T00:00:00Z\nhead: x\n' | git hash-object -w --stdin)"`, and uses `run`:
  - `a held session's commit is refused under ack=all`: exit 1, has `PUBLISH HOLD`, has `$BOB`, has `waiting`.
  - `the refusal names the release command`: has `hold-publish.sh release $BOB`.
  - `without the hold, the same push passes under ack=all`: exit 0 (positive control for the case above).
  - `a hold on one session does not refuse another's commits`: hold on `$CAROL`, push carries only ALICE and BOB: BOB's follows the ordinary path, output hasnt `PUBLISH HOLD`.
  - `the pusher's own held commit is refused and says to release first`.
  - `a hold with no commit of that session in the range is silent`: exit 0 and empty output when pushing only ALICE's commits.
  - `a commit carrying two Session-Id values is refused when the second is held`: build it with a commit message that has `Session-Id: $ALICE` and `Session-Id: $BOB` in the final trailer block.
  - `the prefix below the oldest held commit is named, and a push of it is allowed`: stack ALICE, BOB(held), ALICE; the output has a `git push origin <sha-of-first-ALICE>:main` line; running the guard with that sha as the pushed ref exits 0.
  - `the hold is visible from a linked worktree`: `git worktree add` a second checkout, run the guard there, expect refusal.
  - `the hold survives git commit --amend` and `a git rebase of the held commit`: after each, the guard still refuses. Use a `core.hooksPath` temp dir whose `prepare-commit-msg` execs `scripts/prepare-commit-msg-session-id.sh "$@"`, commit as ALICE, amend with `CLAUDE_CODE_SESSION_ID=$BOB`, and assert the trailer is still ALICE (`git log -1 --format='%(trailers:key=Session-Id,valueonly)'`).
  - `a plain push and push --all do not send refs/holds`: add a bare remote, push both ways, assert `git -C <remote> for-each-ref refs/holds` is empty.
  - `an ack naming a held sid says a hold is not cleared`: ack is `$BOB`, hold on BOB: output has `PUBLISH HOLD` and the sentence from Step 4.
  - `a held session whose lookup fails still refuses`: run with `HOME` pointing at an empty temp dir, so `resolve_sids` yields `?`. Expect exit 1 and output has `?`, not `LIVE`.
  - `an unreadable hold store degrades with one warning line and never claims a hold`: make the hold lookup fail with a `PATH` shim named `git`: it exits 128 for `rev-parse -q --verify refs/holds/*` and passes every other call through. A missing ref exits 1 under `-q`, so 128 is distinguishable from "no hold". Expect the push result to equal the no-hold result and stderr to contain exactly one line with `could not read refs/holds`.
  - `negative control: the new block deleted from a copy lets the held push through`. Assert the markers `# BEGIN PUBLISH HOLD` and `# END PUBLISH HOLD` each appear exactly once in the guard, delete the lines between them from a temp copy with `sed`, run the first case against the copy, expect exit 0. Without the marker-count assertion the `sed` could silently do nothing.
- [ ] **Step 2: Run the suite and confirm the new cases fail** for the right reason (no refusal text). Run: `bash tests/pre-push-foreign-session-guard.sh`.
- [ ] **Step 3: Implement the per-commit check** between `# BEGIN PUBLISH HOLD` and `# END PUBLISH HOLD` markers. Signature of the helper: `held_sid_of <comma-separated-sids>` prints the first sid that has `refs/holds/<sid>`, or nothing. In the loop, when it prints a sid, append a `held_report` row (`    <sha8>  <sid>  <subject>`), record the sid in `held_sids`, record the oldest held sha, the branch name (`remote_ref` without `refs/heads/`) and `remote_sha`, then `continue` before the ack test and before the `mine_n` count.
- [ ] **Step 4: Implement the refusal block** after the outer loop. Decisions the implementer cannot infer:
  - It runs before the untrailered note and before the ack notes, and it exits 1.
  - If the hold lookup itself fails (git exits non-zero for a reason other than "no such ref"), print one line `could not read refs/holds` once and treat the commit as not held. Never print a hold claim from a failed lookup.
  - Reason, age and `set-at` come from `git cat-file -p refs/holds/<sid>`. Age is computed from `set-at` and shown in hours or days.
  - State comes from `resolve_sids`.
  - The prefix line uses `git rev-parse -q --verify "<oldest-held-sha>^"` as the parent. Print the push line only when `git rev-list --count "<remote_sha>..<parent>"` is greater than 0 (or `remote_sha` is the zero sha). Otherwise print `nothing below the held commit is unpublished`.
  - When `$ack` is `all` or contains a held sid (compare the lowercased, whitespace-stripped value, as `acked` does, but without calling it, because `acked` records matches), add one line: an ack does not clear a hold.
  - Heredoc rule from the guard's own history: no backtick and no unescaped `$(` in any unquoted heredoc text. Use `printf '%s'` lines instead.
- [ ] **Step 5: Run the suite.** Expected: all new cases PASS and FAIL 0.
- [ ] **Step 6: Update the guard's header.** The paragraph "It covers the pusher only ... that half is open" must now say the author half is a hold and name `scripts/hold-publish.sh`. Keep the trailerless-commit paragraph unchanged.
- [ ] **Step 7: Commit** the two files by pathspec. Subject: `feat(guard): a held session's commits are refused past CODESCOUT_PUSH_ACK=all`.

### Task 4: Docs, spec amendment and the full gate

**Files:**
- Modify: `docs/conventions/shared-checkout-commit-sequence.md` (section *A session that cannot publish must not commit*), `docs/RELEASE.md` (the bullet added 2026-10-06), `docs/issues/2026-09-06-a-push-publishes-commits-their-author-was-withholding.md` (`## Fix`, `## Resume`), `docs/superpowers/specs/2026-10-06-publish-hold-design.md`

- [ ] **Step 1: Edit the convention section** (heading-addressed edit; the file is catalog-managed). Replace the **Status** paragraph: the policy now has a mechanism. Name `scripts/hold-publish.sh set`, the guard's refusal, the existing pre-push guard, and the limits (trailerless commits, session granularity, release is not proof of an operator).
- [ ] **Step 2: Edit the `docs/RELEASE.md` bullet** to add one sentence with the `set` and `release` commands and that releasing another session's hold is an operator decision.
- [ ] **Step 3: Edit the bug file.** In `## Fix`, record the mechanism and its commit SHAs cited as SHA plus patch-id (see `docs/RELEASE.md` § *Citing a fix*). In `## Resume`, remove the first two decisions and keep the pusher-side paragraph. Do not change `status: open` to closed yet: the commit-side gap remains. Add the frontmatter note only through `doc(action="update")`.
- [ ] **Step 4: Amend the spec** in two places: the prefix line is computed directly from the oldest held commit's parent, and `scripts/resolve-sids.sh` is in "Files touched".
- [ ] **Step 5: Run `./scripts/gate.sh`.** Expected: `GATE EXITS -> FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0`.
- [ ] **Step 6: Live check on a throwaway clone** with two session ids: `set` as one, push as the other with `CODESCOUT_PUSH_ACK=all` and expect refusal, `release`, push and expect success. Record the output in the bug file's `## Fix`.
- [ ] **Step 7: Commit** the docs by pathspec. Subject: `docs: the publish hold - convention, release bullet and bug file record the mechanism (d9d291b44775e50d, OB-20)`.
