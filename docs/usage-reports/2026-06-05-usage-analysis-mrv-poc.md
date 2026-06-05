# Usage Analysis — 2026-06-05 — MRV-poc

**Scope:** single project (requested). DB: `/home/marius/work/stefanini/southpole/MRV-poc/.codescout/usage.db` (74 MB)
**Window:** 2026-05-06 → 2026-06-05 · **26,146 calls** · **5.6% error rate** · **143 sessions**
**Recent week (≥2026-05-29):** ~5,950 calls

> Trigger for this analysis: a pasted transcript showing an `edit_code insert` that
> split a Python test method (`test_anchor_chunk_citation`'s trailing `assert` leaked
> into a newly-inserted method). That friction is the headline below.

---

## Headline: silent `edit_code insert-after` corruption on a parented Python symbol

**This friction does NOT appear in the 5.6% error rate.** All three `edit_code` calls in the
repair sequence returned `outcome: success`. It is a *silent correctness* failure — wall-clock
and repair edits were spent, but no error counter moved.

### Reconstructed timeline (`tests/lane_aware/test_contracts.py`, 2026-06-05)

| time | tool | action | target | outcome |
|---|---|---|---|---|
| 06:11:38 | read_file | — | lines 1–60 | **error** (overlaps `main` symbol) |
| 06:11:46 | read_file | — | lines 1–27 | success |
| 06:12:13 | edit_code | insert | after `test_extracts_anchor_suffix` | success |
| **06:12:22** | **edit_code** | **insert** | **after `test_anchor_chunk_citation`** | **success ← split** |
| 06:12:29 | run_command | — | (test run) | success |
| 06:13:05 | run_command | — | (test run) | success |
| 06:13:31 | read_file | — | lines 160–196 (inspect damage) | success |
| 06:14:06 | edit_code | replace | new method (drop orphan) — repair 1 | success |
| 06:14:14 | read_file | — | lines 166–196 (verify) | success |
| 06:14:38 | edit_code | replace | `test_anchor_chunk_citation` (restore assert) — repair 2 | success |
| 06:14:44 | edit_code | replace | new method again — repair 3 | success |
| 06:14:48 | read_file | — | lines 166–194 (verify) | success |
| 06:15:15 | edit_file | — | final touch-up | success |
| 06:15:20 | run_command | — | final test run | success |

**Cost of one bad insert:** ~3 min, **3 repair `edit_code` replaces + 1 `edit_file` + 4 inspecting reads + 3 test runs.**

### What happened
`edit_code(action=insert)` after `test_anchor_chunk_citation` computed the symbol's end at the
`)` closing its `LaneDraft(...)` constructor, **excluding the method's trailing
`assert CID_ANCHOR in draft.used_chunks`**. The new method was inserted between the `)` and the
`assert`, orphaning that assert at the bottom of the new method's body.

### Root cause — RESOLVED (distinct from the archived bug)
**Filed + fixed this session:** `docs/issues/2026-06-05-edit-code-insert-after-last-python-method.md`.

The initial guess (Python AST end-line walk under-extends) was **wrong** — probes proved the
extractor is robust (correct `end_line` for clean files, f-strings, and files with syntax errors in
following methods). The real cause is an **off-by-one in `do_insert`'s parent clamp**
(`src/tools/symbol/edit_code.rs`):

- `insert_at0 = editing_end_line_strict(sym) + 1` is correct (strict AST).
- The clamp `.min(parent.end_line)` uses an **exclusive** bound, but tree-sitter's `end_line` is
  **inclusive** (the last line the node spans). For Python there is **no closer line**, so
  `parent.end_line == last_child.end_line`. Inserting after the **last** child gives
  `insert_at0 = end + 1 > parent.end_line`, and `.min()` pulls it back one line — into the child
  body, before its trailing statement.

This is **distinct** from the archived `2026-05-02-edit-code-insert-mid-function.md`: that bug was
AST-fails → refuse (Rust). Here AST *succeeds*, the child end is correct, and the *parent* bound is
the defect. The archived `_SUBBLOCK_DISCIPLINE` refusals (2×) are a different, working path.

**Fix:** `parent_body_end_exclusive = parent.end_line + 1`. Brace languages unaffected (a child's
strict-AST end is always strictly below the parent closer). Regression test:
`insert_code_after_last_python_method_keeps_trailing_stmt` (`tests/symbol_lsp.rs`), verified
fails-without / passes-with. Reproduced end-to-end against the shipped binary.

---
---

## Systemic friction families (where the agent fights the tool boundary)

`run_command`'s `outcome='error'` captures **only codescout policy-gate rejections, not shell
exit codes** (all-time `other = 2`). So every `run_command` error is a clean friction signal.

| family | all-time | recent (≥05-29) | what the agent did | gate |
|---|---|---|---|---|
| `run_command` → **source-shell block** | 219 | 71 | `grep`/`cat`/`sed`/`git show` on `.py` source | hard-deny → use codescout tools |
| `run_command` → **IL3 block** | 108 | 50 | piped output to `\| tail`/`\| head`/`\| awk` trimmers | hard-deny → run bare + `@cmd_*` buffer |
| `read_file` → **use read_markdown** | ~103 | — | `read_file` on `.md` | redirect |
| `edit_file` → **use edit_markdown** | ~106 | 9 | `edit_file` on `.md` | redirect |
| `edit_file` → **use symbol tools** (`def`/`class`) | ~74 | 23 | structural Python edit via `edit_file` | redirect → `edit_code` |
| `read_markdown` → **managed-artifact block** | ~51 | 12 | reading librarian-managed trackers directly | block |
| `edit_file` → **anchor miss** (`old_string not found`) | — | 27 | `edit_file` exact-string fragility | batch aborted |
| `edit_code` → **body-must-be-complete** (dropped symbol def) | ~10 | 9 | `replace` body omitted the `def`/signature | RecoverableError |

**Two takeaways:**
1. **`edit_file` is the worst write tool: 288/1521 = 18.9% error**, almost entirely *misroutes*
   — markdown that belongs to `edit_markdown`, structural edits that belong to `edit_code`, plus
   the classic `old_string not found` anchor fragility. The gates catch each, but each is a
   wasted round-trip.
2. **`run_command` source-shell + IL3 blocks dominate** (327 all-time): the agent repeatedly
   reaches for raw shell on source and for `| tail`-style trimming instead of the `@cmd_*` buffer.

### IL3 — over-blocking hypothesis REFUTED (resolved during this session)
Initial read suspected the IL3 detector of flagging bounded-LHS pipes (`git log … -30 | awk`,
`git ls-files … | head -40`). Confirmed **not** a false positive: the live gate message classifies
by the **producer command**, not whether the output happens to be bounded. `git` (like `cargo`,
`npm`, `pytest`, `rg`, bare `find`) is **unbounded-LHS** — piping it to any trimmer is correctly
blocked. The carve-out ("bounded LHS is OK") is only for `ls`/`cat`/`stat`/`du`/`diff`/`awk`/`sed`/
non-recursive `grep`/`find -maxdepth`. So `git log -30 | awk` *is* a legitimate block (run bare,
query `@cmd_*`). The detector is working as designed.

Residual curiosity (low priority): a few IL3 rows showed trim-free commands (`git add … && git
commit`, `python3 - <<'PY'` heredocs). These were truncated in the log and almost certainly had a
pipe later in the command line; not pursued. The 50 recent / 108 all-time IL3 blocks are a genuine
agent-behavior signal (the model — including me, 3× this session — habitually pipes to `tail`/`head`
instead of running bare + querying the buffer), not a tooling defect.

---
---

## Tool popularity (all-time)

| tool | calls | avg_ms | max_ms | errors | err% | overflows |
|---|---|---|---|---|---|---|
| run_command | 11,375 | 7,286 | 983,311 | 327 | 2.9% | 0 |
| symbols | 3,094 | 133 | 10,631 | 18 | 0.6% | 6 |
| read_file | 2,142 | 1 | 1,002 | 355 | 16.6% | 7 |
| grep | 2,102 | 74 | 38,199 | 5 | 0.2% | 39 |
| read_markdown | 1,813 | 0 | 9 | 223 | 12.3% | 0 |
| edit_file | 1,521 | 6 | 2,238 | 288 | **18.9%** | 0 |
| edit_markdown | 1,275 | 2 | 44 | 116 | 9.1% | 0 |
| create_file | 906 | 3 | 535 | 36 | 4.0% | 0 |
| edit_code | 636 | 37 | 2,475 | 49 | 7.7% | 0 |
| tree | 440 | 8 | 101 | 0 | 0% | 9 |
| artifact | 325 | 3 | 190 | 22 | 6.8% | 0 |
| (others <200 calls) | … | | | | | |

### `edit_code` by action
| action | calls | errors |
|---|---|---|
| replace | 424 | 40 (mostly "dropped the symbol definition — body must be complete") |
| insert | 153 | 8 (incl. the 2 `_SUBBLOCK_DISCIPLINE` AST-refusals — the working safety net) |
| remove | 51 | 1 |
| rename | 8 | 0 |

> Note: the headline silent-corruption insert is counted in the **153 successes**, not the 8 errors.

---

## Latency / overflow (not frictions — project nature)

- **`run_command` max 983,311 ms (~16.4 min); 980 calls > 10 s.** All legitimate ML/data work:
  `uv run mrv ingest [--reset]`, `scripts/_build_figure_index.py`, `_classify_topics_flash.py`,
  `_measure_variance_floor.py`, `gcloud logging read`, and `sleep`/`pgrep` poll-loops. Several hit
  600 s / 900 s ceilings. This is an ML pipeline repo — long commands are expected. The only
  friction adjacency: the `| tail -N` trimming pattern that trips IL3 (above).
- **Overflows:** `grep` 39, `tree` 9, `read_file` 7, `symbols` 6 — all the discovery tools doing
  their progressive-disclosure job; no action needed.

---

## Next steps (recommended)

1. ✅ **DONE — bug filed + fixed:** `docs/issues/2026-06-05-edit-code-insert-after-last-python-method.md`.
   Fix in `src/tools/symbol/edit_code.rs` (`parent.end_line + 1`) across **all three** actions —
   `do_insert`, `do_remove`, `do_replace`; regression tests `insert_/replace_/remove_last_python_method_*`.
   Release binary rebuilt (effective on next `/mcp` restart). **Ship:** cherry-pick `edit_code.rs` +
   `tests/symbol_lsp.rs` to master (file-scoped — a concurrent session has unrelated uncommitted
   changes; do NOT `git add -A`).
2. ✅ **DONE — replace/remove spot-check:** the flagged `clamp_range_to_parent` lead was confirmed —
   `do_remove` + `do_replace` shared the identical off-by-one and silently corrupted the last method of
   a Python class (replace left the trailing stmt orphaned; remove left it behind). Reproduced live,
   fixed, regression-tested. Captured as W-9 (`bug-fix-session-log.md`) + R-17 (`reconnaissance-patterns.md`).
3. **Prompt-surface candidate:** `edit_file`'s 18.9% misroute rate suggests the
   `server_instructions` decision quickref isn't steering hard enough toward `edit_code` (structural)
   / `edit_markdown` (md). Consider a T-N entry in `docs/trackers/tool-usage-patterns.md`.
4. **Pre-existing, unrelated:** `server::tests::tool_descriptions_stay_under_budget` fails on HEAD
   (`c99d4228`) — `edit_file`'s description is 352 chars (cap 300), from the whitespace-fallback
   commit. Not caused by this fix; flagged for the owner of that change.
