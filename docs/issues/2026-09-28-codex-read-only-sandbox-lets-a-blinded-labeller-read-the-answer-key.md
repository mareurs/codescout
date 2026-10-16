---
id: '4b7cdb0cb35d12a7'
kind: bug
status: mitigated
title: 'BUG: Codex `--sandbox read-only` blocks writes, not reads, so every blinded Codex labeller in the rule-tell campaign could read the answer key'
owners:
- marius
tags:
- cluster/guard-narrower-than-its-name
opened: 2026-09-28
closed: 2026-10-16
severity: medium
unverified: 'STANDING 2026-10-16. The 70 committed Codex logs show no read outside the call''s workdir (counts under Resume), but the T-syn-cross generation run of 2026-09-25 predates codex_complete''s log parameter and has no record, so whether it read the answer key cannot be settled. The campaign sites are also still unjailed (frozen history, not patched).'
---

# BUG: Codex `--sandbox read-only` blocks writes, not reads, so every blinded Codex labeller in the rule-tell campaign could read the answer key

## Summary

Three committed call sites run `codex exec --sandbox read-only -C <tmpdir>` and pass the task as files in that tmpdir, plus an argv directive that says in effect "use no other file". Codex's read-only sandbox grants READ access to the whole filesystem. The repo sits on that filesystem, including the gold labels and corrections these calls were blind to. So the blinding rests on an instruction, and nothing enforces it.

## Symptom (Effect)

Nothing visible. A labeller that read the answer key would return a plausible, well-formed answer, with higher agreement. That is exactly the result a good labeller also gives.

## Reproduction

These use no model call; they run local commands only (2026-09-28, codex-cli 0.154.0, bubblewrap 0.12.0):

```
D=<repo>/docs/evals/rule-tell-detection.md
codex sandbox -c sandbox_mode=read-only -- test -e "$D"; echo $?      # 0: readable under the sandbox
bwrap --ro-bind / / --dev /dev --proc /proc --tmpfs "$HOME" --tmpfs /tmp -- test -e "$D"; echo $?   # 1: hidden
```

A Task 9a review found the same with `codex debug prompt-input`: the model-visible permission profile is `file_system type="restricted"` with `<entry access="read"><special>:root</special></entry>`.

## Environment

codex-cli 0.154.0 (`/usr/bin/codex`), Linux, ChatGPT-subscription auth via a fresh CODEX_HOME.

## Root cause

`read-only` names what the sandbox prevents (writes). The campaign used it as if it also confined reads to `-C`. The call sites:
- `docs/evals/data/2026-09-24-rule-tell/stage2/generate_synthetic.py:96-117` `codex_complete`, reached by `stage2/audit_synthetic.py:112`;
- `docs/evals/data/2026-09-24-rule-tell/phase1b/run_labellers.py:129-147` `codex_call`;
- `docs/evals/data/2026-09-24-rule-tell/phase1b/gen_codex_clean.py:61-77` `codex_call`.

## Evidence

The two rc values above. The three call sites were read at HEAD 04973710.

## Hypotheses tried

- Codex feature flags. Per the Task 9a review: `--disable shell_tool` resolves to false, but `unified_exec` stays true under both `--disable` and `-c`. No flag set could be shown to remove every file-reading tool without a model call.

## Fix

Not fixed for the campaign's own sites; the campaign's results are what they are, and those scripts are frozen history (patching them would change what the recorded rows mean). The judge does not inherit the flaw, verified 2026-10-16 at the bytes: `scripts/measure/judge.py` `CodexChannel` (R133) sends the prompt on stdin and runs every codex process in a bwrap jail with tmpfs over the home dir, /tmp and /run/user/<uid>. Constructing it runs `check_jail`, a `codex sandbox ... test -e` per hidden path. Run live against this worktree (codex-cli 0.160.0, bubblewrap 0.13.0; no model call), every hidden path gave rc 1 and the workdir rc 0:

| path | rc | expected |
|---|---|---|
| the repo checkout | 1 | 1 |
| its `docs/evals/rule-tell-detection.md` | 1 | 1 |
| `~/.claude`, `~/.claude-sdd`, `~/.claude-kat` | 1 | 1 |
| `/tmp/claude-<uid>` | 1 | 1 |
| the empty workdir | 0 | 0 |

The unjailed repro in the issue still gives its original result on this machine (`codex sandbox -c sandbox_mode=read-only -- test -e <repo file>` exits 0; the same under `bwrap --tmpfs $HOME --tmpfs /tmp` exits 1), so the premise is current, and so is the need for the jail. The live gate additionally refuses any `codex --version` other than the pinned 0.154.0 (R138); this check ran on 0.160.0 and does not exercise that refusal.

## Tests added

The judge channel's tests landed with Task 9a (`0fc330a0`): `tests/test_measure_judge.py::ChannelTests` (`test_a_vote_goes_on_stdin_inside_the_jail_with_three_paths_bound`, `test_the_jail_precondition_refuses_a_visible_answer_path`, `test_the_precondition_hides_the_repo_the_gate_runs_on`). None for the campaign sites, by decision (frozen).

## Workarounds

Run codex inside a bwrap jail that hides the corpus (above).

## Resume

The scan the previous version of this section asked for was run 2026-10-16, counts only, over the transcript each Codex call leaves on stderr (an `exec` line per shell command; `--ephemeral` keeps no session, so the log is the only record). Every committed log whose header reads `sandbox: read-only`: 70 files (69 `*.codex.log` under `docs/evals/data/2026-09-24-rule-tell/{phase1b/codex-clean-run,stage2/synthetic}` plus `phase1b/audit/raw/codex.a1.log`), holding 82 `exec` events and no other tool event:

- 69 logs, 69 events: exactly one `cat` of the call's own task files, in its own throwaway workdir (`cat task.md`, or `cat codex-clean-prompt.md menu.json`);
- 1 log (`codex.a1.log`, the phase 1b audit labeller), 13 events: one `cat audit-instruction.md menu.json items.jsonl` and 12 `python` heredocs, each of which opens only the relative path `items.jsonl`.

So no committed Codex call read outside its workdir, answer key included. That bounds what was committed. It does not cover the T-syn-cross generation run of 2026-09-25, which predates `codex_complete`'s `log_path` parameter and has no record (its docstring says so), or any run whose log was not kept. The `unverified` field says the same. The shell tool was not the only reader a Codex call could have used (`unified_exec` stays on, per the Task 9a review); lines starting `mcp`, `tool`, `apply_patch`, `web search` or `patch` were searched for in the same logs and none exist, but a tool event printed in a shape this scan does not know would not be counted. This is the absence of a logged read, not proof of no read.

## References

- `.superpowers/sdd/2026-09-26-system1-base-rate-measurement/task-9-review.md` I1 (gitignored workspace; the facts are restated above).
