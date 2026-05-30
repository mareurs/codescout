# Usage Analysis — 2026-05-29 (last 2 days, friction focus)

**Window:** `called_at >= date('now','-2 days')` (2026-05-27 → 2026-05-29)
**Scope:** all 37 `usage.db` files under `~/work`; 14 had activity in-window, 10 cleared the ≥20-call bar.

## Cross-Project Summary

Projects scanned (active): 10 | Total calls: ~7,330 | Errors: ~376 | **Combined error rate: 5.1%** | Overflows: 0

| Project | Calls | Errors | Err % |
|---|---:|---:|---:|
| backend-kotlin / weekly-pattern (worktree) | 2,186 | 106 | 4.9% |
| codescout | 1,510 | 66 | 4.4% |
| southpole / MRV-poc | 1,373 | 74 | 5.4% |
| backend-kotlin | 790 | 50 | 6.3% |
| MRV-poc / stack-config-refactor (worktree) | 492 | 25 | 5.1% |
| **eduplanner-ui** | 313 | 23 | **7.3%** ← worst rate (sizable) |
| MRV-poc / vertex-ai-search (worktree) | 224 | 15 | 6.7% |
| mirela / deployment | 228 | 4 | 1.8% |
| claude-plugins | 135 | 7 | 5.2% |
| researcher | 56 | 5 | 8.9% |
| _below 20 calls:_ companion (9), southpole (9), claude (3), codescout-old (2) | — | — | — |

**No overflows anywhere** — output budgeting is healthy. All friction is in the error stream.

### Top Frictions (ranked by recurrence × cross-project spread)

1. **[all] `edit_file` on structural code → bounced to `edit_code`** — the single dominant friction. "edit contains a symbol definition (`fun`/`class`/`def`/`function`/`interface`)" fires ~40× across 7 projects. The model defaults to `edit_file` for source edits and gets redirected every time. In codescout itself it's the stricter `debug_enforce_symbol_tools` block (10×).
2. **[all] `run_command` shell-on-source blocked** — ~40× across 7 projects. Model reaches for shell `grep`/`cat`/`read` on `.rs`/`.py`/`.kt`/`.ts`, gets told to use `grep()`/`symbols()`/`read_file()`. Recurs despite identical hint each time.
3. **[ui, stack-config, deployment] `read_file` raw line-range overlaps a named symbol** — ~15×. Model reads line ranges that straddle a symbol; redirected to `symbols(name=, include_body=true)`. Concentrated in TS/Python projects.
4. **[backend-kotlin, codescout, MRV-poc] `edit_code` replace dropped the symbol definition** — ~5×. Model passes only body statements, not the full declaration (attrs + signature + body). File is restored (safe) but the turn is wasted.
5. **[backend-kotlin] `edit_code` LSP stale/overshoot ranges (Kotlin)** — "would have dropped sibling symbols" (one case threatened 15 siblings of `HomeRoomConstraintTest`), "cannot determine end of … AST parse failed", "LSP returned suspicious range (64-77 but AST spans to 958)". Genuine Kotlin LSP-range instability, not a model error.
6. **[deployment, claude-plugins, researcher, vertex] IL3 pipe-to-log-trimmer** — ~8×. Model pipes `git log … | head`, `find … | head`, `cargo build 2>&1 | tail`. The bare-command + `@cmd_*` buffer pattern still isn't reflexive.
7. **[codescout, ui, vertex] write denied — path outside project root** — ~4×. Cross-project/cross-worktree writes: bug file into a sibling worktree, `.buddy/memory`, `.claude-sdd` memory dir. Needs `approve_write()` first.
8. **[vertex] librarian-managed artifact read/edit directly** — model uses `read_markdown`/`edit_markdown` on tracker artifacts that require the `artifact` tool.

---

## Per-Project Friction Detail

### backend-kotlin / weekly-pattern (worktree) — 2,186 calls, 106 err (4.9%)
- `edit_file` structural redirect: `fun ` ×11, `class ` ×4, `fun `(replace) ×2 = **17**
- `run_command` shell-on-source blocked: ×10
- `edit_file` old_string not found (batch aborted): ×3
- **`symbols` — kotlin-lsp failed to start: ×3** (LSP instability)
- `edit_code` action 'replace' requires 'body': ×2
- `edit_markdown` missing 'action': ×2
- `read_file` overlaps symbol (`Stage1ConstraintConfiguration`): ×2
- `edit_code` LSP suspicious range (`FeasibilityValidationService` 64-77 vs AST 958): ×1

### codescout — 1,510 calls, 66 err (4.4%)
- **`edit_file` blocked (debug_enforce_symbol_tools): ×10** — strictest gate; this is the dev repo
- `edit_file` old_string not found: ×4
- `run_command` shell-on-source: ×8
- `edit_file` "use edit_markdown for .md": ×2
- `read_file` overlaps symbol (`EditMarkdown/call`, `call`): ×4
- `edit_code` replace dropped symbol definition (`resolve_file_path`, `stripped_responses_…`): ×2
- `artifact` entry_filter on non-augmented artifact: ×1
- `create_file` write denied (`.buddy/memory/common/…`): ×1

### southpole / MRV-poc — 1,373 calls, 74 err (5.4%)
- `run_command` shell-on-source: ×10
- `edit_file` "use edit_markdown": ×4
- `edit_file` old_string not found (`_verify_gate_xlsx_live.py` ×4, server.py, vectorstore.py): ×6
- **`symbols` path not found (`src/mrv/chat_server.py`): ×3** — stale path, file moved/renamed
- `edit_file` def structural redirect: ×2
- `edit_code` replace dropped symbol definition: ×1
- `artifact` semantic search requires embedding service: ×1

### backend-kotlin — 790 calls, 50 err (6.3%)
- `edit_file` `fun ` structural redirect: ×4
- `run_command` shell-on-source: ×6
- `edit_file` old_string not found: ×2
- **`edit_code` Kotlin LSP failures cluster:** LSP not running ×1; "cannot determine end of … AST parse failed" ×2 (backtick test names); "would have dropped sibling symbols" ×3 (incl. one threatening 15 `HomeRoomConstraintTest` siblings); replace dropped definition ×1
- `artifact` scope="all" requires umbrella: ×1

### MRV-poc / stack-config-refactor (worktree) — 492 calls, 25 err (5.1%)
- `run_command` shell-on-source: ×3
- `edit_code` missing 'path': ×1; `edit_markdown` missing 'path': ×1
- `edit_file` structural / old_string / found-2-times / batch-split: ×4
- `read_file` overlaps symbol (Python classes): ×4

### eduplanner-ui — 313 calls, 23 err (7.3% — worst sizable)
- **`edit_markdown` write denied — bug file targeted at backend-kotlin worktree: ×2** (cross-project write)
- `read_file` overlaps symbol (`useSolverStream`, `SolverLock`, `SchedulePageHeader`, `SidebarContent`, etc.): **×8** — heavy raw-range reading of TS source
- `run_command` shell-on-source: ×2
- `edit_file` structural (`function `, `interface `): ×2
- `memory` missing 'topic': ×1

### MRV-poc / vertex-ai-search (worktree) — 224 calls, 15 err (6.7%)
- `edit_file` "use edit_markdown": ×2
- `create_file` write denied (`.claude-sdd/projects/…/memory`): ×1 (cross-tool memory dir)
- **librarian-managed artifact accessed directly** (`read_markdown`/`edit_markdown` on tracker artifacts): ×2
- `read_file`/`read_markdown` JSON/heading segment-not-found (guessing structure): ×4
- IL3 pipe-to-head: ×1

### mirela / deployment — 228 calls, 4 err (1.8% — cleanest)
- **IL3 pipe-to-log-trimmer: ×3** (`git log | head`, multi-stage `git status … | head`) — only friction of note
- `read_file` overlaps symbol: ×1

### claude-plugins — 135 calls, 7 err (5.2%)
- IL3 pipe-to-head (`find … | head`, `git show --stat … | head`): ×3
- `artifact` param-shape errors (mutually-exclusive args, missing `patch`): ×2
- `edit_file` markdown / old_string-found-twice: ×2

### researcher — 56 calls, 5 err (8.9%)
- `edit_file` old_string not found in `.mcp.json`: ×3 (config-file edit churn)
- `read_file` overlaps symbol (`run`): ×1
- IL3 `cargo build … | tail`: ×1

---

## Recommendations (prompt-surface candidates)

These map directly to `docs/trackers/tool-usage-patterns.md` (T-N) and `docs/trackers/skill-frictions.md`:

1. **edit_file→edit_code recurrence (#1) and shell-on-source (#2)** are guardrails *working*, but their per-session recurrence means the routing reflex hasn't stuck. The redirects already name the right tool — the open question is whether the `server_instructions` surface front-loads "source edits = edit_code, source reads = symbols/grep" strongly enough. Worth a T-N entry weighing whether more instruction text helps or whether this is irreducible (new sessions start cold).
2. **edit_code "dropped the symbol definition" (#4)** is a genuine usability sharp edge — the contract (pass the *complete* declaration) isn't obvious from the tool description. Candidate for a description tweak.
3. **Kotlin LSP range instability (#5)** is a real defect cluster (stale/overshoot ranges, AST-parse failures on backtick test names). Pairs with the existing `docs/issues/2026-03-24-kotlin-lsp-concurrent-instances.md`. Recommend a fresh bug file for the overshoot/AST-parse cases.
4. **IL3 pipe-to-head/tail (#6)** persists in git-heavy projects (deployment, claude-plugins). The bare-command + `@cmd_*` buffer pattern needs reinforcement for `git`/`find` specifically.
