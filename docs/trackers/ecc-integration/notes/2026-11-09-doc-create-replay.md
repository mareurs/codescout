---
id: '2e7a0d9048d4b1c7'
kind: note
status: done
title: doc(create) replay capture — frozen base and future no-ECC control
tags:
- ecc
- dogfood
- replay
topic: ecc-integration
---

# Replay capture — doc(create), 2026-11-09

## Status

Evidence integrity is indexed by [manifest.json](../evidence/2026-11-09-doc-create-dogfood/manifest.json), with hashes rechecked after capture. Candidate inputs and evaluator-only solution evidence are labeled below; directory separation alone is not an access-control boundary, so the future operator must materialize only candidate inputs in a separate workspace.

Captured at the user's request during the dogfood cycle. **No replay has been started.** The prepared task is a known-defect repair, not a blind bug-discovery benchmark. [Dogfood report](../reports/2026-11-09-doc-create-dogfood.md).

## Exact source baseline

| Property | Value |
|---|---|
| Repository | git@github.com:mareurs/codescout.git |
| Baseline commit | `fe965888377151110f7fd10ab7c20469d6ea2f1c` |
| Baseline tree | `cf72bce1d9232b8361f499fab5136ee696c58a0e` |
| Worktree | `/home/marius/work/claude/codescout/.worktrees/ecc-doc-create-cleanup` |
| Branch | `fix/ecc-doc-create-cleanup-20261109` |
| Source initial state | Tracked source clean at worktree creation; provenance hooks added an untracked counter and lock |
| Source after correction | SHA256 `6614bf9b1184f12398181b7a35d2c2625f8a073159f57ea690a7e2e4ac4c9132` for create.rs |

At capture time, the dogfood worktree HEAD was the baseline commit and the correction was preserved as an uncommitted [source patch](../evidence/2026-11-09-doc-create-dogfood/source.patch), deliberately outside candidate inputs. The user later authorized commit/integration: source commit `561c945d72a3997c972d5ef055e2c9d38e922e67`, integrated on experiments as `b3d02d482eaeb3cabf329f4f37a2d3649a6057b7`, matching patch-id `3e8ba9084781d79228efe0f163c04ce19ffdd203`. The replay starting commit/tree above remain unchanged. The native session's creation metadata names an older parent-checkout commit (`a52d68284f486cd9607f4449242dc92b769e6007`); that is NOT this experiment's baseline.

[base-source.tar.gz](../evidence/2026-11-09-doc-create-dogfood/replay/base-source.tar.gz) preserves committed source independently of future branch movement. Validation compared all 5,370 tracked file/symlink records against archive members and Git blob object IDs: no missing, extra or mismatched members. Archive bytes: 44,551,478; SHA256 `d26cac06b49eda9d352129cc04303f1c6e2bfe3d4c2120d9080402f1fb3d3d6f`. [Validation](../evidence/2026-11-09-doc-create-dogfood/replay/base-archive-validation.json) and [commit object](../evidence/2026-11-09-doc-create-dogfood/replay/base-commit-object.txt) are retained. The archive has no Git history or .git directory; prefer a fresh worktree at the exact commit in the existing repository when replaying history-sensitive tests.

## Captured inputs and runtime

All paths below are under [the replay evidence directory](../evidence/2026-11-09-doc-create-dogfood/replay/).

- `candidate-task.txt`: reconstructed neutral task brief; not falsely described as a one-shot prompt used in this long session. It gives the known suspicion and expected verification work without the chosen fix wording or regression implementation.
- `initial-issue.json`: pre-outcome issue body, captured after claim metadata was set; records what was already known, including the cleanup/reporting suspicion. Give any allowed initial context as detached text rather than granting access to its links into solved reports.
- `task-messages.json`: original task discussion, authorization and later replay request. Personal AGENTS instruction bodies are replaced by hashes in versioned copies because they were explicitly marked private/not tracked; full originals remain locally in the Git-ignored `.codescout/ecc-replay-private-20261109/`.
- `instructions/`: exact worktree CLAUDE.md, AGENTS.md, Cargo manifest/lockfile, toolchain and project config when present.
- `runtime.json`: native session metadata and turn-context model/effort, sanitized relevant default-profile config, config hash, and tool versions. The observed main-session model is `gpt-6.1-sol` with `high` effort. Do not substitute the prior paired experiment's medium setting. Session-creation CLI metadata says 0.160.0; the locally invoked executable reports 0.167.0. They are distinct observations, not a claimed equivalence. Current tools also record Rust 1.99.1, Cargo 1.99.1, Python 3.14.9 and Git 2.56.0.
- `skills/` plus `skill-snapshots.json`: exact installed two-skill ECC pilot package and applied workflow skill bodies/references with hashes. These are evaluator provenance, not no-ECC candidate inputs. The pilot source records its upstream ECC revision and adaptations.
- `codescout-tool-catalog.json`: exposed codescout tool metadata at capture.
- `codescout-server-linux-x86_64.gz` plus `codescout-server-runtime.json`: snapshot/fingerprint of the actual shared MCP server executable used for navigation and commands (the on-disk path had been deleted). It is distinct from the candidate worktree binary exercised in the final MCP probe. Uncompressed SHA256 `9e4db9b94323fb5bc65e3af403904f883459979206077488e448a362291620dc`. Compatible Linux libraries and private state are required; capturing it did not start another server.
- `observed-visible-trace.jsonl.gz`: visible assistant messages and tool calls/results for the current task discussion, without private reasoning. Captured mid-turn, so not a complete final-session transcript.
- `observed-token-events.json`: raw native token events in the captured interval. They contain cumulative history and replay-preparation work; no clean per-task cost or causal saving is inferred.

Auth files, arbitrary MCP environment values, unrelated profile contents and injected instruction text were not copied. Public skill bodies are retained. Personal instruction/transcript originals are kept only in the local Git-ignored private directory, with redacted/hash-bearing copies in this package. Local config reads did not change any profile.

## Future control qualification — prepared, not executed

Interpret “without these skills” as **without the two ECC adapters**, while retaining the existing codescout and other workflow setup. A no-skills-at-all experiment would be a separate condition.

1. Start a fresh candidate session and isolated profile/worktree at the exact baseline; use the frozen task brief and agreed initial knowledge. Keep solution patch, new regression, reports, trace and evaluator skill snapshots inaccessible to the candidate.
2. Keep main model/effort, other skills, codescout tool interface/runtime and verification policy comparable. Record any unavoidable version, cache, load or dependency differences.
3. Remove ECC metadata, skill bodies and invocation paths from the isolated no-ECC profile; verify actual native skills discovery and startup input. A config flag alone is insufficient: the earlier `enabled=false` experiment did not establish disabled discovery. [Failed control evidence](../evidence/2026-11-09-doc-create/failed-disabled-control.json).
4. Use a private catalog/workspace and XDG state so candidate doc/memory searches cannot retrieve this solution from the shared checkout. Do not share the evaluator catalog or activate the solved worktree.
5. Preserve source/config hashes before and after; capture raw trace, final patch, test commands/results, applied mutation outcomes, MCP responses and native usage. Adjudicate output separately from the candidate's self-report.
6. Use normal scratch storage outside the repository. The observed cycle's failed nested-TMPDIR gate is an operational error to retain, not the desired future environment.

The original dogfood ran in a long, already-informed session with compaction, tools and an independent `gpt-6-astra` high-effort reviewer. A fresh replay cannot recreate that entire hidden/runtime context byte-for-byte from these records. It can reproduce the bounded source task and verification environment with explicit differences. One later replay would be descriptive evidence, not a causal proof that skills helped or hurt.
