# Harness-neutral session context

Codescout accepts version 1 context: `harness`, opaque root `session_id`, nullable `agent_id`, canonical absolute Git `workspace`, `source`, and nullable `harness_version`. Identity is `<harness>:<session_id>`; a child's thread ID is metadata and keeps its root principal.

`CLAUDE_CODE_SESSION_ID` and `CODEX_SESSION_ID` are native roots. Simultaneous values conflict, even if equal. `CODEX_THREAD_ID` alone is insufficient. Use an absolute `--context` path or `CODESCOUT_CONTEXT_FILE` for explicit context. Conflicting CLI/environment files or disagreement with native identity fail closed. The invocation's Git root is authoritative; an inherited foreign `REPO_ROOT` cannot redirect formatter attribution.

## Modes and outcomes

```sh
./scripts/gate.sh --check
./scripts/gate.sh
./scripts/gate.sh --context /absolute/context.json
./scripts/gate.sh --exclusive-checkout /absolute/checkout
```

The same options apply to `fmt-mine.sh`. `--check` bypasses ownership resolution and writes nothing: 0 clean, 1 differences, 2 usage/tool failure. Normal standalone formatting exits cheaply on a clean tree; otherwise context is required. Own and git-clean paths may be formatted; peer/shared/unknown paths are refused with exit 1. Missing/conflicting context or an unsafe target exits 2 without formatting writes.

An exclusive declaration requires an absolute matching Git root and established operator control. It permits formatting there without inventing a MINE label. Conflicting or unsupported context still blocks it. Literal path arguments preserve spaces; ambiguous/unrepresentable paths and symlink/module escapes are rejected before writes. A read-only stdout probe checks clean descendants too; fixed-point formatting remains bounded to three passes.

The gate leases independently of identity and reports all five lanes. A context-blocked formatter has `FMT=2 FMT_STATE=context-blocked`; Python, clippy, lean and default tests still run. Default remains last after earlier failures. Other states are `passed`, `differences`, `ownership-refused`, and `tool-error`. Exit 1 aggregates a completed-lane failure; exit 2 is invalid entry or lease failure. A private snapshot is passed through `CODESCOUT_CONTEXT_FILE` and removed on normal or handled signal exits; SIGKILL cannot execute cleanup. Explicit `CARGO_TARGET_DIR` remains the caller's responsibility.

## Evidence boundaries

file-provenance.py [--context /absolute/context.json] <literal paths...> reads Claude and Codex history. FILE_PROVENANCE_ROOTS retains the Claude override; FILE_PROVENANCE_CODEX_ROOTS_JSON is a JSON array of absolute Codex history roots. Invalid configuration or unreadable/opaque history is an explicit coverage gap. Discovery includes native archived Codex sessions and canonical alias deduplication.

Refused atomic calls do not claim ownership. Non-atomic shell errors or missing results can still represent writes. Root/child event identity and native ordinals govern child/fork replay boundaries. Verified native child copies use subagent_history_start_ordinal; new-root forks require their separate verified replay cutoff. Unknown wrappers and external history bases are not guessed. Shell extraction remains heuristic and manual/editor writes remain uncovered.

Paginated native event_msg/item_completed McpToolCall items are normalized from their actual call ID, executing thread, server/tool, argument object, completion status and result content. Turn-context cwd and precise JSON numbers are preserved. Unsupported native effects/statuses and malformed/conflicting records retain coverage gaps. Recovering a completed inner call does not establish that its surrounding code-mode wrapper has a complete execution inventory.

Coverage and peer contact are separate capabilities. Known Codex ownership remains known when contact/liveness are unavailable; only Claude owners consult the Claude registry. A not-live marker does not prove abandonment.

Source tests cover both hosts, mixed ownership, same-ID host collisions, child boundaries, refusals, gaps, literal paths, lease concurrency and default-lane continuation. Observed versions are Claude Code 2.1.296 and Codex 0.160.0, not a supported range. Actual main-profile Claude write/owned formatting, same-root resume, child/parent formatting and conservative fork refusal passed. All three Claude profiles cold-loaded the plugin; main/SDD authenticated smoke passed, Kat authentication remains pending.

Native Codex MCP sessionId/threadId automatically reach spawned commands. Fresh, resumed, compacted, forked and child context probes passed. Actual native root 01a12f3a-ae7e-7882-87e8-8b6e4eb81706 has a recovered writer with exact arguments and full native root/thread/call identity. On same-root resume the normal formatter retained this writer, exited 1/UNKNOWN and left bytes unchanged because wrapper coverage remains incomplete. Successful Codex owned formatting and complete authored-history continuity remain unestablished. The coordinated plugin review records native evidence, applied mutations and dated installation observations.

## Commit guards and Codex sessions

Five commit-safety scripts accept a Codex session besides a Claude session: `commit-mine.sh`, `git-safe-reset.sh`, `mutation-probe.sh`, `pre-commit-foreign-index.sh` and `post-index-change-stage-log.sh`. All resolve identity through `scripts/publish-identity.sh`: `publish_identity` (the principal), `publish_principal_equal` (comparison) and `publish_principal_root` (strip one host qualifier).

- Owner format. The stage-log owner is the bare id for Claude and `codex:<root id>` for Codex. It is `-` when identity is missing, conflicting, invalid, or the resolver cannot run. The write record in `usage.db` stores BARE ids for both hosts (no harness column), so every comparison strips one leading `codex:` or `claude-code:` first. `-` and empty never equal anything. A same-id collision across hosts counts as one writer; this trade-off is accepted.
- Ack token. A Codex peer is acked as `CODESCOUT_INDEX_ACK="codex:<id>"`; a bare id for that peer is refused. A Codex writer found through the write record shows up with its bare id (see the limitations below).
- Conflicting identity (both `CLAUDE_CODE_SESSION_ID` and `CODEX_SESSION_ID` set, for example a Codex run launched from inside a Claude session). The user-run scripts exit 2 with the resolver's reason. The hooks record `-` or stand down. The installed pre-commit dispatcher refuses the commit (the hold guard fails closed). Remedy: unset one of the two variables.
- `python3` is a runtime dependency of attribution in the stage-log hook and the guard. Without it the hook records `-` (it never fails git) and the user-run scripts exit 2.
- `mutation-probe.sh` marker files: the file name is the bare root (never a colon); the JSON `session_id` holds the full principal.

Known limitations:

- A Codex writer attributed through `usage.db` has a bare owner. The guard's refusal therefore describes it with the Claude liveness text, and the pre-push hold lookup maps its co-author token to `refs/holds/<id>` instead of `refs/holds/_codex/<id>`. The real fix is recording the harness in `usage.db` (the spec's option 3); it is not done.
- The guard captures the resolver's output with `2>&1`, so a Python startup warning on stderr can be read as an identity. The direction is conservative (it refuses or labels `theirs`). Open finding N1.
- No live Codex session ran these scripts. All verification is by source tests and fixtures.
