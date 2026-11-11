---
kind: tracker
status: active
title: "Evidence — the il3 gate refuses scratchpad writes with an unexpanded variable path"
tags:
  - frictions
  - run_command
  - il3
---

# Evidence — the `il3` gate refuses scratchpad writes with an unexpanded `$VAR` path

Ledger entry: FR-15 in [index.md](index.md). Window: 2026-10-13 to 2026-11-09 UTC. Corpus: `.codescout/usage.db`. Source: the `runcommand` miner; the count was re-checked by the lead (96).

## Scale

```
select count(*) from tool_calls
where tool_name='run_command' and err_family='il3_shell_on_source'
  and error_msg like '%unexpanded shell expansion%' and id<298900;   -- 96
```

36 sessions. The family `il3_shell_on_source` has 496 rows in all; the other rows are source reads and recover well.

## What the agent sees

Row 291458: "shell access to source files is blocked — hint: offending clause: `cat > $S/opt.sh <<EOF` … NOTE: this clause contains an unexpanded shell expansion … blocked conservatively." The input set `S` to a scratchpad path and wrote through a heredoc.

## Recovery

5 of 5 sampled rows recovered on the next call (`create_file`, `edit_file`, `run_command`, or an `acknowledge_risk` rerun). Only 8 of 96 were followed by an `acknowledge_risk` rerun within 3 calls.

## Known

`docs/issues/archive/2026-09-10-source-gate-joins-an-unexpanded-var-path-onto-the-project-root.md` (fixed). The rows are after that date. Whether this is a regression, a heredoc-only gap or a second path in the gate is **not verified**: no code was read.

Related: `docs/issues/archive/2026-08-17-source-gate-treats-relative-paths-after-cd-as-in-project.md`.

## Bonus (the `runcommand` miner)

47 rows are `sed -n` or `cat` on `scripts/with-slot.sh`. The gate treats shell reads of the script as source access. Not filed; the next call was a successful `read_file` in the sample.

## Idea (hypothesis, not tested)

When the clause writes to a path that starts with an unexpanded variable, say that the gate cannot resolve the path, and suggest `create_file` (which resolves it) in the first line.

## Not verified

- The regression claim.
- Recovery beyond the 5 sampled rows.
