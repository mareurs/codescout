# Atomic catalogued write door Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `write_catalogued_file` replaces a document atomically (staging file, then rename), so a failed or killed write leaves the old bytes and never a cut document.

**Architecture:** The door in `src/librarian/util.rs` keeps its two refusals (a symbolic link, a path outside the repository) and then calls the existing `crate::util::fs::atomic_write` in place of `std::fs::write`. A writability probe keeps the old refusal of a read-only file. The Plan C2 stopgap sentence "may be partly written" is removed, because it is false after this change. An inventory step finds librarian writers that do not use the door.

**Tech Stack:** Rust, `std::fs`, the existing `util::fs::atomic_write` (exclusive staging file, mode kept, setuid and setgid dropped, staging file removed on failure).

**Spec:** bug `a806db1ec4ea162b` (`docs/issues/2026-11-13-write-catalogued-file-truncates-in-place-so-a-failed-write-loses-the-ledger.md`) and the identity spec `docs/superpowers/specs/2026-11-11-repo-relative-artifact-key-design.md` (v5: committed files are the only store, so a write must not lose a file). Precondition of Plan D (scout `489339eef5f9e93b`, finding D-8; execution state `bf5a4e67591b53e0`, decision I-4).

## Global Constraints

- The door keeps the order of its guards: symbolic-link refusal first, outside-repository refusal second, then the write. A refusal still writes nothing.
- The door still returns `anyhow::Result<()>`. A refusal stays a `LibrarianRecoverableError`. An I/O error stays an `io::Error` inside the `anyhow::Error`.
- No second atomic-write implementation: call `crate::util::fs::atomic_write`. Change that function only if a test shows a gap, and say so in the report.
- A read-only target (no write permission for the writer) is still refused with the same kind of error as before, and the file is unchanged.
- No text that the door or its callers show holds an absolute path (use `without_path` and `document_label`, as the callers do).
- `TOOL_SURFACE_CHAR_BUDGET` is exactly 57_664 (headroom 0). No advertised description or schema changes.
- Write plan, guide and tracker text in Simplified Technical English. Shared-checkout commit rules: stage paths, run `git diff --cached --name-only` as its own call, commit by pathspec with `-m` before `--`.

## Rulings (controller; the operator may overrule any of them)

- **A-R1** No `fsync` is added. `atomic_write` does not call it, and the edit tools use it the same way. The change closes the failures the bug names (a full disk, a killed process). It does not claim power-loss durability. Cost if wrong: a power loss right after a write can leave an empty file on some filesystems.
- **A-R2** (revised after the plan review) The door checks writability of an existing target before it replaces it, so a read-only file stays refused (a rename would replace it). The check is a function `refuse_unwritable_existing(path: &Path) -> std::io::Result<()>` in `src/util/fs.rs` beside `atomic_write`, not in `src/librarian/util.rs`: the test `no_librarian_production_read_bypasses_the_read_door` (`src/librarian/read_door.rs:1414`) counts any production `OpenOptions` under `src/librarian` as a raw read, and the check is a write-only open. Behaviour: a missing target gives `Ok` (the door creates files, for example `augmentation_sidecar::write` and `repo_name::write`); an existing target is opened for writing WITHOUT create or truncate, with `O_NONBLOCK | O_NOFOLLOW` on Unix (a FIFO must not block, a link planted after the leaf check must not be followed); a target that is not a regular file is refused with a path-free `io::Error`; any other open error is returned unchanged (`EACCES` for mode 0444, `EISDIR` for a directory, `EROFS`, `EPERM`). A root process is admitted, as before. `access(W_OK)` is not used (it checks the real uid). Cost if wrong: one extra `open` per write.
- **A-R3** If every caller passes text, the door takes `impl AsRef<str>`. If a caller passes bytes that are not text, add a sibling that writes bytes. The compiler decides; report which.
- **A-R4** The Plan C2 stopgap (`PARTLY_WRITTEN`, `write_failed_in_io`, and the sentence in the `event_create` and `doc(link)` write errors) is removed. After this change an `io::Error` from the door means the document is unchanged. `with_sentence` stays, because `note_destination_change` uses it.
- **A-R5** A process killed between the staging create and the rename leaves `<name prefix>.<32 hex>.tmp` beside the document. It is not indexed (the indexer takes `.md` only) and `.tmp` is not in `.gitignore`. A reaper is not built here. Cost if wrong: an untracked file in `git status` after a crash.
- **A-R6** The inventory (Task 3) files a bug for each librarian writer that rewrites a document outside the door. It fixes none of them in this plan.
- **A-R7** (plan review F-18) An atomic replace needs write and search permission on the DIRECTORY; the old in-place write needed only the file's permission. After this change a librarian write to a writable document in a read-only directory is refused, a bind-mounted document fails with `EBUSY`, the write needs free space equal to the document, and the new file belongs to the writer (owner, group, ACLs and xattrs are not kept; a hard link is broken). Accepted. Cost if wrong: such a document cannot be updated by the librarian until its directory is writable. The test that pins it is `a_write_in_a_read_only_directory_is_refused_and_leaves_the_old_bytes`.
- **A-R8** (plan review finding 5) The door now inherits two effects of `atomic_write`: on Linux `observe_write` (`src/util/fs.rs` ~L431-450) records target facts, so `doc` writes emit `dispatch-observed` facts, and a `doc` call refused after an attempted write is recorded as `error-possible-effects` and not `refused` (`observations/server.rs` ~L338). Accepted as correct; no code change.
- **A-R9** (plan review finding 7) A reader that opens a document at the instant a peer replaces it can see `read_verified` refuse with "it changed while it was being opened (a link was replaced…)" (`read_door.rs` ~L445-497). `edit_file` already causes this; now any librarian write does. The window is microseconds. Accepted and named. The Windows watcher bug `c77c2a6e63c3fe86` (a watcher misses a rename) now also applies to documents.
- **A-R10** (plan review finding 9) Two `atomic_write` errors spell the path (`no file name`, `16 staging attempts`, `src/util/fs.rs` ~L349-375). They are practically unreachable. `event_create` and `doc(link)` scrub the document's path; `update`, `mv` and `rekey` pass the error up unscrubbed (unchanged by this plan). Accepted; bug `3f0dfd4524d7164f` covers the class.
- **A-R11** (plan review, operator questions answered by the controller) The probe lives in `src/util/fs.rs`. Task 3 covers `src/librarian/` and also the `.md` writers in `src/tools/onboarding.rs`, `src/librarian/tools/create.rs` and the `fs::rename` in `src/librarian/tools/mv.rs`. A write-side source gate (like the read door's gate) is the structural remedy for the class; it is named as a follow-up and not built here.

## Review Focus

1. A failed write: the staging file cannot be created (a read-only directory) or its write fails. The old bytes must stay and no staging file may remain (Task 1). This case is a lost capability, not only a safe failure: see A-R7.
2. A read-only target file. It must still be refused, and the bytes must not change (Task 1).
3. The target's mode (an exec bit, a private mode) must survive the replace. A symbolic link at the target must still be refused before any write. A FIFO at the path must be refused without blocking. A missing target must still be created, with no staging file left (Task 1).
4. After the change, no error text from `event_create` or `doc(link)` says the document may be partly written, and a read-only document still gives an error that records no row (Task 2).
5. A librarian writer that rewrites a catalogued document outside the door must be named in a bug file, not left unknown (Task 3).
6. The source gate `no_librarian_production_read_bypasses_the_read_door` must stay green (the probe lives in `src/util/fs.rs`, A-R2) and the five tests that force a write failure with mode 0444 must keep failing at the probe (Task 1, Step 4).

---

### Task 1: The door writes atomically

**Files:**
- Modify: `src/util/fs.rs` (add `refuse_unwritable_existing`, A-R2)
- Modify: `src/librarian/util.rs` (`write_catalogued_file`; the doc comments of `write_catalogued_file` and `refuse_write_through_symlink`)
- Modify: call sites only if the signature changes (A-R3; the review found all 19 call sites pass text, so `impl AsRef<str>` holds)
- Test: the tests module of `src/util/fs.rs` (the probe) and of `src/librarian/util.rs` (the door)

**Interfaces:**
- Consumes: `crate::util::fs::atomic_write(path: &Path, content: &str) -> std::io::Result<()>`.
- Produces:
  - `pub(crate) fn refuse_unwritable_existing(path: &Path) -> std::io::Result<()>` in `src/util/fs.rs` (behaviour in A-R2).
  - `pub(crate) fn write_catalogued_file(roots: &[PathBuf], path: &Path, contents: impl AsRef<str>) -> anyhow::Result<()>` in `src/librarian/util.rs`: symbolic-link refusal, outside-repository refusal, `refuse_unwritable_existing`, then `atomic_write`, in that order.

- [ ] **Step 1: Write the failing tests** (each on a real file under a `TempDir`; skip the permission cases when the process ignores file modes, as the existing read-only tests do). In `src/util/fs.rs`:
  - `an_unwritable_existing_file_is_refused_with_its_own_error`: mode 0444, the result is `PermissionDenied`, the bytes are unchanged and nothing is created.
  - `a_missing_path_is_fine_and_is_not_created_by_the_check`.
  - `a_fifo_at_the_path_is_refused_without_blocking` (create it with `mkfifo`; run the check under a short thread timeout so a block fails the test instead of hanging it) and `a_directory_at_the_path_is_refused`.
  - `a_symlink_at_the_path_is_not_followed_by_the_check` (`ELOOP` or the type refusal; the target is unchanged).
  
  In `src/librarian/util.rs`:
  - `a_write_replaces_the_document_and_leaves_no_staging_file`: the new text is there and the directory holds no `*.tmp` file.
  - `a_missing_target_is_created_and_leaves_no_staging_file` (`augmentation_sidecar::write` and `repo_name::write` create files through this door).
  - `a_write_in_a_read_only_directory_is_refused_and_leaves_the_old_bytes` (directory 0555, file writable; the call errors, the file is byte-identical, no `*.tmp` exists). This pins the new behaviour of A-R7; it is RED against the current code, which truncates through the file's own permission.
  - `a_read_only_document_is_still_refused_and_unchanged`: file mode 0444; the error is a `PermissionDenied` `io::Error` inside the `anyhow::Error`; the bytes are unchanged. Mutation named: a bare `atomic_write` would replace the file and fail this test.
  - `the_document_keeps_its_exec_bit_and_a_private_mode` (0755 and 0600).
  - `a_symlink_at_the_target_is_refused_before_any_write` and `a_path_outside_the_roots_is_refused_before_any_write` (the existing refusals; the link's target is unchanged).
  - `a_directory_at_the_path_is_an_error_and_nothing_is_replaced`.
- [ ] **Step 2: Run** `scripts/with-slot.sh cargo test --lib util::fs` and `scripts/with-slot.sh cargo test --lib librarian::util`. Expected RED: `a_write_in_a_read_only_directory_is_refused_and_leaves_the_old_bytes` (against the current truncating write, it succeeds), the probe tests (the function does not exist), and `a_write_replaces_the_document_and_leaves_no_staging_file` only if it fails to compile first. State in the report which tests were RED and which were regression guards; do not claim a test was RED that passed.
- [ ] **Step 3: Implement** the probe (A-R2) and the door. Map the `io::Error` into `anyhow` as before. Update both doc comments honestly. The door says the write replaces the file atomically. `refuse_write_through_symlink` drops the sentence that a link planted between the check and the write "still wins" and says what happens now: a link planted in that window is REPLACED by a regular file and the call reports success. The function's own first bullet calls that outcome wrong; say that it is accepted here because the window is two syscalls and the old behaviour (a write through the link) was worse. Keep what is still true (the leaf check only).
- [ ] **Step 4: Run** the two commands of Step 2, then `scripts/with-slot.sh cargo test --lib read_door` (the source gate `no_librarian_production_read_bypasses_the_read_door` must be green), `... cargo test --lib dual_write`, `... cargo test --lib tools::link` (this filter also runs `link_dir_tests`, where `doc_move_completes_and_reports_when_only_the_id_repair_write_fails` needs the probe: without it a 0444 file in a writable directory would be replaced), `... cargo test --lib tools::mv`, `... cargo test --lib tools::update`, `... cargo test --lib event_create`, `... cargo test --lib augmentation`, `... cargo test --lib rekey`, `... cargo test --lib doctor`, `... cargo test --lib identity`, and `scripts/with-slot.sh cargo test --lib tools::onboarding` (its tests at ~L2538 and ~L2563 write `repo.toml` through the door in a 0o500 directory; staging fails with `EACCES` and the file stays absent). Expected: PASS. The Task 2 "partly written" tests still pass here, because their read-only cases fail at the probe with an `io::Error`. If another test fails, stop and report which and why.
- [ ] **Step 5: Commit** by pathspec: `fix(librarian): write_catalogued_file replaces the document atomically (a806db1ec4ea162b)`.

### Task 2: Remove the partial-write stopgap

**Files:**
- Modify: `src/librarian/catalog/dual_write.rs` (remove `PARTLY_WRITTEN`, `write_failed_in_io` and their tests; keep `with_sentence`)
- Modify: `src/librarian/tools/event_create.rs`, `src/librarian/tools/link.rs` (the write-error mapping)
- Test (the tests that assert the sentence, found by the plan review): `a_failed_io_write_says_the_document_may_be_partly_written` (`event_create.rs` ~L1586; rename it), `a_failed_file_write_is_an_error_and_records_no_row` (`link.rs` ~L1117), `only_an_io_error_of_the_write_itself_counts_as_a_possible_partial_write` and the `PARTLY_WRITTEN` lines of `a_sentence_joins_a_recoverable_error_and_keeps_its_hint` (`dual_write.rs` ~L909-952). NOT in this list: `a_write_failure_after_the_destination_write_names_the_destination_change` (`link.rs` ~L1270) does not assert the sentence and stays as it is; `a_failed_file_write_leaves_no_row` (`event_create.rs` ~L1572) asserts the absence of the sentence and stays valid.

**Interfaces:**
- Consumes: Task 1's door.
- Produces: write errors of both writers without the partial-write sentence. The error still names the document by label, holds no absolute path and records no row. The supersedes destination sentence from `note_destination_change` stays.

- [ ] **Step 1: Change the tests first.** Rewrite each test that asserts the sentence so it asserts the new truth: the error text does not contain `partly written`, the document bytes are unchanged, no row exists, no path appears. Run `scripts/with-slot.sh cargo test --lib event_create` and `... cargo test --lib tools::link`. Expected: FAIL (the sentence is still produced).
- [ ] **Step 2: Remove** `PARTLY_WRITTEN`, `write_failed_in_io`, the `partial` branch in both writers, and the helper tests in `dual_write.rs` (`only_an_io_error_of_the_write_itself_counts_as_a_possible_partial_write`, and the `PARTLY_WRITTEN` lines of `a_sentence_joins_a_recoverable_error_and_keeps_its_hint`, which still tests `with_sentence`). Keep every path-scrubbing step.
- [ ] **Step 3: Run** `scripts/with-slot.sh cargo test --lib dual_write`, `... cargo test --lib event_create`, `... cargo test --lib tools::link`, and `scripts/with-slot.sh cargo clippy --lib --no-default-features -- -D warnings` (clean, no `dead_code`). Expected: PASS.
- [ ] **Step 4: Commit** by pathspec: `refactor(librarian): the partial-write stopgap is gone; a failed write leaves the document unchanged`.

### Task 3: Inventory of writers outside the door

**Files:**
- Create: bug files in `docs/issues/` (copy `docs/issues/_TEMPLATE.md`; one file per distinct writer; use `doc(action="create", kind="bug", …, extra={opened, severity, unverified})` as the C2 bug files did)
- Modify: none in `src/`

- [ ] **Step 1: Find** every production site that rewrites a catalogued or frontmatter-declared document, or a markdown document the tools produce, and does not go through `write_catalogued_file` or `atomic_write`. Search `src/librarian/`, `src/cli/` and these named files: `src/tools/onboarding.rs` (`std::fs::write` of prompt and synthesis `.md` files at ~L552, ~L614, ~L623), `src/librarian/tools/create.rs` (~L569, a plain `std::fs::write` after an existence check, not `create_new`) and `src/librarian/tools/mv.rs` (~L372, `std::fs::rename` that overwrites a destination that appears after the check). Patterns: `fs::write`, `File::create`, `OpenOptions`, `.write_all(`, `fs::rename`, `fs::copy`, and calls to a helper that does any of these. Separate production code from tests by symbol, not by the line after `#[cfg(test)]` (many inline test modules do not start there): use `symbols(path)` to see which module a line is in. The plan reviewer's scratch script `/home/marius/.cache/cc-tmp/claude-1000/-home-marius-work-claude-codescout/e4a747a2-e0af-4747-9fc1-a5de0891945a/scratchpad/writers.py` is a starting point, not a result. Sites the reviewer already found (verify each, do not copy): `create.rs:569`, `mv.rs:372`, `catalog/audit/shard.rs:628` (append-mode audit shard, not a document), `workspace.rs:128` (`create_new`, safe), `onboarding.rs:552/614/623` (markdown), `onboarding.rs:140/1600/1813` (toml config), `tools/symbol/edit_code.rs:865/868` (source files, out of scope). List each as `path:line`, what it writes, and whether it can truncate or overwrite a document that git does not hold (a freshly created file is not at risk).
- [ ] **Step 2: For each writer that can lose a committed document,** file a bug (summary, class fit, symptom, root cause with `path:line`, fix idea: route through the door or `atomic_write`, severity). Do not fix any. Set `unverified` honestly: read from the code, not reproduced.
- [ ] **Step 3: Report** the list in the task report, including the sites you judged safe and why. If none exists, say so with the search you ran.
- [ ] **Step 4: Commit** the bug files by pathspec: `docs(bug): librarian writers that rewrite a document outside the write door`.

### Task 4: Gate (controller)

- [ ] **Step 1:** Run `./scripts/gate.sh` in the background on an untouched tree and end the turn. Expected: lean and default nextest green, python green, fmt and clippy clean.

## Self-review

1. **Coverage.** The bug's fix (temp file and rename): Task 1. Its test ("a failing write leaves the old bytes"): Task 1. The false stopgap: Task 2. The class (other writers): Task 3. Power-loss durability: ruling A-R1, not claimed.
2. **Step scan.** Each step names the command, the files and the expected result. Bodies are left to the implementer except the probe rule, which the plan fixes (A-R2).
3. **Types.** `write_catalogued_file` keeps one signature across tasks (A-R3 may change `contents`). `with_sentence` stays in Task 2.
4. **Review Focus.** Lines 1-3 map to Task 1 tests, line 4 to Task 2, line 5 to Task 3.
5. **Proportion.** Three small tasks plus the gate. The door is 4 lines today; most of the work is tests and removal of the stopgap.

## Facts verified by the plan review, and what is still open

Verified by an independent Opus review (session log entries F-17, F-18, W-11 in `docs/trackers/artifact-identity-session-log.md`): all 19 call sites pass text (A-R3 holds); `atomic_write` has no `fsync`, creates its staging file exclusively, keeps the mode without setuid and setgid, and removes the staging file when the write or the rename fails; a hard link is broken by the rename; `create_staging_file` fails at once in a read-only directory and creates nothing; the five mode-0444 tests all go through the door and fail at the probe; `with_sentence` stays in use after Task 2; `partly written` appears only in `dual_write.rs`, `event_create.rs` and `link.rs`; no in-process file watcher exists; no `.gitignore` rule or indexer path reads a `*.tmp` staging file.

Still open (the implementer checks):
- The set of writers in Task 3 is the reviewer's search, not a final list.
- That `libc` is available for `O_NONBLOCK | O_NOFOLLOW` in `src/util/fs.rs` (`src/librarian/read_door.rs` already uses `O_NONBLOCK`).
- On Windows, a rename onto a file an editor holds open can fail. Not tested here; named so a Windows run is read with it in mind.
