---
id: '261eb726c498b195'
kind: plan
status: active
title: Store each operation's write targets in the history cache — Implementation Plan
tags:
- attribution
- file-provenance
- performance
---

# Store each operation's write targets in the history cache — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** a warm `scan_provenance` stops re-deriving every operation's write targets, with its output unchanged.

**Architecture:** the pure target extraction moves out of `scripts/file-provenance.py` into a new `scripts/write_targets.py`, which joins the history cache key. Each cached `Operation` carries its static targets; the doc tool's catalog-id lookups stay live at scan time and are appended after the static part, so the order of targets is unchanged.

**Tech Stack:** Python 3 (stdlib only, run as `python3 -I`), pytest; Rust for the embed list and one probe test.

**Spec:** `docs/superpowers/specs/2026-11-04-attribution-write-targets-in-cache-design.md` (approved 2026-11-04). Bug `20927ccfe293c34e`; queue entry `resume-context-injection-remaining-work:CIW-10`; recon `bug-fix-session-log:W-157`.

## Global Constraints

- Output identical: `scan_provenance` returns the same owners, evidence, gaps and streams for every input (spec § Goals; anti-goal "even in gaps").
- An edit to the ownership policy in `file-provenance.py` must NOT change the cache key; an edit to `write_targets.py` must.
- `catalog_paths()` results are never stored.
- The engine runs embedded: every module it imports is in `EMBEDDED_SCRIPTS` (`src/tools/run_command/attribution.rs`), today 7 entries, after this plan 8.
- Stdlib only; the engine runs under `python3 -I`.
- If the measured warm saving is under 15%, stop and report (spec § Left to the builder).

## Review Focus

1. A doc-tool operation with an `id` whose catalog row moves between two warm scans: the second scan must report the new path (the live part is not cached). → Task 2, `test_a_doc_id_target_follows_the_catalog_between_warm_scans`.
2. A cache entry written before this change (an `Operation` pickled without `targets`) is read after it: the scan must give the old answer, not an empty target list. → Task 2, `test_an_operation_without_stored_targets_is_recomputed`.
3. An atomic operation refused with `no-write-refused` still contributes no targets, whatever is stored. → Task 2, `test_a_refused_atomic_write_still_contributes_nothing`.
4. A non-dict `input` (malformed transcript line) must not raise in the wrapper that fills targets. → Task 2, `test_filling_targets_tolerates_a_non_dict_input`.
5. A batch carrying an "unreadable ... source" gap is still never stored after targets are filled. → existing `test_a_result_with_an_unreadable_source_gap_is_never_stored` must stay green (Task 2, Step 4).

---

### Task 1: Extract `scripts/write_targets.py`, behaviour unchanged

**Files:**
- Create: `scripts/write_targets.py`
- Modify: `scripts/file-provenance.py` (remove the moved definitions, import them; `write_targets` becomes static part + live part)
- Modify: `src/tools/run_command/attribution.rs` (`EMBEDDED_SCRIPTS`: `[(&str, &str); 7]` → `8`, add `"write_targets.py"`)
- Modify: `src/server.rs` `provenance_probes_reference_only_real_tool_names` (add `("scripts/write_targets.py", …)` to `surfaces`; the positive check passes when the UNION of the surfaces names each write-capable tool)
- Test: `tests/test_write_targets_cache.py` (new)

**Interfaces:**
- Produces, in `scripts/write_targets.py`: the constants `CS_WRITE_TOOLS`, `NATIVE_WRITE_TOOLS`, `ARTIFACT_WRITE_ACTIONS`, `_TARGET_NAMES`, `_Q`, `PY_WRITE_LITERAL`, `PY_WRITE_VAR`, `PY_ASSIGN`, `BASH_WRITE_PATTERNS`, `RELOCATORS` (moved verbatim, comments included); the functions `python_write_targets(cmd: str)`, `patch_write_targets(patch: str) -> list[str]`, `relocation_targets(cmd: str)`, `_operands(rest)` (moved verbatim); and new `static_targets(name: str, inp: object) -> Iterator[str]`.
- Produces, in `scripts/file-provenance.py`: `catalog_targets(name: str, inp: object) -> Iterator[str]` (the doc-tool id lookups, via `catalog_paths()`), and `write_targets(name, inp, root)` with unchanged signature = `static_targets(...)` then `catalog_targets(...)`.
- `file-provenance.py` keeps every moved name importable as an attribute (`fp.CS_WRITE_TOOLS` etc.), because `tests/test_policy_loop_speedup.py` reads them.

- [ ] **Step 1: Capture a golden table from the CURRENT code, before any edit.** Write `tests/test_write_targets_cache.py` with a module-level table `CASES` of `(name, inp)` covering every branch of today's `write_targets`: `apply_patch` (Add/Update/Delete/Move), each `CS_WRITE_TOOLS` key form, each `NATIVE_WRITE_TOOLS` name, `mcp__codescout__doc` with `action` in and out of `ARTIFACT_WRITE_ACTIONS` and with `rel_path` + `new_rel_path` + `id`, `mcp__codescout__artifact_augment`, `Bash` and `mcp__codescout__run_command` with a redirect, `sed -i`, `git mv`, a python `open(…,'w')` literal and a variable form, a read-only tool, and a non-dict input. Monkeypatch `catalog_paths` to return `{"fixture-artifact-id": "docs/x.md"}` (a non-hex key on purpose: a 16-hex literal in a committed doc reads as a dead artifact citation), and give the doc cases `id="fixture-artifact-id"`. Generate `GOLDEN = [list(fp.write_targets(n, i, root)) for n, i in CASES]` once with the current code and paste the literal list into the file.

- [ ] **Step 2: Write the tests that fail now.**
  - `test_write_targets_matches_the_golden_table`: for every case, `list(fp.write_targets(n, i, root)) == GOLDEN[k]`.
  - `test_static_then_catalog_is_write_targets`: for every case, `list(wt.static_targets(n, i)) + list(fp.catalog_targets(n, i)) == GOLDEN[k]`.
  - `test_static_targets_never_reads_the_catalog`: monkeypatch `fp.catalog_paths` and `wt` to raise; `static_targets` over every case does not raise.
  - `test_the_moved_names_are_still_attributes_of_file_provenance`: `fp.CS_WRITE_TOOLS is wt.CS_WRITE_TOOLS` (and the other moved constants).

- [ ] **Step 3: Run them.** `python3 -m pytest tests/test_write_targets_cache.py -q` → the golden test passes, the other three FAIL (`ModuleNotFoundError: write_targets` or `AttributeError: catalog_targets`).

- [ ] **Step 4: Implement the move.** Create `scripts/write_targets.py` with the moved definitions and `static_targets` (today's `write_targets` body minus the `catalog_paths()` loop). In `file-provenance.py` import them, add `catalog_targets`, and redefine `write_targets` as the static part followed by the catalog part. Add the `EMBEDDED_SCRIPTS` entry and the probe-test surface.

- [ ] **Step 5: Run.** `python3 -m pytest tests/test_write_targets_cache.py tests/test_policy_loop_speedup.py tests/test_harness_provenance.py tests/test_history_cache.py -q` → all pass. `scripts/with-slot.sh cargo test --lib -- provenance_probes_reference_only_real_tool_names the_materialized_set_is_closed_under_its_own_local_imports` → pass.

- [ ] **Step 6: Mutant check.** Put the catalog loop's `yield` BEFORE the `rel_path` keys in `write_targets` → `test_write_targets_matches_the_golden_table` fails for the doc move case. Revert.

- [ ] **Step 7: Commit** by pathspec (shared checkout): the five files above. Message names the bug and CIW-10.

### Task 2: Cached operations carry their static targets

**Files:**
- Modify: `scripts/provenance_records.py` (`Operation` gains `targets: tuple[str, ...] | None = None`, last field)
- Modify: `scripts/history_cache.py` (`_CODE_FILES` gains `"write_targets.py"`)
- Modify: `scripts/file-provenance.py` (`scan_provenance`: fill targets inside the cached reader; use them in the loop)
- Test: `tests/test_write_targets_cache.py`

**Interfaces:**
- Consumes: `static_targets`, `catalog_targets` (Task 1).
- Produces, in `scripts/write_targets.py`: `with_targets(batch: HistoryBatch) -> HistoryBatch`, returning a batch whose operations are `dataclasses.replace(op, targets=tuple(static_targets(op.name, op.input)))`; gaps unchanged.
- In `scan_provenance`: readers become `caching_reader(lambda p: with_targets(read_claude_history(p)), cache)` (same for Codex), and the loop's target list is `list(op.targets if op.targets is not None else static_targets(op.name, op.input)) + list(catalog_targets(op.name, op.input))`, still `[]` for `op.atomic and op.outcome == "no-write-refused"`.

- [ ] **Step 1: Write the failing tests.**
  - `test_a_cached_batch_carries_targets`: warm the cache over the `checkout` fixture with writes (`add_writes`), then load an entry with `hc._load` and assert every operation's `targets` equals `tuple(static_targets(op.name, op.input))`.
  - `test_the_key_changes_with_write_targets_and_not_with_file_provenance`: `"write_targets.py" in hc._CODE_FILES` and `"file-provenance.py" not in hc._CODE_FILES`; with a temp copy of the scripts dir, editing `write_targets.py` changes `code_digest()` and editing `file-provenance.py` does not (clear the `lru_cache` between).
  - `test_a_doc_id_target_follows_the_catalog_between_warm_scans` (Review Focus 1).
  - `test_an_operation_without_stored_targets_is_recomputed` (Review Focus 2): a batch whose operations have `targets=None` scans to the same `ProvenanceScan` as a filled one.
  - `test_a_refused_atomic_write_still_contributes_nothing` (Review Focus 3).
  - `test_filling_targets_tolerates_a_non_dict_input` (Review Focus 4).

- [ ] **Step 2: Run** `python3 -m pytest tests/test_write_targets_cache.py -q` → the six new tests FAIL.

- [ ] **Step 3: Implement** the field, `with_targets`, the `_CODE_FILES` entry and the two `scan_provenance` changes.

- [ ] **Step 4: Run** `python3 -m pytest tests/test_write_targets_cache.py tests/test_history_cache.py tests/test_policy_loop_speedup.py tests/test_harness_provenance.py tests/test_harness_history.py -q` → all pass, including `test_a_warm_scan_equals_a_cold_scan_and_an_uncached_scan` and `test_a_result_with_an_unreadable_source_gap_is_never_stored` (Review Focus 5).

- [ ] **Step 5: Mutants**, each must fail at least one test, then revert: (a) `_CODE_FILES` without `"write_targets.py"`; (b) the loop ignores `catalog_targets`; (c) the loop puts `catalog_targets` before the static part; (d) `with_targets` stores `catalog_targets` too.

- [ ] **Step 6: Commit** by pathspec: the four files.

### Task 3: Prove identity on the real corpus, measure, record

**Files:**
- Create (scratch, not committed): a one-process comparison script in the session scratchpad.
- Modify: `docs/issues/2026-10-31-eight-lib-tests-fail-under-16-to-1-cpu-oversubscription-through-wall-clock-windows.md` (§ Remaining items, item 1) and `docs/trackers/resume-context-injection-remaining-work.md` (CIW-10) — through `doc(action="update", body_edits=…)`.

- [ ] **Step 1: One-process identity.** Extract the pre-change scripts with `git archive <Task-1 parent SHA> scripts | tar -x -C <tmp>/old`. In ONE python process, import the old and the new `file-provenance.py` by path (each with its own `sys.path` head and module names), point both at the same `environ` with a fresh `FILE_PROVENANCE_CACHE_DIR` each, and over the real checkout, restricted to history files untouched for 15 minutes, run old, new, old, new. Digest each `ProvenanceScan` (sorted owners, evidence, gaps, streams). Expected: four equal digests.

- [ ] **Step 2: Warm wall time.** With each version's cache warm, time `python3 -I scripts/attribute-red.py` on the same dirty-path set, 3 runs each, alternating, same load. Report the medians and the saving. If the saving is under 15% of the old warm time, stop and report.

- [ ] **Step 3: Gate.** `./scripts/gate.sh` → `GATE EXITS -> FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0`.

- [ ] **Step 4: Record.** Bug file item 1: SHA and patch-id of Tasks 1–2, the four digests, the timings, what stays open (the first scan after deploy misses every entry once; load can still exceed 15 s). CIW-10 → `done`, or `open` with the measured reason if Step 2 stopped. Commit by pathspec.
