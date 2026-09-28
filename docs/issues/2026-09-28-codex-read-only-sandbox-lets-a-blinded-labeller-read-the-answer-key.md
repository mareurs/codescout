---
id: '4b7cdb0cb35d12a7'
kind: bug
status: open
title: 'BUG: Codex `--sandbox read-only` blocks writes, not reads, so every blinded Codex labeller in the rule-tell campaign could read the answer key'
owners:
- marius
tags:
- cluster/guard-narrower-than-its-name
opened: 2026-09-28
severity: medium
unverified: 'Whether any past campaign call ACTUALLY read outside its workdir is NOT established: no committed log has been scanned. Only the exposure is measured.'
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

Not fixed for the campaign's own sites; the campaign's results are what they are. The System 1 base-rate judge (`scripts/measure/judge.py`) must not inherit the flaw. Ruling R133 of the SDD run (spec dd69959ffc06248a, Amendment 8 when it lands) gives it:
- a judge-owned channel that sends the prompt on stdin;
- a bwrap jail with tmpfs over the home dir, /tmp and /run/user/<uid>;
- a per-run `codex sandbox … test -e` precondition that must FAIL on the repo and the transcript roots before any vote.

## Tests added

None yet for the campaign sites. The judge's channel tests land with Task 9a fix round 1.

## Workarounds

Run codex inside a bwrap jail that hides the corpus (above).

## Resume

To learn whether the exposure was ever used, scan the committed `*.codex.log` files of these campaigns for exec or file-read events outside the call's own workdir. Report counts only. Until then, treat the Codex arms' agreement figures as unblinded-by-mechanism.

## References

- `.superpowers/sdd/2026-09-26-system1-base-rate-measurement/task-9-review.md` I1 (gitignored workspace; the facts are restated above).
