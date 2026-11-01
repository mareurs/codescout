# Provenance Policy-Loop Speed-up Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the warm `scan_provenance` of the real corpus measurably faster, with an identical `ProvenanceScan` for every input.

**Architecture:** Two changes in `scripts/file-provenance.py` and nothing else: `normalize` uses a string prefix test instead of `Path.relative_to`, `scan_provenance` reuses `normalize` results within one call, and `write_targets` returns at once for tools that can never write. Equivalence is proved with the old code as an oracle in unit tests, and with an old-versus-new digest comparison on the real corpus in ONE process.

**Tech Stack:** Python 3 standard library, pytest (`tests/`), `scripts/gate.sh`.

**Spec:** `docs/superpowers/specs/2026-11-01-policy-loop-speedup-design.md` (approved 2026-11-01). Bug: `20927ccfe293c34e`.

## Global Constraints

- Output of `scan_provenance(root, environ)` (owners, evidence, gaps, streams) is identical to the old output for every input. Anti-goal: any difference, even in gaps.
- No floor on operations, no lean path, no cache-format change, no parallelism, no whole-result cache.
- Standard library only. No new module, so `EMBEDDED_SCRIPTS` in `src/tools/run_command/attribution.rs` does not change.
- A memo lives for one `scan_provenance` call only. It is never module-level.
- If the measured saving is under 15% of the warm scan, stop and report: the next step needs a new decision.

## Review Focus

- A path with `..` segments that leaves the root and returns: `normalize` must resolve first, then test.
- A sibling directory sharing the root's name as a prefix (`/repo-other` under root `/repo`): must be `None`, not a remainder.
- Root `/`: an absolute path gives the path without its leading slash, as `relative_to` does.
- `base` equal to `UNKNOWN_TREE` with a relative path: the result is `os.path.normpath(p)`, as before.
- A symlink retargeted between two `scan_provenance` calls in one process: the second call sees the new target.

---

### Task 1: `normalize` with a string prefix test

**Files:**
- Modify: `scripts/file-provenance.py` (function `normalize`, about lines 531-549)
- Create: `tests/test_policy_loop_speedup.py`

**Interfaces:**
- Consumes: nothing from earlier tasks.
- Produces: `normalize(p: str, root: Path, base: Path | None = None) -> str | None`, signature and results unchanged.

- [ ] **Step 1: Write the failing test.** In the new file, keep a copy of the OLD body as `old_normalize` (`str(q.resolve().relative_to(root))` form). Add `test_normalize_agrees_with_the_old_implementation`, parametrised over a table built under `tmp_path`: inside, equal to, outside the root; `a/../b` that stays in; `../outside` that leaves; trailing slash; a symlinked directory inside pointing outside and one outside pointing inside; a sibling `root-other`; relative with a base and without; empty string; `base=UNKNOWN_TREE` with a relative path; and root `Path("/")` with `/etc/hostname`. Assert `fp.normalize(p, root, base) == old_normalize(p, root, base)` for every row. Add `test_a_sibling_directory_sharing_a_prefix_is_outside` asserting `fp.normalize(str(root) + "-other/x", root) is None`.
- [ ] **Step 2: Run it to see the sibling case and the table pass against the OLD code.** Run: `cd tests && python3 -m pytest -q test_policy_loop_speedup.py`. Expected: PASS (the oracle equals the old code; this proves the table is valid before the change).
- [ ] **Step 3: Implement.** In `normalize`, replace the `try: return str(q.resolve().relative_to(root)) except ValueError: return None` with a string test on `str(q.resolve())` against `str(root)`: equal gives `"."`; starts with `str(root)` plus a separator (no doubled separator when `root` is `/`) gives the remainder; otherwise `None`. Keep the rest of the function as it is.
- [ ] **Step 4: Run the tests and the existing provenance tests.** Run: `cd tests && python3 -m pytest -q test_policy_loop_speedup.py test_harness_provenance.py test_harness_history.py test_history_cache.py`. Expected: all pass.
- [ ] **Step 5: Mutant.** Change the separator test so `root-other` passes as under `root` (for example a bare `startswith(str(root))`). Run Step 4's first command. Expected: `test_a_sibling_directory_sharing_a_prefix_is_outside` fails. Revert with the exact inverse edit.
- [ ] **Step 6: Commit** by pathspec (`scripts/file-provenance.py`, `tests/test_policy_loop_speedup.py`) after Task 5's gate, not before.

### Task 2: a memo local to one scan

**Files:**
- Modify: `scripts/file-provenance.py` (function `scan_provenance`, the call `rel = normalize(raw, root, base)`)
- Test: `tests/test_policy_loop_speedup.py`

**Interfaces:**
- Consumes: `normalize` from Task 1.
- Produces: inside `scan_provenance`, a local dict `memo: dict[tuple[str, Path | None], str | None]` and a local `norm(raw: str, base: Path | None) -> str | None`. No new module-level name.

- [ ] **Step 1: Write the failing tests.** `test_the_memo_calls_normalize_once_per_distinct_argument`: using the `checkout` fixture from `test_harness_provenance`, write the same path five times through `add_writes`, wrap `fp.normalize` with a counter via `monkeypatch.setattr`, run `fp.scan_provenance(root, env)`, assert the count equals the number of distinct `(raw, base)` pairs seen (collect them in the wrapper). `test_the_memo_does_not_outlive_the_call`: a directory symlink under the checkout pointing inside the root; scan; repoint it outside the root; scan again in the same process; assert the second scan no longer lists the path (use `os.replace` of a new link).
- [ ] **Step 2: Run them.** Expected: the first FAILS (count is higher than distinct), the second passes.
- [ ] **Step 3: Implement.** Create `memo = {}` at the top of `scan_provenance`; define `norm` to look up `(raw, base)` and otherwise call the module-global `normalize(raw, root, base)` and store the result, so a monkeypatched `normalize` is still used and an exception is never cached. Use `norm` at the one call site.
- [ ] **Step 4: Run Step 1's tests plus the Task 1 command.** Expected: all pass.
- [ ] **Step 5: Mutant.** Move `memo` to module level. Expected: `test_the_memo_does_not_outlive_the_call` fails. Revert with the exact inverse.

### Task 3: `write_targets` exits early for tools that cannot write

**Files:**
- Modify: `scripts/file-provenance.py` (constant near `NATIVE_WRITE_TOOLS`; function `write_targets`)
- Test: `tests/test_policy_loop_speedup.py`

**Interfaces:**
- Consumes: `CS_WRITE_TOOLS`, `NATIVE_WRITE_TOOLS` (existing).
- Produces: `_TARGET_TOOLS: frozenset[str]` = `CS_WRITE_TOOLS | NATIVE_WRITE_TOOLS | {"apply_patch", "Bash", "mcp__codescout__run_command", "mcp__codescout__doc", "mcp__codescout__artifact", "mcp__codescout__artifact_augment"}`; `write_targets` returns before its first branch when `name not in _TARGET_TOOLS`.

- [ ] **Step 1: Write the failing test.** `RICH` is one input dict carrying every key the branches read: `path`, `file_path`, `rel_path`, `new_rel_path`, `action="create"`, `command="echo x > out.txt"`, `patch` (a valid `*** Begin Patch` block naming one file). `test_every_name_that_yields_targets_is_in_the_set`: for every name in a list holding the constants' names plus 40 real tool names (`mcp__codescout__read_file`, `grep`, `symbols`, `Read`, `Glob`, ...), assert `bool(list(fp.write_targets(name, RICH, root))) <= (name in fp._TARGET_TOOLS)`. Also assert `list(fp.write_targets("mcp__codescout__read_file", RICH, root)) == []` and that `Bash` with `RICH` yields `out.txt`.
- [ ] **Step 2: Run it.** Expected: FAIL with `AttributeError: _TARGET_TOOLS`.
- [ ] **Step 3: Implement** the constant and the one-line early exit, after the existing `isinstance(inp, dict)` check.
- [ ] **Step 4: Run the new tests and the provenance tests.** Expected: pass.
- [ ] **Step 5: Mutant.** Remove `"mcp__codescout__run_command"` from `_TARGET_TOOLS`. Expected: the test fails (run_command with a redirect yields). Revert with the exact inverse.

### Task 4: real-corpus proof and measurement

**Files:**
- Create (not committed): `$CLAUDE_JOB_DIR/tmp/policy-loop-equivalence.py`

**Interfaces:**
- Consumes: Tasks 1-3 applied in the working tree; the pre-change engine from `git archive <HEAD before Task 1> scripts` into a temp dir.
- Produces: four digests and the wall times, quoted in the commit message and the bug file.

- [ ] **Step 1: Extract the old engine** into a temp dir with `git archive`.
- [ ] **Step 2: Write the script.** In ONE process load the old and the new `file-provenance.py` under different module names; freeze the file set to files untouched for 15 minutes (as in `cache-equivalence-frozen.py`); use the already-filled cache directory; run old, new, old, new; print seconds, a canonical digest (owners, evidence, gaps sorted, streams) and the gap count for each.
- [ ] **Step 3: Run it.** Expected: one digest on all four lines. If they differ, find the differing gaps (the script prints the first differences) before anything else.
- [ ] **Step 4: Compute the saving** as the mean of the old runs against the mean of the new runs. If under 15%, stop and report.

### Task 5: gate, commit, record

**Files:**
- Modify: `docs/issues/2026-10-31-eight-lib-tests-fail-under-16-to-1-cpu-oversubscription-through-wall-clock-windows.md` (through `doc(action="update", id="20927ccfe293c34e")`)

- [ ] **Step 1: Gate** in a clean worktree: apply the diff, copy the new test file, run `./scripts/gate.sh`. Expected: every lane exit 0.
- [ ] **Step 2: Commit** the code and tests by pathspec, with the digests and times in the message, and the patch-id.
- [ ] **Step 3: Update the bug file**: SHA, patch-id, digests, times, and what is still open (the unpickle cost and the shell regexes; approach 2 is deferred).
