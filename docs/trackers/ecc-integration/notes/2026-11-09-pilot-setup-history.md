---
id: d656c93a38dd43b1
kind: note
status: done
title: ECC pilot setup history — migrated project memory
tags:
- ecc
- history
topic: ecc-integration
---

# Historical pilot setup notes — captured 2026-11-09

This is a retained snapshot of project memory before moving ongoing evaluation to this folder. Earlier pending/no-install statements describe their historical stage; the [current evaluation index](../README.md) and dated reports supersede them. The original text follows without dropping its setup/mutation/provenance history.

# ECC Codex pilot — implemented 2026-11-09

User authorized a worktree pilot after research/ecc-integration-20261109. New worktree /home/marius/work/claude/claude-plugins.worktrees/ecc-codex-codescout on feat/ecc-codex-codescout, based on claude-plugins main 85720782f0efbaba3a2fc607f7a44317fc95adec. The upstream ECC clone remains /home/marius/work/claude/ecc, pinned d2ef2680594f9a17526a6e1a2e18b3d242c5d17b. Implementation is uncommitted in this worktree; no merge/push/release/global install was performed.

The optional native Codex package ecc-codescout contains ecc-spec-miner and ecc-silent-failure-review, a local ecc-codescout-pilot marketplace, upstream file hashes and retained MIT license. No Claude marketplace registration, hooks, MCP server bundle, model policy, auto-delegation or second memory backend. Both skills route source work through codescout and artifact persistence through librarian. README contains separate CODEX_HOME installation/use instructions.

Four package tests passed, including actual Codex 0.167.0 marketplace add/plugin add/app-server skills/list in disposable profile, enabled namespaced skill discovery, installed SKILL.md and agents/openai.yaml byte identity, and defaultPrompt invocation assertions. Both skill-creator validators and existing check-versions.sh passed. An independent reviewer applied 4 package mutants: 3 killed, 1 survived (default_prompt names nonexistent skill). After observing that survivor, parent added native metadata assertions, reapplied exact mutation and observed failure, restored original and observed all 4 tests pass; follow-up reviewer closed finding. This was one survivor retest, not a repeated full campaign.

Fresh-context actual fixture applications: spec miner saved librarian draft ecc-codescout/evidence/retry-observed.md; review confirmed required_sync's prohibited catch-all empty fallback and accepted optional_preview's contractual timeout fallback. A real authenticated Codex CLI execution in temporary profile with connected codescout loaded the native review skill and returned the same distinction. Private temporary auth copy was cleaned; routine package tests use no auth/model calls. In that native scratch environment references/call_graph were reported unavailable; an independent initialize+tools/list against /home/marius/.cargo/bin/codescout start in a fresh temporary directory likewise advertised symbols but not references/call_graph/symbol_at. Follow-up diagnosis established that the scratch project had languages=[]: current_capabilities checks configured project languages for a registered LSP config; advertised_mcp_tools filters RequiresLsp tools. Plain .py presence, activating the same unconfigured fixture and --project alone all kept languages=[] and omitted the tools. With the same binary, the same fixture declared [project] languages=["python"] in .codescout/project.toml and tools/list included references/call_graph/symbol_at. This is an incomplete test-fixture initialization, not an established ECC limitation. The first configured follow-up proved tool advertisement only. That original native smoke had incomplete caller tracing; the later corrected native execution below completes caller tracing and persistence without establishing general workflow parity or benchmark improvement.

Evidence and usage: ecc-codescout/README.md, ecc-codescout/evidence/verification.md, failure-review.md, retry-observed.md. Managed plan docs/superpowers/plans/2026-11-09-ecc-codex-codescout.md (8f9fadd44fb9d948) and session log docs/trackers/ecc-codex-codescout-session-log.md (0632dd9d30c863d2, worktree-specific ids). Main claude-plugins original status entries remained present; final status also showed additional unrelated issue-file changes from concurrent work. Pilot writes were confined to the worktree; status output does not prove file-content equality. Future work can use this tree for real repo trials; no measured accuracy/cost/latency gains yet.

## Corrected native workflow follow-up — 2026-11-09

After user requested repair and continuation, the fixture gained explicit Python configuration and a real commit_sync → required_sync caller. A local fixtures/.gitignore exception makes .codescout/project.toml visible despite repository exclusion. Native direct probes now assert the actual reference line and caller graph, with an unconfigured negative control. A scratch-spec regression exposed shared catalog inheritance; private LIBRARIAN_DB/LIBRARIAN_WORKSPACE, sqlite-vec artifact backend and XDG state fixed direct probes. Real Codex still failed because ambient state env did not cross MCP filtering. Explicit [mcp_servers.codescout.env] with those six state variables fixed the actual native launch. Do not enable CODESCOUT_ALLOW_TEMP_WORKSPACE to bypass the catalog guard.

Successful corrected both-mode run: /tmp/ecc-native-corrected-20261109-3, exit 0. Both native skills used actual completed symbols/references/call_graph calls. Review traced the actual LSP edge and confirmed required_sync while accepting optional_preview. Spec saved through doc, found and read back as kind=spec/status=draft. Machine evidence retained in ecc-codescout/evidence/native-report.json; native draft imported as separate artifact 2595ef98f1f655ad at evidence/native-retry-observed.md, with links rebased to retained fixture. Private temp profile/auth copy/catalog cleaned; normal profiles unchanged.

Final pilot suite: 12 tests passed, no skips, 41.857 seconds, using CODESCOUT_BIN=/home/marius/.cargo/bin/codescout. Both skill validators and version-consistency passed. Independent follow-up applied five mutations individually (language declaration, caller edge, MCP isError rejection, private DB override, explicit MCP env table): 5 killed, 0 survived, exact hashes restored, no outstanding review findings. Managed plan done; session-log F-2/F-3/F-4 and W-3 document setup gaps and completed workflows. No fixture behavior tests executed; no measured cost/latency/accuracy gains. Work remains uncommitted in the same isolated worktree, no global install/merge/push/release.

## Default-profile activation — 2026-11-09 (latest state)

User explicitly requested activating only the pilot in the default profile, making a backup, then restarting Codex manually. Installed ecc-codescout@ecc-codescout-pilot version 0.1.0 into /home/marius/.codex/plugins/cache/ecc-codescout-pilot/ecc-codescout/0.1.0 using native marketplace add/plugin add. Marketplace source remains the integration worktree ecc-codescout directory. This supersedes earlier isolated-only/no-global-install status statements; no full ECC integration was requested.

Backup: /home/marius/.codex/backups/ecc-codescout-20261109T105201Z, containing exact pre-install config.toml (SHA-256 a76fea25e7d26afc2ddb69fd39f0e903f647e3e51ba03e7d3aee58033cbdeb3a, mode 0600), before.json, after.json, README.md, and revert.sh. To revert surgically: bash /home/marius/.codex/backups/ecc-codescout-20261109T105201Z/revert.sh. It removes only the new plugin/cache and marketplace entries, preserving later unrelated config edits. Script syntax and backup hash verified; revert itself not executed. No auth/history replacement.

Native app-server skills/list for codescout returned both namespaced skills enabled, with installed SKILL.md and agents/openai.yaml bytes matching the worktree; default prompts name the correct skills. No skill-load errors. Parsed config comparison showed only added marketplaces.ecc-codescout-pilot and plugins.ecc-codescout@ecc-codescout-pilot; all prior settings unchanged semantically. Only ~/.codex exists on this machine: checks found no ~/.Codex, ~/.Codex-sdd, ~/.Codex-kat or lowercase alternate profiles. Do not invent/create missing profiles. User will restart Codex; real-code doc(create) pilot comparison is pending after restart.
