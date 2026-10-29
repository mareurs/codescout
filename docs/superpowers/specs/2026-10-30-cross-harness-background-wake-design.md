# Background-job continuation across harnesses and operating systems

Date: 2026-10-30
Status: approved by the operator on 2026-10-30; implementation plan follows.

## Intent and agreed scope

The operator selected a shared continuation engine with small harness adapters. Claude Code and Codex are the first concrete adapters. Windows is part of the design and validation scope from the beginning; Pi and Copilot are future adapters. This extends the existing background-job wake feature rather than replacing background execution or introducing a general agent messaging service.

Success means that a terminal codescout background job can cause its originating, still-live conversation to continue, with an accurate job identity and delivery status. A duplicate observer must not ordinarily cause another continuation. An ambiguous network failure must remain visible instead of being reported as successful or silently retried.

The implementation choices explicitly labelled below are the author's proposals, not additional operator requirements. The operator approved this design on 2026-10-30. Implementation follows the separately reviewed task plan.

## Evidence and current boundaries

- [Original Claude wake design](2026-10-29-background-job-wake-design.md) records the completion-file approach, heartbeat re-arm and stale-handle behavior. Its historical filename description predates the server-token component; current record identity must follow the actual response and code.
- [Codex/Copilot measurements](../../research/2026-10-29-background-wake-codex-copilot.md) and their committed evidence establish real continuation through the existing Codex daemon and synthetic-provider Copilot SDK behavior. They establish neither Windows support nor an installed cross-harness bridge.
- `src/tools/job_record.rs`: `JobRecord`, `record_path`, `write_record`, `sweep_stale` supply versioned outcome records. The server remains the sole writer of job outcomes.
- `src/tools/output_buffer.rs` references `RECORD_MAX_AGE` for stale-record cleanup. A delivery receipt must remain distinct from a job outcome and from the continued availability of its output buffer.
- In the sibling `claude-plugins` repository, `codescout-companion/hooks/wake-on-job-end.mjs` imports Node filesystem/path modules and `readInput`/`stateHome` from `hooks/lib.mjs`. Its `main` uses `CLAUDE_PROJECT_DIR`, skips `agent_id`, normalizes spawn/re-arm input, invokes `watch`, and emits stderr with exit 2. `watch` calls `claimDelivery`, which creates a `.delivered` marker before the host consumes the wake. These are verified coupling points, not proof of acknowledged delivery.
- `codescout-companion/hooks/hooks.json` registers this entry as a Claude `PostToolUse` hook with `asyncRewake`. The shared engine must not import this registration or assume its exit-code contract.

Current first-party references, read during design:

- [Codex app-server](https://learn.chatgpt.com/docs/app-server): existing-thread lifecycle, standalone tool output, event subscription and completion notifications. `thread/read` does not subscribe to events.
- [Codex hooks](https://learn.chatgpt.com/docs/hooks): lifecycle input includes session identity and MCP tool hooks can observe results. Background-hook output alone does not start an idle turn. Installed-version input and registration shapes still require fixtures.
- [Claude hooks](https://code.claude.com/docs/en/hooks): `asyncRewake` uses exit 2; executable-and-argument hook form avoids shell parsing. Version support must be checked before adopting a registration form.
- [Pi extensions](https://github.com/earendil-works/pi/blob/main/packages/coding-agent/docs/extensions.md): an in-session extension can send a custom message that triggers a turn; session replacement and shutdown invalidate session-bound objects. This is documented feasibility, not a measured Pi implementation.

## Goals, non-goals and anti-goals

Goals:

- Reuse outcome observation, notification formatting and delivery-state rules between Claude Code and Codex.
- Bind the job to the correct host instance, profile, execution environment and conversation at observation time.
- Keep native Windows, Linux and WSL assumptions out of the shared policy logic.
- Preserve Claude's existing completion, heartbeat, re-arm and stale-handle behavior during migration.
- Expose unsupported capabilities and uncertain delivery honestly.

Non-goals for the first implementation:

- Pi or Copilot production support, remote-machine delivery, arbitrary peer messaging, a central daemon, a new message bus, or a new database.
- Waking unloaded conversations, creating replacement conversations, reviving a session that the operator closed, or changing its model/permissions/tools to make delivery work.
- Supporting subagent-owned jobs until the adapters can prove the exact subagent binding. Root-conversation support must reject or decline ambiguous ownership.
- General background execution changes or replacing codescout's output buffer.

Anti-goals:

- Treating the project directory, a bare `@bg_*` handle, PID, display title or model-written text as an authoritative conversation identity.
- Injecting a machine event as if the operator typed it when the host offers a tool/custom-message surface.
- Claiming exactly-once delivery across a crash between host acceptance and durable local recording.
- Reporting unsupported Windows integration as supported merely because pure unit tests pass.

## Approaches and decision

**Decision:** retain codescout outcome records and implement shared delivery policy in the companion with host-specific entry/delivery adapters and small platform-operation wrappers.

**Alternatives considered:**

1. Separate complete implementations per harness. Initially simple, but duplicates the reconnect, stale-handle and delivery-ambiguity policy. Rejected because the Claude implementation and measured Codex route already provide two concrete consumers of that policy.
2. Shared engine plus adapters. Selected by the operator. Keeps host semantics visible while sharing policy and tests.
3. Central session daemon. Rejected for this scope: service installation, authentication, restart and endpoint administration add costs without a named need for cross-machine operation.

**Consequences:** adding a harness or changing an endpoint stays outside job execution. There is an additional normalization contract and a host/platform validation matrix. Different hosts can prove different delivery milestones; the common interface must preserve those differences.

**Change scenarios absorbed:** adding a Pi extension; replacing Codex's local transport on Windows; changing the Claude hook registration; changing output-buffer availability without changing job-outcome delivery.

**Revisit when:** remote-machine delivery becomes required, host lifecycle makes process-local watchers insufficient, or measured watcher overhead justifies a session-level supervisor.

**Confidence:** high in the outcome/policy/host separation; medium in unmeasured automatic Codex binding and Windows transports. Those are release-gated integration questions, not assumed APIs.

## Components and dependency direction

These names describe proposed responsibilities, not existing modules.

1. **Outcome producer, in codescout:** owns running/terminal records and server-instance identity. It does not know Codex, Claude, Pi, profiles or conversation IDs. Keep schema 1 unless implementation reconnaissance finds a required missing fact; any schema change needs compatibility handling.
2. **Continuation engine, in companion:** owns normalized watch requests, record observation policy, event identity, heartbeat policy, bounded retry decisions and delivery receipts. Prefer Node ESM to reuse the existing companion runtime (author's choice). Its state-transition logic is pure; filesystem, timers, process checks and delivery are injected operations. It imports no `CLAUDE_*` helper or host SDK.
3. **Harness adapters:** normalize their host's result/lifecycle events, establish and validate the target binding, report capabilities, and translate a delivery request into the host's native continuation operation. They own hook exit codes, protocol handshakes, event subscriptions and client shutdown semantics.
4. **Platform operations:** provide argument-vector process launch, private state storage, atomic publication/exclusive claims, liveness results and local transport access. They do not identify conversations or choose delivery policy. Use existing Node primitives where their semantics are sufficient; introduce an OS-specific implementation only when a measured difference requires it.

No one-implementor transport class hierarchy is required. Extract only the shared contract needed by the two first adapters. Do not grow `hooks/lib.mjs` into a generic engine while it still contains Claude assumptions; move only the helpers actually needed at this boundary.

## Normalized identities and intake

A watch request contains:

- Format version and a stable job reference: the exact record path from the spawn response, record schema, server-instance identity including the generation token, and handle. Never reconstruct a filename from a handle.
- A target binding: harness, profile/host-instance identity, execution environment, exact conversation ID, and any adapter-owned runtime generation or branch binding needed to prevent stale delivery. Transport endpoints are adapter-owned opaque data, not a common Unix-socket field.
- Origin metadata sufficient to distinguish root from subagent calls and the originating tool event, plus the adapter's capability result.

The adapter obtains identity from trusted host event/configuration data. Model output and message bodies cannot register a target. A single registered job has one originating target; this is not a fan-out API.

Claude intake continues to recognize the response's spawn shape, including acknowledged replay calls whose input lacks `run_in_background`. Rearm reads use the persisted mapping for that same host/profile/conversation/server instance, never a global search for the handle.

For Codex, use a version-verified `PostToolUse` event to normalize the actual spawn result and its trusted session ID. Resolve the daemon endpoint within that explicitly configured profile, validate the exact existing thread, and refuse fallback to another daemon or a newly started app-server. Automatic endpoint/profile derivation is not yet proven. Until a safe bootstrap is measured, require an explicit profile endpoint configuration; missing or ambiguous binding disables wake with a visible reason. A short bootstrap probe must validate these input shapes before implementation commits to the adapter's hook registration.

The common event is identified by stable job identity, target binding and event kind. A terminal outcome has a stable terminal event ID. Heartbeats have separate persisted sequence IDs and cannot consume the terminal outcome's deduplication key.

## Delivery state and recovery policy

Separate the local attempt from host evidence:

| State | Meaning and recovery |
|---|---|
| pending | Event is durable and no send attempt has begun. Safe to claim. |
| claimed | A watcher holds a bounded lease. Expired leases can be recovered if no send intent was persisted. |
| sending | Send intent is durable before crossing the host boundary. If the watcher disappears, recover as unknown, not pending. |
| accepted | Host explicitly acknowledged the request. Do not blindly resend because a subsequent event was missed. |
| continuation_observed | A correlated host event or independent receipt proves the intended conversation continued. |
| completed | Where supported, the correlated continuation turn finished. This does not assert the model acted correctly on the result. |
| unknown | The send might have reached the host. Reconcile by stable event identity/correlation when supported; otherwise surface uncertainty and require explicit retry. |
| unsupported / failed | Capability unavailable, or a definite failure is known. Store a reason and whether retry is safe. |

The names above define the policy, not a mandatory class layout. Receipts store attempt IDs, transition times, last evidence and error classification. Lease ownership uses an unguessable token and safe compare/replace rules, not a PID-only claim. A stale owner must not delete a newer owner's claim.

A definite pre-send connection failure can retry with bounded backoff while the same target binding remains valid. A timeout or disconnection after sending is unknown unless the adapter can prove non-acceptance. Do not use the observed lack of Codex deduplication as permission to retry identical payloads automatically.

Claude's hook exit 2 proves an emission attempt, not an acknowledged model receipt. Its adapter may report a weaker milestone than Codex. Do not fabricate `accepted` or `completed` when the hook supplies no acknowledgement. A receipt may remain unconfirmed while still suppressing automatic duplicate emission; expose that distinction.

Unknown or unconfirmed attempts retain their compact audit receipt through the recovery window. Proposed defaults: retain receipts for at least the associated record lifetime, and never delete pending/unknown state as a side effect of ordinary delivered-output eviction. Final retention/configuration values belong to the plan and must name their coverage limits.

## Harness behavior

### Claude Code

Retain the native `asyncRewake` hook entry and its wake formatting. Shared observation returns a delivery event; the Claude adapter emits its reminder and exits 2. Running-job reads can rearm a heartbeat observer under the original mapping. Preserve existing handling of missing records, server death and output-handle invalidation. Do not replace this path with cross-session inbox sockets merely to make all adapters look alike.

### Codex

Connect to the existing bound daemon, initialize once, validate the target is loaded, and establish an event subscription through a version-supported attach/resume operation. A `thread/read` response is not a subscription. Verify that attachment leaves the original client's tools, permission callbacks and policy intact; omit all model/config overrides. If this cannot be proved for a host version, decline automatic delivery for that version.

Use standalone tool output with empty user input and a stable event ID in its envelope. Retain the exact server reply and resulting turn/item correlation. Observe the tool-output item and continuation for this event rather than any unrelated `idle`/`completed` notification. A concurrent active turn may receive the output at its supported tool-result boundary; do not interrupt it or create another thread. Client cleanup must close only the secondary connection, never unload/destroy the original conversation.

### Future Pi

Prefer an in-session extension over attaching to a launcher-owned RPC stdin. The extension can call `sendMessage` with a custom event and `triggerTurn`, using follow-up delivery as the proposed non-interrupting default. Session shutdown/reload/fork/switch must invalidate old bindings. A branch change needs an explicit stale-origin policy: first proposal is to retain the event as pending for its origin and decline injection into a different branch until the adapter can prove ancestry. This is future work, not first-release Pi support.

### Future Copilot

Reuse the common watch/receipt policy but own the SDK lifecycle in the adapter. Validate real authenticated existing-session delivery and original-owner callback preservation first. The measured SDK's `stop()` versus external `forceStop()` distinction must not leak into generic cleanup policy or be assumed for other SDK versions.

## Platform contract and Windows

Native Windows is a validation target; WSL is a separate Linux execution environment. Host, codescout server and watcher must initially share the same filesystem/process environment. Windows-to-WSL bridging, path translation and remote filesystems are out of scope; reject mixed-environment bindings with a clear reason.

- Pass executable and argument arrays to process launch. Avoid shell interpolation. Hook registrations use a version-supported exec form where available, or a separately tested platform-specific registration; do not assume Codex and Claude accept the same hook fields.
- Consume the server-provided record path as opaque data. Use platform path APIs; do not derive a sibling host's temp directory from environment-variable spelling.
- No `/proc`, process-parent walks, POSIX-only signals or mandatory Unix sockets in the shared policy. Liveness is `alive`, `dead` or `unknown`; unknown cannot justify claiming the server died. Avoid PID reuse by binding to server generation/handshake evidence.
- Isolate local transport inside the adapter. The measured Codex Unix WebSocket is the Linux implementation. Select a native Windows transport only after checking that installed host exposes it; do not invent named-pipe support. An authenticated loopback endpoint is an option only when actually exposed and configured by that host.
- Verify exclusive creation, rename/replace, open-handle behavior and private permissions on the supported Windows filesystem. POSIX mode bits alone are not a Windows privacy guarantee. Polling is a portable observation fallback; filesystem events are an optimization and never the sole terminal-state check.
- Node runtime availability, spaces/non-ASCII paths, profile isolation and process cleanup are installation checks, not assumptions inherited from this Linux machine.

No platform/harness combination becomes advertised as supported until an end-to-end native test passes. If native Windows host access is unavailable, commit portable code/tests but leave that capability disabled or explicitly experimental; Linux unit tests are not substituted for that release check.

## Migration, cleanup and failure behavior

Keep the existing completion record as the source of outcome truth. New delivery state is stored separately and scoped by target and engine version. Existing `.delivered` markers are legacy claims, not new acknowledged receipts.

Opt in per profile to a single owner for new jobs: legacy Claude hook or new engine, never both. Do not migrate already-watched jobs mid-flight. Preserve the old entry point as a compatibility wrapper after its shared-engine replacement passes parity tests. Installation switching must drain/expire old watchers before new ownership is enabled; no global kill-by-name.

Server output eviction and delivery receipts need a compatibility review together. Write a legacy cleanup marker only at the adapter-specific safe cleanup boundary; do not let an `accepted` receipt delete evidence needed to reconcile `unknown`. If compatibility requires a small server cleanup change, document the invariant and pin it with a regression test rather than expanding the record into a host registry.

On server-generation loss, never tell the model to read a possibly reused handle. Report the persisted outcome if known; otherwise explain that the job state is unknown. Do not report a signal death when no signal was observed. If the target closes, retain the receipt but do not reopen the conversation. On record/receipt write failure, surface the degraded state and stop claiming durable delivery.

Keep diagnostic events structured and local. Record identities, transitions and correlation IDs; avoid full command output, credentials, endpoint tokens or unrelated conversation contents.

## Verification and release gates

Before implementation, collect bounded fixtures for Codex hook input, profile endpoint resolution, root/subagent identity and safe event attachment. Resolve any missing capability as disabled, not as guessed identity. Fixtures must identify host/version/OS and separate documented behavior from measured behavior.

Shared tests cover normal completion, nonzero exit, signal outcome, spawn failure, heartbeat/terminal separation, duplicate observers, crash before send intent, crash after intent, accepted-but-disconnected delivery, stale lease takeover, state-write failure, vanished records, server-generation change and closed targets. Use deterministic clocks/fake transports for state-machine tests.

Adapter contract tests assert actual normalized payloads, attribution, safe continuation timing, capability failures, binding isolation and cleanup. Preserve Claude's spawn/replay/rearm/stale-handle behavior. Real Codex tests use the original live conversation, a second connection and a subsequent original-client tool/permission callback to detect ownership damage. Correlate the exact event marker; generic idle events do not pass the test.

The integration matrix separates Linux, native Windows and WSL; add macOS when a host is available. Test paths with spaces/non-ASCII characters and unavailable optional transports. Pi/Copilot rows remain unverified until their own real probes run.

For load-bearing implementation reviews, actually apply candidate mutations to identity discrimination, send-intent persistence, ambiguous retry suppression and stale-owner claim release. Run the relevant suite for each and report observed surviving/killed counts before writing any missing regression tests. A design-only review has no implementation to mutate and must not invent counts.

Run each repository's required gate in its own isolated checkout; the codescout gate does not validate companion behavior. Record the exact tested commits and any skipped native-host matrix rows. Do not change all local profiles or install releases as a side effect of drafting this design. When installation is later authorized, check all configured profiles and each profile's own cache paths.

## Left to the builder and approval boundary

The builder chooses module/file names, test fixture layout, deterministic clock/transport injection style, polling/backoff settings, exact receipt serialization and safe state-directory implementation. These choices must preserve the identity, ambiguity, compatibility and platform constraints above. Prefer existing Node ESM and filesystem records; introducing a persistent service or another storage system reopens the design decision.

The next step after operator approval is an implementation plan, including the bounded capability probes and incremental Claude/Codex migration. Product code, dependency installation and profile changes wait for that approval. This document itself is reviewable documentation, not an installed feature.
