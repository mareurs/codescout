# Background-job wake in Codex and Copilot — measured 2026-10-29

Codex can resume an idle, already-open conversation when a separate client submits a background result through its existing app-server; the installed Claude-style companion hook does not provide that integration. Copilot's installed SDK/runtime also accepts automatic continuation, but its hosted model and an existing VS Code conversation were not tested.

**Reader:** a maintainer deciding how to add background-job completion delivery to codescout-companion.

**Scope:** dated feasibility measurements, not a shipped adapter or a reliability benchmark. The companion installation and its configuration were unchanged. The final Codex experiment used the real codescout job, existing daemon, existing conversation and authenticated model. Copilot used the real installed SDK/runtime with a deterministic local HTTP provider.

## Results and evidence

The committed [evidence bundle](evidence/2026-10-29-background-wake/manifest.json) contains result records, selected event streams, provider-request records, the final model receipt and the historical probe sources. The manifest records SHA-256 digests for its listed evidence files. Copied files also carry digests identifying the temporary originals used to assemble the bundle. The original scratch directory was `/tmp/wake-measurements-20261029`; the committed files are the durable evidence.

Observed versions: Codex CLI **0.165.0**, existing Codex daemon **0.160.0**, Copilot CLI and its bundled SDK **1.0.2**, codescout-companion **1.20.23**. These observations do not establish a supported version range.

| Question | Observation | Evidence |
|---|---|---|
| Does a standalone tool result start generation after Codex is idle? | Three trials completed with their three expected markers. They used an isolated thread on the existing daemon and a real model. | [Codex matrix](evidence/2026-10-29-background-wake/codex-results.json) |
| Does delivery work while a Codex turn is active? | After observing `turn/started` and status `active`, the result joined the same turn and its marker appeared before completion. | [Supplement](evidence/2026-10-29-background-wake/codex-supplement-results.json) |
| Can another Codex client deliver into the same thread? | The second client resumed the same thread and submitted a result; the first client observed its completion. Closing both clients and reconnecting also allowed continuation. | [Codex matrix](evidence/2026-10-29-background-wake/codex-results.json) |
| Does the actual open Codex conversation wake without another user message? | A real 30-second codescout job finished with exit 0. A separate watcher observed the existing conversation idle, submitted tool output, and the resumed model read the job's completion marker. | [Watcher timeline](evidence/2026-10-29-background-wake/current-session-wake.jsonl), [model receipt](evidence/2026-10-29-background-wake/current-session-model-receipt.json) |
| Are repeated Codex deliveries deduplicated? | Two identical outputs produced distinct completed turns. Repeating the same `clientUserMessageId` also produced distinct turns. | [Matrix](evidence/2026-10-29-background-wake/codex-results.json), [supplement](evidence/2026-10-29-background-wake/codex-supplement-results.json) |
| Does context injection alone start an idle Codex turn? | `thread/inject_items` succeeded; no `turn/started` event arrived in the two-second observation window, and the thread remained idle. This is a bounded observation. | [Supplement](evidence/2026-10-29-background-wake/codex-supplement-results.json) |
| Can Copilot continue automatically after idle? | Three expected markers were returned in the stdio matrix; the TCP matrices also exercised idle delivery, queueing, duplicate prompts and session resume. The provider was synthetic. | [Stdio matrix](evidence/2026-10-29-background-wake/copilot-stdio-results.json), [TCP matrix](evidence/2026-10-29-background-wake/copilot-force-detach-results.json) |
| Can a second Copilot SDK client reach the resident session? | It resumed the same session ID, delivered a prompt and the original client observed the answer. | [TCP matrix](evidence/2026-10-29-background-wake/copilot-force-detach-results.json) |
| Can the secondary Copilot client leave safely? | In this bundled SDK, `stop()` destroys attached sessions. After external-client `forceStop()`, the original owner sent another prompt and observed its expected answer. | [Stop result](evidence/2026-10-29-background-wake/copilot-client-stop-results.json), [transport-detach result](evidence/2026-10-29-background-wake/copilot-force-detach-results.json) |
| Are nonzero exits and signals distinguishable in codescout? | The job reader reported `exited 7` and `terminated by signal`. Its own command exit code was 0 in both cases. | [Terminal-state reads](evidence/2026-10-29-background-wake/codescout-terminal-results.json) |

The Codex idle elapsed times include the model's complete response. They are not measurements of transport latency or notification latency. Copilot's fixture usage fields are placeholders and must not be reported as measured token usage. Trial counts describe these runs only.

## Codex transport and delivery contract

The working route was a WebSocket connection over the existing daemon's Unix control socket. A direct HTTP Upgrade returned `101 Switching Protocols`; initialization identified the running daemon, and `thread/read` recognized the current conversation. Earlier attempts through the CLI's stdio proxy timed out. That failure did not establish that the underlying socket was unreachable, nor that the CLI/daemon version difference caused it.

The final test followed this sequence:

1. Launch a real background job with codescout's `run_command` and retain its returned record path, handle and server identity.
2. Connect to the existing Codex daemon and send `initialize`, followed by `initialized`.
3. Verify that the exact target thread is present. The temporary watcher waited for the job's terminal record and three seconds of continuously observed idle state.
4. Submit `turn/start` with an empty input array and a named standalone tool result.
5. Observe actual generation in that same conversation. The resumed model used codescout to read the original job and saw its completion marker.

This request shape was exercised against the real daemon; the placeholder strings below stand for the recorded thread and job result:

```json
{
  "id": 4,
  "method": "turn/start",
  "params": {
    "threadId": "<existing-thread-id>",
    "input": [],
    "toolOutput": {
      "name": "codescout_background_measurement",
      "output": "<completed-job-result>"
    }
  }
}
```

The measured controls rejected mixed nonempty input plus tool output, an empty tool name, a well-formed unknown thread ID, and `turn/steer` on an idle thread. Use `turn/start` for idle continuation. The active-turn experiment showed that standalone tool output can join an already-active turn, but the final open-conversation watcher deliberately waited for idle.

**Acceptance is not completion.** The final watcher used `thread/read` without subscribing to the target thread's event stream. Its log therefore ends at request acceptance. The independent model receipt and real job read establish automatic continuation; the watcher was stopped after that confirmation. A maintained adapter needs an explicit subscription and a correlated completion observation if it promises to track the resulting turn.

## Copilot boundary and lifecycle

The minimal ACP handshake worked. Adding the experimental probe option `--deny-tool '*'` caused the installed CLI to exit before initialization; removing that option fixed the probe. Creating a session through the unauthenticated ACP setup then returned `Authentication required`. No authenticated hosted-model ACP run or existing VS Code conversation was established.

The bundled SDK provided a separately measurable route: start its headless runtime, create a session with a custom provider, then use `send`/`sendAndWait` and `resumeSession`. A local OpenAI-compatible fixture let the real runtime execute these lifecycle operations without a GitHub token. The second TCP SDK client used that installed SDK's `cliUrl` option to reach the same resident runtime; this is version-specific and must not be substituted into newer SDK examples without checking their API.

Lifecycle calls matter. The [captured installed implementation](evidence/2026-10-29-background-wake/copilot-sdk-lifecycle-observation.json) of `stop()` calls `destroy()` on the client's attached sessions before closing its connection. The test confirmed `Session not found` for the original owner afterward. The installed session prototype did not expose `disconnect()`. An external client's `forceStop()` closed the transport without killing the independently owned runtime, and the original owner's subsequent marker was observed. This result is scoped to an external client on the measured SDK version; it is not a general recommendation to force-stop an owning client. The fixture runs made no permission requests, so preservation of the original client's tool handlers, permission callbacks and hooks after another client resumes the session remains unverified.

Two instrumentation corrections are part of the evidence boundary:

- The first fixture always returned SSE. The runtime had requested an ordinary JSON completion; its logs identified the parsing mismatch. Successful runs used the requested response format. Fixture failures are not Copilot wake failures.
- A multi-client `sendAndWait` call initially returned without the expected marker. The final detach measurement instead waited for the expected assistant event and checked that the provider received the subsequent request. A previous idle event is not proof that the new message completed.

## Implications for a companion adapter

The experiments support a host-specific bridge: terminal job record, exact conversation identity, host transport, continuation API. They do not install or validate a portable companion hook.

A maintained bridge needs to preserve the following measured distinctions:

- Pin the job record to its originating codescout server and the conversation to the correct host/profile. A readable record or an accepted request alone does not establish delivery to the intended conversation.
- Deduplicate completion delivery in the bridge. The Codex experiment did not find deduplication from identical tool output or `clientUserMessageId`.
- Separate request acceptance, observed generation and completion. Correlate messages/turns instead of treating an unrelated idle event as success.
- Keep secondary-client cleanup separate from session destruction. The measured Copilot SDK makes this distinction consequential.
- Read the background job's state/exit code. The exit code of the command reading its log is a different measurement.

Shared-daemon crash/restart behavior, recovery after an ambiguously accepted delivery, authenticated Copilot hosted models, an existing Copilot terminal/VS Code session, and a fully installed companion adapter remain unverified.

## Historical instruments and rerun limits

The [source snapshots](evidence/2026-10-29-background-wake/manifest.json) preserve the measurement instruments byte-for-byte with a `.snapshot` suffix. They retain machine-specific paths, model choices, temporary directories and the exact API assumptions used that day. They are historical evidence, not installed scripts or a supported replay command. The snapshots are the final harness revisions; some earlier result labels predate the final instrumentation corrections. The manifest distinguishes exact copies from byte-preserved selections of the raw Codex event streams. In particular, the current-session watcher claims only one delivery and lacks an event subscription. The independent model receipt was used to confirm the run and stop that watcher.

A fresh live rerun must first establish its own host endpoint, job/server identity, session ownership, provider availability and cleanup behavior. Use the result predicates in the snapshots: expected markers and correlated events, not merely request acceptance or the presence of a log file. Keep synthetic-provider results separate from real-model results.

## References and stale-when

Official documentation consulted during the investigation:

- [Codex app-server](https://learn.chatgpt.com/docs/app-server): transports, thread lifecycle and standalone tool output.
- [Codex background hooks](https://learn.chatgpt.com/docs/hooks#how-background-hooks-run): idle hook completion waits for the next user turn rather than starting one.
- [Copilot ACP server](https://docs.github.com/en/copilot/reference/copilot-cli-reference/acp-server): ACP initialization, sessions and prompts.
- [Copilot SDK compatibility](https://docs.github.com/en/copilot/how-tos/copilot-sdk/troubleshooting/compatibility): programmatic session and messaging capabilities. Current documentation is not proof of the older installed SDK's lifecycle behavior; the local experiments establish that narrower claim.

**Stale-when:** remeasure after a host, SDK, companion or job-record schema upgrade; an endpoint/ownership change; or a change to continuation, subscription, deduplication or cleanup semantics. Preserve these dated results rather than relabeling them as observations of the replacement build.
