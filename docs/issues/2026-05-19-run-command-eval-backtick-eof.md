---
status: wontfix
opened: 2026-05-19
closed: 2026-05-19
severity: low
owner: marius
related: []
tags: ["run_command", "host-harness", "misleading-output"]
kind: bug
---

# BUG: Misleading `run_command` output — host Bash buffer content read by `tail` looks like a fresh codescout eval error

## Summary

User saw `run_command("tail -50 .../b41zejzof.output")` return stdout `[stderr] /bin/bash: eval: line 1: unexpected EOF while looking for matching ``'` and assumed run_command itself crashed with eval EOF. xxd confirms the buffer file's 80 bytes literally are that error string — tail faithfully printed it. Two distinct facts: (a) run_command worked correctly; (b) some **earlier** host-side Bash/Monitor task (path namespace `/tmp/claude-1000/<workdir>/<session>/tasks/<id>.output`, ID `b41zejzof` is harness format not codescout's `@cmd_<8hex>`) genuinely failed with bash eval EOF — unknown invocation, possibly a `bash -c "eval ..."` wrapper around a command that ended mid-backtick.

**Action:** misread by reader, not bug in codescout. Re-classify as a documentation / UX friction — raw tail of opaque buffer files invites this confusion. Real upstream eval-EOF (in host harness) deferred until we can identify the originating call.
## Symptom (Effect)

Invocation:

```
run_command(command: "tail -50 /tmp/claude-1000/-home-marius-work-claude-code-explorer/feb80bd2-dac0-4c97-8f70-5eddb9eed826/tasks/b41zejzof.output")
```

Response:

```
{
  "exit_code": 0,
  "stdout": "[stderr] /bin/bash: eval: line 1: unexpected EOF while looking for matching `\`'\n"
}
```

Notable: `exit_code: 0` but only stderr emitted, no file content. The error is bash-side parse failure before `tail` runs.

## Reproduction
Not yet reproducible in isolation — best lead: the target file (`b41zejzof.output`) likely contains a stray backtick that something on the run_command pipeline read and re-eval'd, OR the wrapper double-evals the command string. Repro candidate:

1. Write `echo '`unterminated' > /tmp/probe.out`
2. `run_command(command: "tail -50 /tmp/probe.out")` — if this reproduces the EOF, the wrapper is re-evaluating output. If not, the bug is in the path-handling layer for the persisted-output buffer directory.

## Environment
- branch: `experiments` @ bfa2f8bc
- MCP via stdio, release binary
- Linux 7.0.0-15-generic

## Root cause

Two-layer misread:

1. **No bug in codescout's run_command.** `src/platform/unix.rs:65` wraps via `sh -c "$cmd"`. No `eval` builtin anywhere. The string `eval:` cannot originate from codescout's shell wrapper.
2. **Buffer file content is a captured stderr from an earlier host-side Bash task** (Claude Code harness `tasks/` namespace, not codescout's `@cmd_*` buffers). That earlier task invoked `eval` and bash reported unbalanced backtick. The 80 bytes persisted; later `tail` reads them; reader misattributes the error to run_command.

Upstream bug (the actual eval EOF) lives outside codescout — in whichever harness/wrapper invoked `bash -c "eval ..."` with malformed input. Not in scope of this repo unless we can identify the originating command.
## Evidence

### E1 — Failing invocation
Quoted in Symptom above. The escaped `` `\` `` in the JSON suggests bash saw a literal `` ` `` and consumed to EOF.

## Hypotheses tried

1. **Hypothesis:** `tail` invocation itself triggers bash eval EOF.
   **Test:** `tail -50 /tmp/probe.out` where probe contains a literal backtick → exit 0, full content.
   **Verdict:** rejected.

2. **Hypothesis:** hyphen-prefixed dir name like `/tmp/-home-...` confuses arg parsing.
   **Test:** `mkdir /tmp/-home-test-dir/tasks; tail -50 .../probe.output` → exit 0, full content.
   **Verdict:** rejected.

3. **Hypothesis:** target file content is itself an error string from a prior failed run_command.
   **Test:** `xxd .../b41zejzof.output` shows 80 bytes = literal eval-EOF error text with `[stderr]` prefix.
   **Verdict:** confirmed. Tail is not the bug — it printed real file content.

4. **Hypothesis (open):** an earlier `run_command` call wrapped its command in a way that bash's `eval` saw unbalanced backtick. Possibly IL3 pipeline-enforcement wrapper from commit 2c3badfc.
   **Test:** TBD. Need to identify which earlier call wrote the buffer (timestamp of `b41zejzof.output`) and what command it ran.
   **Verdict:** deferred.
## Fix
TBD pending repro.

## Tests added
N/A — no fix yet.

## Workarounds
- Use `read_file` for buffer files instead of `tail` via `run_command`.
- Or `head -c N` which avoids stdin/eval pipelines if the bug is content-driven.

## Resume

1. Close as `wontfix` from codescout's side once user confirms understanding — it's a host-harness artifact, not a codescout tool bug.
2. If reader-confusion is judged worth fixing UX-side: `run_command` could prefix-strip stdout that **is itself** a captured error buffer (e.g. detect `[stderr] /bin/bash: ...` head-of-file and add a `source: "buffer-content"` hint to the response). Low priority — single datapoint, decoration not design (two-concretes rule).
3. If the upstream host-side eval EOF recurs: capture the exact command being eval'd (likely a Monitor/background invocation in `.claude/` plugin code), file separately under host plugin tracker not here.
## References
- Originating session: this conversation, 2026-05-19
- Related: IL3 work in 2c3badfc, `src/tools/run_command/tests.rs` (currently dirty)
