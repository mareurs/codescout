---
kind: bug
status: open
opened: 2026-08-31
closed:
severity: low
owner: marius
related: ["docs/issues/archive/2026-07-28-il3-gate-matches-pipes-inside-heredoc-text.md"]
tags:
  - run_command
  - dangerous-command-gate
  - heredoc
  - false-positive
---

# BUG: the dangerous-command gate scans heredoc bodies, so a commit message that mentions `rm -rf` needs an ack

## Summary

`run_command`'s dangerous-command gate matches on the command text without excluding
heredoc bodies. Writing a commit message that *describes* a deletion — via
`cat > msg.txt <<'EOF' … EOF` — trips the gate, because the literal `rm -rf` appears in
the prose being written to a file.

The command performs no deletion. It writes a text file.

The heredoc carve-out this needs already exists one gate over: the IL-3 pipe gate had the
same defect and it was fixed (`related`), and the source gate carries an explicit heredoc
carve-out (`docs/issues/archive/2026-08-17-heredoc-carve-out-defeated-by-a-pipe-in-the-body.md`
names it). So the remedy is precedented; it appears simply not to have been applied to
this gate.

## Symptom (Effect)

Observed 2026-08-31 while writing a commit message that explained which directories had
been deleted:

```
run_command("cat > $S/msg1.txt <<'EOF'
… deleted with rm -rf, plus five empty marker-file shells. …
EOF
wc -l $S/msg1.txt")

→ { "pending_ack": "@ack_54660096", "reason": "rm with --force or --recursive" }
```

Acknowledging ran the command, which wrote a 41-line text file and deleted nothing.

Cost is one extra round trip. The reason it is still worth filing is the direction:
a gate that fires on prose *describing* a dangerous action trains the reader to
acknowledge without reading, which is the one habit the gate depends on not forming.

## Reproduction

```
run_command("cat > /tmp/x.txt <<'EOF'
we removed it with rm -rf
EOF
wc -l /tmp/x.txt")
```

Expected: runs, writes 1 line. Actual: `pending_ack` with `reason: "rm with --force or --recursive"`.

## Environment

Linux; codescout `experiments` at `1291f468`; `run_command` via MCP.

## Root cause

Not confirmed at the source. Inferred from the observed behaviour plus the shape of the
sibling fixes: the gate matches the dangerous-command patterns against the whole command
string, and unlike the IL-3 and source gates it has no heredoc-body exclusion. Naming this
as inference rather than reading, per this repo's own discipline — I did not open the
matcher.

## Evidence

The only occurrence of `rm` in the failing command was inside the quoted heredoc body,
which `sh` treats as literal data. The command's executable parts were `cat` and `wc`.

## Hypotheses tried

1. **Hypothesis:** the gate fired on something else in that command. **Test:** re-read the
   command; `cat`, `wc`, and a `$S` expansion are the only executable elements, and the
   reported reason names `rm` specifically. **Verdict:** rejected.

## Fix

Not attempted. The likely shape is to reuse whichever heredoc-body exclusion the IL-3 gate
gained, rather than adding a second implementation — see `catalog-sql-hazards` memory for
what happens when one law gets two implementations and only one is gated.

Do not "fix" this by loosening the `rm` pattern; the false positive is about *where* the
gate looks, not what it matches.

## Workarounds

Acknowledge the `@ack_*` handle, having read the command. Or write the message with
`Write`/`create_file` instead of a heredoc.

## Resume

Read the dangerous-command matcher, confirm it lacks the heredoc carve-out the sibling
gates have, and reuse theirs.

## References

- `docs/issues/archive/2026-07-28-il3-gate-matches-pipes-inside-heredoc-text.md` — same defect, adjacent gate, fixed
- `docs/trackers/reconnaissance-patterns.md` — `R-144`, on gates and tests that assert on the wrong substrate
