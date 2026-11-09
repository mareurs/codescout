# Pilot verification — 2026-11-09

## Package and scope

Branch feat/ecc-codex-codescout in the claude-plugins worktree, based on 85720782f0efbaba3a2fc607f7a44317fc95adec. New native Codex package only; the Claude marketplace, normal Codex profiles, upstream ECC checkout and existing plugin files were not changed. Pilot file writes were confined to this worktree. The main checkout's original status entries remained present at final inspection; it also showed additional unrelated issue-file changes from concurrent work. No file-content equality was inferred from status output.

## Executed checks

- `python3 ecc-codescout/tests/test_package.py`: four tests passed, including real native marketplace add, plugin add and app-server skills/list under a disposable CODEX_HOME. Both installed namespaced skills were enabled, their cache paths were under that profile, and their SKILL.md and agents/openai.yaml bytes matched this worktree. Returned default prompts invoke the corresponding discovered namespaced skill.
- skill-creator quick_validate.py: both adapted skills passed.
- `bash scripts/check-versions.sh`: exit 0; existing marketplace/plugin versions remained consistent.
- Fresh-context application of ecc-spec-miner created the bounded retry-observed.md draft through librarian.
- Fresh-context read-only application of ecc-silent-failure-review confirmed the planted required_sync violation and accepted the contractual optional_preview fallback.
- A real authenticated Codex 0.167.0 execution in a disposable profile loaded the native review skill, inspected the fixture through codescout and returned the same contract distinction. That original scratch run's incomplete caller tracing is recorded in failure-review.md; the corrected native rerun is recorded below.

## Applied package mutation checks

The independent reviewer applied one mutation at a time, ran the complete four-test package suite, then restored the original bytes and verified hashes.

| Candidate | Initial observed outcome |
|---|---|
| Remove plugin manifest skills declaration | Killed |
| Add plugin manifest hooks object | Killed |
| Redirect marketplace source to missing-package | Killed |
| Change spec default prompt to nonexistent skill | Survived |

Initial campaign: 3 killed, 1 survived, 0 inconclusive. The surviving mutation was observed before its regression assertion was added. The parent then added assertions on native defaultPrompt and installed metadata bytes, reapplied that exact mutation and observed the new assertion fail. After restoring metadata, all four package tests passed. This was one additional retest, not a repeated four-candidate campaign. Independent follow-up review inspected the fix and closed the finding.

## Corrected native workflows and navigation — 2026-11-09

The fixture now declares Python in .codescout/project.toml and includes the actual commit_sync → required_sync call. A local .gitignore exception keeps that intentional configuration visible to Git despite the repository's general .codescout exclusion.

- Full pilot suite: `CODESCOUT_BIN=/home/marius/.cargo/bin/codescout python3 -m unittest discover -s ecc-codescout/tests -p 'test_*.py'` — 12 tests passed, no skips, 41.857 seconds.
- Navigation tests start the actual installed codescout, retain an unconfigured negative control, and assert the actual reference site and caller graph. The configuration and caller additions each followed observed failing regressions.
- A real-server scratch-spec test first failed the shared-catalog temporary-workspace guard, then passed with a private catalog. The native Codex child additionally requires explicit MCP environment configuration; a direct probe alone did not establish forwarding. A second native run caught this gap.
- The corrected authenticated `tests/live_smoke.py --live` run completed both native skills with exit 0. Completed MCP events include symbols, references and call_graph in each mode. The review traced the actual LSP edge and confirmed required_sync while accepting optional_preview. The spec was saved through doc, found again and read from the same isolated catalog as kind=spec/status=draft.
- Evidence: [native-report.json](native-report.json), [captured native draft](native-retry-observed.md), and [failure review](failure-review.md). Raw JSONL and stderr remain in /tmp/ecc-native-corrected-20261109-3; the private authenticated profile and temporary catalog were removed.
- Codescout state isolation uses private LIBRARIAN_DB/LIBRARIAN_WORKSPACE, sqlite-vec artifact storage and XDG directories. Those six values are also emitted in the temporary [mcp_servers.codescout.env] table; no temporary-workspace bypass is enabled. The installed CLI's --env option and [official MCP documentation](https://learn.chatgpt.com/docs/extend/mcp?surface=cli) establish that explicit launch configuration.
- Both skill validators and repository version-consistency check passed again.

Independent follow-up review applied five individual mutations: remove Python declaration; remove caller edge; disable isError rejection; remove private database override under an adversarial inherited catalog; remove explicit native MCP environment table. Each was observed killed by its relevant test, then exact original hashes were restored. Outcome: 5 killed, 0 survived, 0 inconclusive across these follow-up checks. Restored targeted tests passed; the final complete suite above ran after all source restoration. No outstanding review findings.

## Limits

These are pilot portability, native loading and bounded application checks. They do not establish lower cost/latency, improved accuracy, general reliability, or full coexistence across every installed workflow. The corrected run completes the fixture caller tracing and spec-persistence checks; it does not retroactively change the original incomplete run. No fixture behavior tests were executed. Structural tests and applied packaging/navigation/configuration mutations do not certify the semantic effectiveness of prompt guidance.

No commit, merge, push, release or normal-profile install was performed. The worktree remains available for further experiments.
