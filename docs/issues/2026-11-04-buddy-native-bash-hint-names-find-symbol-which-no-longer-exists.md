---
id: '9419db55285a2f81'
kind: bug
status: open
title: buddy's native-Bash [cs-hint] tells the agent to use find_symbol, a tool that no longer exists
owners:
- marius
tags:
- cluster/doc-contradicted-by-code
opened: 2026-11-04
related:
- cffe594cbf26fd72
severity: medium
unverified: Delivery count (2,428) and the 491-of-1,350 false-positive figure come from a mining subagent's transcript scan and were not re-checked; no fix applied, no test.
---

# BUG: buddy's native-Bash [cs-hint] tells the agent to use find_symbol, a tool that no longer exists

## Summary

The `buddy` plugin's hook `cs_heuristics.py` emits a `[cs-hint]` after a native Bash `cat`/`head`/`tail`/`sed` on a source file. Its text says "Use `read_file` or `find_symbol`". codescout has no `find_symbol` tool; the symbol tool is `symbols`. The hint therefore names a tool the agent cannot call.

## Class fit

This is `doc-contradicted-by-code`: the hint text is documentation of how to proceed, and the tool list (the code) contradicts it. It differs from the class's usual members because the contradicting text is not a markdown file but a string a hook sends to the model at run time, so no doc-drift check reads it.

## Symptom (Effect)

After a native Bash read of a source file the agent receives: "Use `read_file` or `find_symbol` — Bash on source files bypasses codescout's LSP index." `find_symbol` is not among the codescout tools of the current server (`symbols`, `symbol_at`, `references`, `call_graph`, `grep`, `read_file`, and others). A hint that names an unavailable tool costs the agent a failed lookup or a wrong guess. Under the user-endorsement declaration studied in the hint channel eval (`docs/evals/harness-channel-hint-authority-preregistration.md`, id `cffe594cbf26fd72`, AM-11) such a hint would be obeyed, not ignored.

## Reproduction

1. Read `claude-plugins:044a0d0` `buddy/scripts/cs_heuristics.py`, function `_check_native_bash_on_source`: it returns the string above when a Bash command matches `\b(cat|head|tail|sed)\b.*?(\S+\.\w+)` and the matched token ends in a source extension.
2. List the tools of a current codescout session: no tool named `find_symbol`.

## Environment

claude-plugins `buddy` at `044a0d0`; codescout on `experiments` at `4d659fe57`; 2026-11-04.

## Root cause

The hint text was written when the symbol tool had another name and was not updated when the tools were folded into `symbols`. Inferred from the text and the current tool list; the history of the rename was not traced.

## Evidence

- The function body, read 2026-11-04 (`symbols(name="check", include_body=true)` on the file lists the check order; `_check_native_bash_on_source` returns the `find_symbol` string).
- Delivery counts (a mining subagent's scan of 213 top-level transcripts, 2026-11-04, unique attachment uuids): 2,428 deliveries of this hint. Its regex found 604 of 1,350 mappable deliveries starting with `cat`/`sed`/`head`/`tail` on a file and 491 that are pipelines which only end in `head`/`tail`/`sed` (false positives). Not re-checked. Report: `docs/evals/data/2026-11-04-harness-channel-hints/hint-census/mining-report.md`.

## Hypotheses tried

N/A — the text and the tool list settle it.

## Fix

Not applied. Replace `find_symbol` by `symbols` in the text. Separately, the check fires on pipelines that merely end in `head`/`tail`/`sed` (per the subagent's figure), which a fix should narrow to a command whose first word reads a file.

## Tests added

N/A — no fix applied. A fix should come with a test that every tool name in a `cs_heuristics.py` hint exists in codescout's registered tool list.

## Workarounds

None needed on the agent side: `symbols` is in the tool list and `pre-tool-guard` already redirects native reads of source.

## Resume

1. Fix the text in `claude-plugins/buddy/scripts/cs_heuristics.py`.
2. Add the tool-name test across all six checks.
3. Re-count deliveries from transcripts before and after.

## References

- `claude-plugins:044a0d0` `buddy/scripts/cs_heuristics.py` (`_check_native_bash_on_source`)
- `docs/evals/data/2026-11-04-harness-channel-hints/hint-census/mining-report.md`
- sibling bug `67ee3305dce3224c` (the forgot-restore check keys on the retired `activate_project`)
