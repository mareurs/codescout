---
id: '73fd209da1f001e5'
kind: bug
status: open
title: 'BUG: the source gate splits on `|` before applying its heredoc carve-out, so one pipe in a heredoc body blocks the command — `git commit -F -` with a message quoting a regex is refused as source-file access'
tags:
- iron-law
- il3
- run-command
- path-security
- false-positive
- heredoc
opened: 2026-08-17
owner: marius
related:
- docs/trackers/codescout-usage-frictions.md
severity: medium
---

## Summary

`check_source_file_access` splits the command on `&&`, `||`, `;` and `|` **before**
applying its heredoc carve-out, and the carve-out then only skips the one segment that
literally contains `<<`. So any `|` inside a heredoc body chops that body into segments
which are scanned as if they were commands. A segment beginning with a reader name plus a
source-looking filename anywhere in it is refused.

This **blocks**, it does not warn. A `git commit -F -` whose message quotes a regex
alternation is refused with `shell access to source files is blocked` — for a command that
opens no file.

## Symptom (Effect)

Real occurrence, 2026-08-17, committing U-44 in this repo. The message body quoted the
hook regex under discussion, which is an alternation of command names separated by `|`:

```
run_command("cd … && git add <two paths> && git commit -q -F - -- <two paths> <<'MSG'
docs: U-44 — …
  (cargo|npm|pnpm|yarn|python|pytest|go|mvn|gradle|git|find|ls|grep|cat|diff|du|stat|rg|fd)
  … `ls docs | head -2` returns exit 0 …
  … until plugin.json is bumped from 1.16.8 …
MSG")

→ shell access to source files is blocked
  hint: use read_file(path, start_line, end_line) or symbols(path) + symbols(name=…,
        include_body=true) instead. Re-run with acknowledge_risk: true …
```

No source file appears as an argument to anything. The commit was of two markdown files.

### Minimal reproduction — one token apart

Control. Heredoc body names a reader and a `.rs` file, no pipe:

```
run_command("true <<'EOF'\nhead -1 foo.rs\nEOF")
→ exit_code: 0
```

Add `x | ` to the body. Nothing else changes:

```
run_command("true <<'EOF'\nx | head -1 foo.rs\nEOF")
→ shell access to source files is blocked
```

The carve-out holds in the first and is gone in the second. Both measured 2026-08-17 at
`3b4ccac9`.

## Reproduction

Branch `experiments`, HEAD `3b4ccac9`. Live MCP.

1. `run_command("true <<'EOF'\nhead -1 foo.rs\nEOF")` → `exit_code: 0`.
2. `run_command("true <<'EOF'\nx | head -1 foo.rs\nEOF")` → blocked.
3. The delta is `x | ` in a heredoc body. `foo.rs` need not exist — the gate is a string
   test, not a filesystem test.

## Environment

codescout MCP server, `run_command`, IL-3 source gate.
`src/util/path_security.rs`. Rust. Branch `experiments`, project `codescout`.

## Root cause

Measured 2026-08-17 via the one-token pair above; mechanism read at `3b4ccac9`.

`check_source_file_access` (`src/util/path_security.rs:1211-1277`) orders its work
split-then-test:

```rust
// Split on compound-command operators and pipes, respecting quoted strings.
let segments = split_outside_quotes(command, &["&&", "||", ";", "|"]);

let blocked = segments.iter().find(|seg| {
    // Heredoc: the command reads from stdin, not a source file.
    if seg.contains("<<") {
        return false;
    }
    …
    segment_reads_project_source(seg, ext_re, project_root)
})?;
```

A heredoc is a **region**, delimited by `<<DELIM` and a line equal to `DELIM`. By the time
the carve-out runs, the split has already destroyed the region: the body's text is
distributed across however many segments its `|` characters created, and only the first —
the one holding the `<<` token — is skipped. Every later fragment of the same body is
tested as a command.

`split_outside_quotes` respects quotes, so the sibling problem is already solved one level
down. Heredocs simply were not given the same treatment one level up.

**The function's own doc comment states the rule it does not implement:**

> *"Heredocs (`cat <<'EOF'`) read stdin, not a file; **any source extension appearing
> inside the heredoc body is not a filename argument.** Segments containing `<<` are
> skipped — the operator unambiguously means stdin redirection."*

Sentence one is correct and is the intended behavior. Sentence two describes a strictly
narrower mechanism. The gap between the two sentences is the defect, and it is documented
in the same comment — which is why this was cheap to confirm once the code was opened, and
invisible from the outside.

**Why the diagnostic misdirects.** The refusal says source-file access and routes to
`read_file`/`symbols`. Nothing was reading a file, and there is no file to read instead, so
the remedy is not merely unhelpful — it does not correspond to anything the caller did.
Compare the sibling defect in
`docs/issues/2026-08-17-source-gate-treats-relative-paths-after-cd-as-in-project.md`, where
the hint is at least about the right activity.

## Evidence

### The one-token pair

Both calls above, verbatim, at `3b4ccac9`. `exit_code: 0` versus a
`RecoverableError`. `foo.rs` does not exist in the project, which also shows the gate never
consults the filesystem — it is a pure string predicate.

### Third instance of one shape in this ledger

- **U-22** (`fixed-verified`, `codescout-companion:d64749e`) — a literal `|` inside a
  quoted `git commit -m` string, flagged by the hook's pipe detector. Fixed by stripping
  quoted substrings before the pipe regex. Its own closing note scopes out compound
  decomposition: *"the detector continues to treat compound commands as a single CMD."*
- **U-44** — the hook's unbounded-LHS list flags bounded commands.
- **U-45 / this bug** — the server's source gate reads shell syntax out of a heredoc body.

Same shape each time: shell structure inferred from text that is opaque data. Three
instances across two implementations and two gates argues the shape needs one fix, not a
third patch.

## Hypotheses tried

1. **Hypothesis:** this is U-22 recurring, so the existing de-quoting fix should be extended.
   **Test:** read U-22's entry and its fix; compared surfaces.
   **Verdict:** rejected. U-22's fix lives in the companion **hook** and strips **quoted**
   substrings; this is the **server's** source gate and the offending text is an *unquoted*
   heredoc body. De-quoting cannot reach it. U-22 also explicitly deferred compound
   decomposition, which is the mechanism here.
   **Evidence link:** *Third instance of one shape in this ledger*.

2. **Hypothesis:** the heredoc carve-out is missing entirely.
   **Test:** the control call — a heredoc body naming `head -1 foo.rs` with no pipe.
   **Verdict:** rejected; it returns `exit_code: 0`. The carve-out exists and works. It is
   defeated by the split order, which is a narrower and more actionable finding than
   "missing".
   **Evidence link:** *The one-token pair*.

3. **Hypothesis:** the `.rs` file has to exist for the gate to fire.
   **Test:** `foo.rs` does not exist anywhere in the project; the call is still refused.
   **Verdict:** rejected — pure string predicate. Worth recording because it rules out a
   filesystem-stat fix and means the repro needs no fixture.

## Fix

Not yet implemented. Excise heredoc regions **before** splitting:

1. Scan the raw command for `<<-?\s*['"]?(\w+)['"]?`. From the end of that line, drop
   everything through the first subsequent line whose trimmed content equals the
   delimiter.
2. Split the remainder with `split_outside_quotes` exactly as today, and drop the
   now-redundant per-segment `seg.contains("<<")` check — or keep it as a cheap backstop.
3. **Conservative bias, unchanged:** an unterminated heredoc drops to end-of-command
   (matching today's effective behavior for the first segment). Never let a malformed
   heredoc *open* the gate on a segment that follows a real, non-heredoc pipe.
4. Apply to the IL-3 pipe gate too if it shares the split — one heredoc-stripping helper,
   used by both, is the point. Check `src/tools/run_command/inner.rs:308` for the call
   sites.

## Tests added

None yet. Planned, in `src/util/path_security.rs` tests alongside
`check_source_file_access_at_root` (`src/util/path_security.rs:2233-2235`):

- `heredoc_body_with_a_pipe_is_not_scanned_for_source_reads` — the failing case; RED today.
- `heredoc_body_without_a_pipe_stays_allowed` — the control; green today and must stay so,
  since it is what a split-order fix could regress.
- `a_real_pipe_after_a_closed_heredoc_is_still_scanned` — the one a careless fix breaks:
  `true <<'EOF'\nbody\nEOF\ncat src/main.rs | head -1` must remain refused.
- `unterminated_heredoc_swallows_to_end_of_command` — pins the conservative bias.
- Mutation check: restore the split-then-test order and confirm only the first test goes
  red.

The third test is the important one. The fix removes text from the gate's view, so the
risk it introduces is a genuine violation hidden behind a heredoc.

## Workarounds

Write the message or payload to a file and pass the path:

```
git commit -F /abs/path/to/message.txt
```

The command string then contains neither the pipes nor any filename that looks like source.
This is the same workaround U-22 documented; it is still required for the case U-22's fix
did not cover.

For non-commit cases, `acknowledge_risk: true` bypasses the gate, but prefer the file route
— it keeps the gate armed for the rest of the command.

## Resume

Add a `strip_heredocs(command) -> String` helper in `src/util/path_security.rs` and call it
at the top of `check_source_file_access`, before `split_outside_quotes`. Write
`heredoc_body_with_a_pipe_is_not_scanned_for_source_reads` first and watch it fail on the
two-line repro. Then check whether the IL-3 pipe gate performs its own split
(`src/tools/run_command/inner.rs` around the `check_source_file_access` call at :308) and
route it through the same helper, so the fix covers both gates rather than one.

## References

- `docs/trackers/codescout-usage-frictions.md` — U-45 (this friction), U-44 (the hook's
  bounded-LHS list), U-22 (same shape, fixed on the hook side, compound decomposition
  deferred).
- `docs/issues/2026-08-17-source-gate-treats-relative-paths-after-cd-as-in-project.md` —
  sibling defect in the same gate, found the same session.
- `src/util/path_security.rs` — `check_source_file_access`, `split_outside_quotes`,
  `segment_reads_project_source`.
- `src/tools/run_command/inner.rs:308` — the call site.

