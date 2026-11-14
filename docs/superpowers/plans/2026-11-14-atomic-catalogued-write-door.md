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
- **A-R2** The door probes writability by opening an existing target for writing without truncation (`OpenOptions::new().write(true).open(path)`, dropped at once). Reason: a rename replaces a read-only file, and the old `fs::write` refused it. The probe keeps that refusal. Cost if wrong: one extra `open` per write.
- **A-R3** If every caller passes text, the door takes `impl AsRef<str>`. If a caller passes bytes that are not text, add a sibling that writes bytes. The compiler decides; report which.
- **A-R4** The Plan C2 stopgap (`PARTLY_WRITTEN`, `write_failed_in_io`, and the sentence in the `event_create` and `doc(link)` write errors) is removed. After this change an `io::Error` from the door means the document is unchanged. `with_sentence` stays, because `note_destination_change` uses it.
- **A-R5** A process killed between the staging create and the rename leaves `<name prefix>.<32 hex>.tmp` beside the document. It is not indexed (the indexer takes `.md` only) and `.tmp` is not in `.gitignore`. A reaper is not built here. Cost if wrong: an untracked file in `git status` after a crash.
- **A-R6** The inventory (Task 3) files a bug for each librarian writer that rewrites a document outside the door. It fixes none of them in this plan.

## Review Focus

1. A failed write (staging file cannot be created in a read-only directory; the staging write fails). The old bytes must stay and no staging file may remain (Task 1).
2. A read-only target file. It must still be refused, and the bytes must not change (Task 1).
3. The target's mode (an exec bit, a private mode). It must survive the replace, and a symbolic link at the target must still be refused before any write (Task 1).
4. After the change, no error text from `event_create` or `doc(link)` says the document may be partly written, and a read-only document still gives an error that records no row (Task 2).
5. A librarian writer that rewrites a catalogued document outside the door must be named in a bug file, not left unknown (Task 3).

---

### Task 1: The door writes atomically

**Files:**
- Modify: `src/librarian/util.rs` (`write_catalogued_file`; the doc comments of `write_catalogued_file` and `refuse_write_through_symlink`)
- Modify: call sites only if the signature changes (A-R3); the compiler lists them
- Test: `src/librarian/util.rs` tests module

**Interfaces:**
- Consumes: `crate::util::fs::atomic_write(path: &Path, content: &str) -> std::io::Result<()>`.
- Produces: `pub(crate) fn write_catalogued_file(roots: &[PathBuf], path: &Path, contents: impl AsRef<str>) -> anyhow::Result<()>` (or the A-R3 variant), with the same guards in the same order.

- [ ] **Step 1: Write the failing tests** in `util::tests` (each on a real file under a `TempDir`; skip the permission cases when the process ignores file modes, as the existing read-only tests do):
  - `a_write_replaces_the_document_and_leaves_no_staging_file`: after the call the file holds the new text and the directory holds no `*.tmp` file.
  - `a_write_that_cannot_stage_leaves_the_old_bytes`: the directory is mode 0555, the file exists with old bytes; the call returns an error and the file is byte-identical. Mutation named: the truncating `fs::write` would fail this only if the directory allowed it; so also assert the error comes from creating the staging file by checking the old bytes and the absence of a `*.tmp` file.
  - `a_read_only_document_is_still_refused_and_unchanged`: file mode 0444; the call returns a `PermissionDenied` `io::Error` inside the `anyhow::Error`; bytes unchanged. Mutation named: a bare `atomic_write` would replace it and make this fail.
  - `the_document_keeps_its_exec_bit_and_a_private_mode`: modes 0755 and 0600 survive.
  - `a_symlink_at_the_target_is_refused_before_any_write` and `a_path_outside_the_roots_is_refused_before_any_write`: the existing refusal behaviour, with the target of the link unchanged.
  - `a_directory_at_the_path_is_an_error_and_nothing_is_replaced`.
- [ ] **Step 2: Run** `scripts/with-slot.sh cargo test --lib librarian::util`. Expected: the new tests FAIL where they depend on the change (at least `a_write_that_cannot_stage_leaves_the_old_bytes` or `a_read_only_document_is_still_refused_and_unchanged` when run against a draft that calls bare `atomic_write`; against the current code the staging test already passes, so state in the report which tests were RED and which were regression guards).
- [ ] **Step 3: Implement** the door: the two guards, then the writability probe for an existing regular target (A-R2), then `atomic_write`. Map the `io::Error` into `anyhow` as before. Update both doc comments: the door says the write replaces the file atomically and never follows a link at the target; `refuse_write_through_symlink` drops the sentence that a link planted between the check and the write "still wins" (a rename now replaces the link, it never writes through it) and keeps what is still true (the check covers the leaf only).
- [ ] **Step 4: Run** the same command and `scripts/with-slot.sh cargo test --lib dual_write`, `... cargo test --lib tools::link`, `... cargo test --lib event_create`, `... cargo test --lib augmentation_sidecar`, `... cargo test --lib identity`. Expected: the `partly written` tests of Task 2 still pass at this point (their read-only cases fail at the probe with an `io::Error`, as before). If another test fails, stop and report which and why.
- [ ] **Step 5: Commit** by pathspec: `fix(librarian): write_catalogued_file replaces the document atomically (a806db1ec4ea162b)`.

### Task 2: Remove the partial-write stopgap

**Files:**
- Modify: `src/librarian/catalog/dual_write.rs` (remove `PARTLY_WRITTEN`, `write_failed_in_io` and their tests; keep `with_sentence`)
- Modify: `src/librarian/tools/event_create.rs`, `src/librarian/tools/link.rs` (the write-error mapping)
- Test: the tests named `a_failed_io_write_says_the_document_may_be_partly_written` (event_create) and the link tests that assert the sentence (`a_failed_file_write_is_an_error_and_records_no_row`, `a_write_failure_after_the_destination_write_names_the_destination_change`)

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

- [ ] **Step 1: Find** every production site in `src/librarian/` and `src/cli/` that rewrites a catalogued or frontmatter-declared document with `std::fs::write`, `File::create` or `OpenOptions` and not through `write_catalogued_file` or `atomic_write`. Exclude tests (inline `#[cfg(test)]` modules count as tests). List each as `path:line`, what it writes, and whether it can truncate a document that git does not hold (for example a freshly created file is not at risk).
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

## Facts I did not verify (the implementer must check)

- Whether every one of the 19 call sites passes text (A-R3). The compiler answers.
- Whether `atomic_write` keeps a hard-linked document's other link (it does not: the rename gives the path a new inode). Treat as accepted; say so in the report if a test shows otherwise.
- That `create_staging_file` fails cleanly in a read-only directory on this filesystem (a test proves it).
- The set of writers in Task 3. I only grepped `src/librarian` for `fs::write` and the hits were mostly tests.
