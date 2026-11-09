# Which [cs-hint] classes could the codescout server deliver itself (2026-11-10)

Question: the hint hooks need a harness with hooks and, in Codex, an approval. Could the server put the same advice in its own tool results (the `hint` / `advice` fields), which every harness delivers without any approval? Related: [[hint-visibility-channels-20261109]], [[cs-hint-transcript-mining-20261107]], [[hint-channel-authority]].

## The six classes in `buddy/scripts/cs_heuristics.py` (`check`, claude-plugins `main` at `766808a`) against the server
| Class | Inputs it needs | What the server already has (read in the source) | Deliveries counted | Verdict |
|---|---|---|---|---|
| structural_edit (`edit_file` with a definition) | the call | `guard_structural_rewrite` refuses it: `src/tools/edit_file/mod.rs`, IL-2, batches too | CC 20, of which 13 came after the server had refused; Codex 20 | redundant, nothing to port |
| forgot_restore (a foreign activate after a foreign activate) | the previous activation of the session | `concurrent_switch_warning` in `src/agent/mod.rs`; test `activate_project_ro_hint_warns_switch_back`; texts NOT read | none counted separately | partial overlap, low volume |
| ignored_buffer_ref | outcome of the previous call of the session | not checked whether the server keeps a per-session last call | none counted separately | feasibility unverified |
| native_bash_on_source | a native Bash call | the server never sees native Bash | Codex 99 (about 6%) | hook only, cannot move |
| parallel_write (two writes in one second) | the previous call's timestamp | `src/agent/write_guard.rs`: held by the write-tool gate in `call_tool_inner` for every write call, async mutex in the process plus `flock` on `.codescout/write.lock` across processes | CC 18, Codex 1,544 of 1,717 (90%) | the premise is covered, see below |
| grep_for_concept | one call | no equivalent; `semantic_search` docs only | CC 17 (before the 2026-11-09 narrowing), Codex 12 | computable from one call, weakest evidence |

Counts: CC from the mining note (21 files, before the 2026-11-09 fixes); Codex from 1,717 `developer` messages carrying `[cs-hint]` in `~/.codex/sessions` (all months, all hint classes).

## Parallel writes: do they fail more? (usage.db, read-only)
`.codescout/usage.db` table `tool_calls`, write tools `edit_file`, `create_file`, `edit_code`, calls since 2026-10-20 with a start time, 11,178 calls (10,500 success, 663 `recoverable_error`, 15 `error`), grouped by whether the call started less than 1 s after the previous write call of the same server session and client:
| Group | Calls | Success | recoverable_error | error | Failed |
|---|---|---|---|---|---|
| started within 1 s of the previous write | 1,488 | 1,425 | 62 | 1 | 4.2% |
| all others | 9,690 | 9,075 | 601 | 14 | 6.3% |
Writes in a burst did not fail more often than the rest. The failure counts include every kind of recoverable error, not only concurrency ones, and the proxy "within 1 s" also catches quick sequential calls, so this does not prove that no parallel call ever conflicts. It does not support the hint's claim of inconsistent state.

## Conclusion
Porting the six checks into the server gains little: two are redundant with existing server guards (structural_edit, parallel_write), one cannot be computed by a server (native Bash), two are low volume or unverified, and one is weak. The `parallel_write` hint is 90% of the Codex deliveries and its premise is not supported by the data. Not decided: retiring or rewording it (an owner of `buddy` decision; the keep-cs-hint-in-Claude-Code decision of the operator stands). Not measured: whether the order of parallel writes ever matters to a model, and the texts of the server's switch-back warnings.
