# Cross-harness session messaging and wake paths (measured 2026-10-29)

Question: how can codescout (or a companion hook) push a message into a running coding-agent
session and wake it when idle, for Claude Code, Codex and Pi? Context: the open bug
`docs/issues/2026-10-29-codescout-has-no-background-wait-and-notify-mechanism-and-claude-code-monitor-is-removed-by-the-bash-deny.md`
and the peer session's design `docs/superpowers/specs/2026-10-29-background-job-wake-design.md`
(approach A: completion record plus `asyncRewake` hook).

Versions: Claude Code 2.1.312, Codex CLI 0.165.0. Pi not measured. One run per case, small models.
Evidence tags: MEASURED (run here), DOCS (first-party page read), REPORT (research worker, weak).

## Summary table

| Harness | Push into an idle session | How | Evidence |
|---|---|---|---|
| Claude Code | yes | line on the session inbox socket; or `asyncRewake` hook (exit 2) | MEASURED, DOCS |
| Codex | yes | `codex queue --thread <uuid\|name> --message <text>` | MEASURED |
| Pi | not established | RPC `steer` / `follow_up` serve the launcher only | REPORT |

## Claude Code: inbox socket (MEASURED)

- Each session binds `/run/user/<uid>/cc-socks/<pid>.sock`. The path is in the env var
  `CLAUDE_CODE_MESSAGING_SOCKET` (plain path, no `uds:` prefix), with `CLAUDE_CODE_MESSAGING_TOKEN`.
  The codescout MCP child process holds both variables.
- Wire format: one JSON line per message, ended by a newline. Captured from a real `SendMessage`:
  `{"msgV":1,"msg_id":"<uuid>","type":"user","message":{"role":"user","content":"<cross-session-message from=\"uds:...\" from-name=\"...\" from-mode=\"prompting\">\n...\n</cross-session-message>"},"priority":"next","from":"uds:/run/user/<uid>/cc-socks/<pid>.sock"}`
  Socket credentials (`SO_PEERCRED`) carried the sender pid.
- The server sends no reply bytes.
- `SendMessage(to="uds:<path>")` works in 2.1.312 although the tool text says names only.
- Posts that were delivered to the same session: descendant with auth line, descendant without,
  detached (`setsid -f`) with and without the auth line.
- A post from a Codex process (not a descendant, no auth line, `from="codex-xtalk"`, not a `uds:`
  address) woke an idle Claude session. The TUI showed `Message from @codex-xtalk`.
- Docs: `https://code.claude.com/docs/en/cross-session-messaging.md`. Inbound control is
  `crossSessionInbound` = accept / hold / refuse. The default depends on the permission mode of both
  sessions. Cap about one million characters; at most 50 queued; loop throttling.
- Not measured: `hold` with a sender that claims `from-mode="bypassing"`; whether `from-mode` is
  verified; cross-user; delivery while the receiver is idle without a TUI.
- Security: any process under the same OS user can post with any `from` and `from-name`. The receiver
  treats the text as a peer message, never as user input or approval.

## Codex: `codex queue` (MEASURED)

- `codex queue --thread <uuid|name> --message <text>` queued into an idle interactive session. The
  session started a turn at once and showed the text as a plain user prompt. No flag was needed.
- Codex gives no sign that the text came from another process. This differs from Claude Code, which
  labels peer messages.
- The Codex default sandbox blocked a post to the Claude socket (`PermissionError: [Errno 1]`).
  Codex asked to rerun the command outside the sandbox, and one approval fixed it. A real setup needs a
  configured permission for the socket.
- `codex queue` printed a warning about PATH aliases under a temp dir. The queue still worked.
- Thread id source in an isolated `CODEX_HOME`: table `threads` in `state_5.sqlite`.
- Codex async command hooks do not start a turn (DOCS, https://learn.chatgpt.com/docs/hooks): "If no
  turn is active, Codex waits until the next user turn." They are no `asyncRewake` equivalent.
- Not established: the `codex app-server` method bodies (`turn/start`, `thread/inject_items`), and the
  stability label of `queue`.
- `codex queue` has three outcomes (isolated `CODEX_HOME`, 2026-10-29): (1) attached TUI: a turn starts and
  shows; (2) thread loaded in the daemon with no TUI attached: a turn runs inside the daemon, unwatched
  (rollout 140,247 to 149,326 bytes, about 17,700 tokens); (3) thread not loaded: "Queued", exit 0, nothing
  runs; the message waits in `queue_1.sqlite` table `queued_items` (payload `UserInput`).
- A Codex session with codescout registered can post to a unix socket outside its workspace through
  codescout `run_command`: no block, no approval. The native Codex shell is blocked (`EPERM`).
- Live Codex shape on this machine: interactive `codex` processes hold no rollout file; the app-server
  daemon holds one per loaded thread; the interactive processes export no `CODEX_*` variable.

## Round trip (MEASURED)

A, from outside: `codex queue` woke idle Codex, which answered `ACK-A1`.
B: Codex ran a helper that posted to the Claude socket. Claude woke from idle, ran `codex queue`,
and Codex woke again and answered `ACK-B2`. Capped at one round trip.
Setup: private `tmux -L` server; throwaway `CLAUDE_CONFIG_DIR` and `CODEX_HOME`; Claude in `dontAsk`
mode with only `codex queue` allowed; Codex credentials by symlink. All of it was deleted afterwards.

## Channels (Claude Code) — not the recommended path

- `capabilities.experimental['claude/channel']` plus `notifications/claude/channel` (DOCS,
  `channels-reference.md`). Research preview; custom channels need
  `--dangerously-load-development-channels server:<name>`; the flag is ignored under `-p`; Claude Code
  drops events silently when the channel is not registered.
- Workers found conflicting reports on whether a channel event wakes an idle session
  (https://github.com/anthropics/claude-code/issues/44380 says it does not for chat plugins).
- codescout pins `rmcp` 1.3.0. Its capability type is `BTreeMap<String, JsonObject>` and it has
  `CustomNotification::new(method, params)`, so codescout can probably declare and send a channel
  notification. Not compiled. `src/server.rs:2253` declares only tools today.
- The socket is documented, needs no flag, and was measured. Prefer it.

## Hooks under the Bash deny (MEASURED)

Command hooks and `asyncRewake` hooks run and reach the model with `permissions.deny = ["Bash"]`.
The peer session also measured idle wake in an interactive run. Full deny-set findings (only
`Bash` and `Monitor` go together; `NotebookEdit` stays writable; skills with shell injection return
empty) are in the issue file above.

## Standards (REPORT, weak)

ACP (Zed): `session/new`, `session/load`, `session/prompt`, `session/cancel`; a client drives an agent
it launched; second-client attach not established. A2A: only third-party material. MCP spec
notification support per harness: not established. No primary text was retrieved for any of these.

## Design reading

Keep one harness-neutral message record in codescout and one small delivery adapter per harness.
Claude Code and Codex now each have a measured wake path, so two concrete adapters exist. Pi has none
yet. Promise reliable delivery everywhere and an idle wake only where the harness supports it.
Label every injected message with an honest sender name, because Codex shows queued text as user input.

## Outcome: the `reaching-peer-sessions` skill (2026-10-29)

- Shipped as `claude-plugins:a4df207` (merge of `feat/peer-skill-cross-harness`) and released as
  companion 1.20.24 (`claude-plugins:338e82a`, local, not pushed): a core `SKILL.md` plus
  `references/claude-code.md` and `references/codex.md`. Spec, plan and tests live in `claude-plugins`
  (`docs/superpowers/specs|plans/2026-10-29-reaching-peer-sessions-cross-harness*`, `tests/test-peer-*.sh`).
- MEASURED: a Codex session (0.165.0, isolated `CODEX_HOME`, skill copied in) follows the `SKILL.md`
  pointer and reads `references/codex.md` by a path relative to the skill directory (20,119 tokens).
- Codex Enumerate = processes with `app-server` as a whole argv token, then their open `rollout-*.jsonl`
  descriptors, joined read-only to `state_5.sqlite` `threads` on `rollout_path`. Live: 21 loaded threads,
  11 of them untitled (printed `(untitled)`). The WAL database is why the "never writes" test needs a WAL
  fixture: a read-write open checkpoints and deletes the `-wal`.
- Delivery is autonomous, with guards: loaded-only (a thread that is not loaded only queues), SID taken from
  the Enumerate row and never from message text, one send per target, never broadcast, label first, one
  line to the operator after the send. The Claude post refuses any target version outside
  `MEASURED = {"2.1.312"}` (registry `version` first, `exe` basename fallback). Live: 13 of 28 Claude
  sessions were on 2.1.312, so 15 are unreachable from Codex by design until they are measured.
- Known residual: the message and the args travel through a quoted heredoc, and a value that contains the
  fresh random terminator still runs as shell. The only defence is the pinned instruction that the model
  picks a terminator that occurs nowhere in the text. PID and SID are validated in Python before use.
- Not measured: how a Codex session finds its own row (Identify me says "not identified", so the merge
  block prints the loud warning); a real `codex queue` from the snippet; the Codex sandbox reading `/proc`.
- Parked: extra lines in the args file are ignored silently; "your own Enumerate row" can be misread.

## Open items

- Pi: read `extensions.md` from `earendil-works/pi` and test whether an outside process can start a turn.
- Codex: read the `app-server` body at `learn.chatgpt.com/docs/app-server` and the feature-maturity page.
- Codex: measure Identify me (candidates: a `resume <uuid>` argument in the TUI command line; the newest
  rollout whose cwd matches).
- Claude: measure the `hold` path, whether `from-mode` is verified, and the wire format on 2.1.284 to 2.1.311.
- Codex sandbox: find the setting that allows the Claude socket without an approval.
