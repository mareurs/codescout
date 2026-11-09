# Which hook output field reaches the model and which reaches the user (2026-11-09)

Question: how to show `[cs-hint]` to the user in Claude Code and Codex. Related: [[cs-hint-transcript-mining-20261107]], [[codex-hook-content-trust-20261107]].

## Method
One throwaway hook (`emit.py`, not kept) on PostToolUse/Bash. It picks an output shape from the command text (`echo ARM-A` .. `ARM-E`). One turn, five commands, per harness. Claude Code 2.1.318: `claude -p --model haiku --setting-sources project --settings <hooks> --output-format stream-json --verbose --include-hook-events`. Codex 0.167.0: own `codex app-server` over stdio, isolated `CODEX_HOME` (auth.json symlink, own hooks.json, trusted_hash copied from `hooks/list` `currentHash` into the isolated config.toml), read-only sandbox. The user's real config and session history were not touched.

## Result (same in both harnesses)
| Hook stdout | Model sees it | User-facing channel |
|---|---|---|
| A: `additionalContext` only | yes | CC: only the raw `hook_response` event (needs `--include-hook-events`). Codex: `hook/completed` entry kind `context`. |
| B: `additionalContext` + `systemMessage` | the context only, not the message | CC: `system/informational` "PostToolUse:Bash says: <text>", level notice. Codex: entries `warning` + `context`. |
| C: `systemMessage` only | no | CC: informational notice. Codex: entry `warning`. |
| D: plain stdout | no | nothing user-facing in either |
| E: stderr, exit 0 | no | nothing user-facing in either |

Model-side proof: CC, the model answered `ARM-A: PROBE-A context`, `ARM-B: PROBE-B context`, `NONE` for C, D, E. Codex, the rollout holds developer messages `PROBE-A context` and `PROBE-B context` (kind `hooks.additional_context`) and nothing for C, D, E; the Codex model declined to quote its notes, so its own report was not used.
Codex hook output entry kinds in the app-server schema: `warning`, `stop`, `feedback`, `context`, `error`.

## Limits
- One model per harness (CC haiku, Codex default), one PostToolUse/Bash shape, one run per arm. n=1: this shows the channel mapping, not its stability.
- Whether the interactive TUIs render a `context` entry or an `informational` notice was NOT looked at; only the event streams were.
- Plain `exec --json` in Codex shows no hook entries; use app-server.
- PreToolUse `additionalContext` was not probed here. Codex rollouts hold 99 developer messages `[cs-hint] Use read_file or find_symbol — Bash on source`, which suggests it works there; not checked per event.

## Check on the real configs (2026-11-09, evening)
- Claude Code: `buddy` 0.11.14, `codescout-companion` and `hint-echo` enabled in all four profiles; this session received the real hook's `grep` hint and `Parallel writes` hint.
- Codex 0.167.0, real config, ephemeral thread, read-only sandbox (probe `codex_real.py`, scratchpad, not kept): the real `[cs-hint]` arrived as a `context` entry on `postToolUse`.
- Found: 22 `codescout-companion` hooks were `modified` and every run failed with `plugin contents changed; hook approval required` (same error as [[codex-hook-content-trust-20261107]]). Two files in `~/.codex/plugins/cache/sdd-misc-plugins/codescout-companion/1.21.0/skills/reaching-peer-sessions/` had changed at 20:36 that day. No script in claude-plugins or codescout writes to that cache, and the writer was not identified.
- Repaired with the operator's authorization: only `trusted_content_hash` of the 22 records (declaration hashes unchanged, one new content hash), backup `~/.codex/config.toml.before-hook-trust-20261109`. After: 28 of 28 `trusted`, 16 of 16 hook runs `completed`, one more `context` entry on `sessionStart`. The approved bundle was first the cache as installed, which was OLDER than the repo for `hooks/subagent-guidance.mjs` and `buddy/scripts/cs_heuristics.py`. The operator then asked to update: `codex plugin add codescout-companion@sdd-misc-plugins` and `... buddy@sdd-misc-plugins` (local marketplace; `plugin marketplace upgrade` covers Git sources only) refreshed both cache dirs to be byte-identical with the repo (only `.codex-plugin/` extra), `config.toml` untouched by `add`. That changed the content again, so the same repair ran a second time: 27 records, two content hashes (one per plugin), backup `~/.codex/config.toml.before-hook-trust-20261109-b`. After: 28 of 28 `trusted`, 16 of 16 hook runs `completed`. Pre-update copies of both cache dirs and the config are in the session scratchpad (session-local).
- Trust is bound to content, so any edit under `~/.codex/plugins/cache/` invalidates it again. Re-run the same repair, or re-approve with `/hooks` in Codex.

## A new Codex user gets no hooks until they approve them (2026-11-09)
Measured: a clean `CODEX_HOME` (auth.json symlink), `codex plugin marketplace add <claude-plugins>`, `codex plugin add buddy@sdd-misc-plugins` (config then holds only `[plugins."buddy@sdd-misc-plugins"] enabled = true`). App-server turn with one Bash call: `hooks/list` shows 5 hooks `untrusted`, 0 hook runs, no `[cs-hint]`, the turn completes with no error. With no hook run and no `[cs-hint]` the hooks are absent (with companion it would be 22 more), and a headless run (app-server, `codex exec`) says nothing. The interactive TUI does ask at its next start; see "Interactive TUI check" below.

From the Codex source at tag rust-v0.167.0 (read by a subagent via raw.githubusercontent.com: `plugin_fingerprint.rs`, `plugin_trust.rs`, `discovery.rs`, `dispatcher.rs`, `command_runner.rs`) and the plugin docs:
- The content hash covers the WHOLE plugin root, relative paths, lengths and bytes (no mtime). Skipped: only `__pycache__`, `.venv`, `*.pyc`, `*.egg-info`. README, tests, skills, node_modules, dotfiles are hashed. So any edit anywhere in the bundle, even a skill markdown, invalidates every hook approval of that plugin. This is the mechanism of the 20:36 break.
- Runtime discovery loads a hook only if an approval record exists; the content is compared right before each command runs ("plugin contents changed; hook approval required"; no process is spawned). What a trust error does to the tool call was NOT read (event parsers not fetched); in the probes the turns completed.
- Docs: "Installing or enabling a plugin doesn't automatically trust its hooks." Plugin hooks are never managed (`is_managed: false`), so managed policy cannot trust them. No install-time approval and no `plugin.json` field confers trust. Exemptions found: an internal builtin allowlist (contents not read) and the dangerous `--dangerously-bypass-hook-trust` / `bypass_hook_trust` flag. A legacy approval without `trusted_content_hash` pins the content on first run (trust on first use, baseline in `<config dir>/.hook-trust/`).
- Plugins with lifecycle hooks are not eligible for the public plugin directory.

## State of ~/.codex after installing cs-hint-codex (2026-11-09, late evening)
- `codex plugin add cs-hint-codex@sdd-misc-plugins` (claude-plugins `a632678`), its one hook approved in `config.toml` (`trusted_hash` and `trusted_content_hash` from `hooks/list`), and buddy's PostToolUse hook (`buddy@sdd-misc-plugins:hooks/hooks.json:post_tool_use:0:0`) set `enabled = false` there, because it was the only buddy hook that emits `[cs-hint]` (`hook_helpers.handle_cs_tool_use`). buddy's other four hooks are untouched. Backup `~/.codex/config.toml.before-cs-hint-codex-20261109`; script `install_real.py` in the session scratchpad (not kept).
- Probe on the real config (`codex_real2.py`, ephemeral thread, activate, a concept-shaped codescout `grep`, one `head -3 src/main.rs`): 28 plugin hooks `trusted`, 30 hook runs; `cs-hint-codex` ran 4 times and produced exactly one `[cs-hint]` entry (the grep one); no buddy PostToolUse run. The native-Bash half was not seen on the real config (the model may have used codescout `run_command`); it was seen on the clean home.
- Seen, not investigated: the user-level `~/.codex/hooks.json` PostToolUse hook failed twice with `hook exited with code 1` in that probe (`codescout-wake.mjs`, matcher `mcp__codescout__run_command`).
- A refresh of buddy's heuristics reaches Codex only through `scripts/sync-cs-hint-codex.sh`, a version bump of `cs-hint-codex`, `codex plugin add`, and a renewed approval. buddy's Codex cache is still the pre-fix version (its PostToolUse hook is disabled, so its texts no longer matter there).

## New-user install checks (2026-11-09, last)
- README/CLAUDE.md said `@claude-plugins`; the marketplace registers as `sdd-misc-plugins` (the `name` in `marketplace.json`). On a clean `CLAUDE_CONFIG_DIR` `claude plugin install buddy@claude-plugins` fails ("Plugin buddy not found in marketplace claude-plugins"). Fixed and pushed: claude-plugins `fb78597` (README, with Codex install steps) and `f6f5c60` (CLAUDE.md).
- Codex, clean `CODEX_HOME` (auth.json symlink), source GitHub (`codex plugin marketplace add mareurs/claude-plugins`, HTTPS clone): `plugin add cs-hint-codex@sdd-misc-plugins`, `codex mcp add codescout -- codescout start`, approval written from `hooks/list` (simulated, not `/hooks`): 2 MCP calls, 2 hook runs, one `[cs-hint]` `context` entry for the concept-shaped grep, turn completed in 13 s. An earlier run on a local-path marketplace did not emit `turn/completed` within 300 s although the agent had answered DONE (seen twice, cause not found, not hook related).
- Claude Code, clean profile, source GitHub (`claude plugin marketplace add mareurs/claude-plugins`, clones over SSH): `sdd`, `codescout-companion`, `buddy` installed with `@sdd-misc-plugins`, no approval step. `head -3 src/main.rs`: 13 hook runs, the tool ran (companion did not block, reason not checked), buddy's PostToolUse emitted `[cs-hint]` naming `symbols`. The clean profile has no codescout MCP server, so only the Bash hint could fire.
- Not tested: a user without an SSH key for GitHub on Claude Code, `/hooks` typed by hand in Codex (the start prompt and the Hooks screen were tested on 2026-11-10, see below), the project-level `extraKnownMarketplaces` block of claude-plugins CLAUDE.md, one run and one model per harness.

## Interactive TUI check (2026-11-10)
Clean `CODEX_HOME`, GitHub marketplace, `cs-hint-codex` installed, Codex 0.167.0 in a `tmux` pane (`codex --no-alt-screen`), screen read with `capture-pane`.
- At start the TUI shows "Hooks need review. 1 hook is new or changed. Hooks can run outside the sandbox after you trust them." with: 1 Review hooks, 2 Trust all and continue, 3 Continue without trusting (hooks won't run). "Review hooks" opens a Hooks screen by event (`PostToolUse Installed 1 Active 0 Review 1`, "t trust all · enter review · esc close"); `t` makes it `Active 1`. `config.toml` gets `trusted_hash` and `trusted_content_hash`, the form the scripted approval writes.
- A prompt for `head -3 src/main.rs` ran; the session file holds the `developer` message `[cs-hint] Use read_file or symbols …`; nothing about it is on screen. The user does not see the hint in Codex either (like Claude Code before `hint-echo`).
- Not exercised: choices 2 and 3, `/hooks` typed by hand, other versions; one run. The README, spec and CLAUDE.md of claude-plugins were corrected accordingly (`e5f2548`).

## Consequence
The producing hook can emit `systemMessage` with the same text beside `additionalContext`: it adds a user-visible line in both harnesses and the model still gets one copy. `hint-echo` (claude-plugins `28f69e5`) stays useful for other plugins' context.
