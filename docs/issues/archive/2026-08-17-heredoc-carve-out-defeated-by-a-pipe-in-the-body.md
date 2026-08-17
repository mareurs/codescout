---
id: f4784780d5413db1
kind: bug
status: fixed
title: 'BUG: the source gate splits on `|` before applying its heredoc carve-out, so one pipe in a heredoc body blocks the command — `git commit -F -` with a message quoting a regex is refused as source-file access'
tags:
- iron-law
- il3
- run-command
- path-security
- false-positive
- heredoc
closed: 2026-08-17
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

**Fixed on `experiments`, commit `4fad1aa4`** — this is the SHA to cite.

Cherry-picked from `0a955491` on `worktree-il3-gate-and-find-lift` (pushed to `origin`).
A fast-forward was **not** available: the two had diverged by one commit each way, because
the bookkeeping commit that recorded this fix (`f273f187`) landed on `experiments` after
the branch was cut. Per the after-cherry-pick rule, the branch-side original orphans on
the next rebase, so `0a955491` is history and `4fad1aa4` is the citation.

**The plan in this file's first draft was wrong in a useful way.** It proposed writing a
`strip_heredocs` helper. That helper already existed: `strip_heredoc_bodies`
(`src/util/path_security.rs:747`), and `detect_il3_violation` had been calling it since
the *pipe* gate's own heredoc fix. Only `check_source_file_access` kept the older
approximation. So this was never missing machinery — it was **one call site that never
adopted an existing contract**, which is the `platform-law-leaks-at-call-sites` shape, and
the fix is an ordering change of two lines:

```rust
let stripped = strip_heredoc_bodies(command);
let segments = split_outside_quotes(&stripped, &["&&", "||", ";", "|"]);
```

The per-segment `if seg.contains("<<") { return false; }` is deleted, now redundant.

**Removing it closed a second, independent bypass** — found by doing the fix rather than
by the original investigation. A here-string puts `<<` in the *same* segment as a real
read, so `cat src/main.rs <<< x` matched the skip and the whole segment was dismissed:
the source read went through. `<<<` takes no body, so there was never anything to excuse.

The doc comment that described the removed mechanism ("Segments containing `<<` are
skipped") is rewritten rather than left behind — it now states that stripping happens
before the split, and why that order is load-bearing.
## Tests added

All in `src/util/path_security.rs`, beside the existing heredoc cases.

- `source_file_access_allows_a_pipe_inside_a_heredoc_body` — the two-line repro.
  Watched RED (returned the refusal hint), then GREEN.
- `source_file_access_allows_a_commit_message_quoting_a_pipe_alternation` — the reported
  symptom. **Two earlier drafts of this test passed before the fix and so reproduced
  nothing**: the bare alternation put no reader name at a segment head, and `plugin.json`
  turns out not to be a source extension. The pair that actually fired was `tail` plus
  `il3-warn-hook.mjs`. Both dead ends are recorded in the test's own comment.
- `source_file_access_blocks_a_source_read_on_a_here_string_line` — the bypass above.
  Written *after* the fix, so **mutation-verified**: restoring the per-segment skip fails
  exactly this test and leaves the other 27 green, which also proves the two mechanisms
  are independent.
- `source_file_access_here_string_does_not_swallow_a_following_read` — guard. The pipe in
  it is load-bearing; the first draft omitted it and failed for an unrelated reason (see
  the sibling bug on newline splitting).
- `source_file_access_does_not_split_on_newlines` — pins a *pre-existing* gap this work
  surfaced but does not fix, asserting today's permissive behavior deliberately as a
  tripwire. See `docs/issues/2026-08-17-source-gate-does-not-split-on-newlines.md`.

Pre-existing tests that had to stay green and did:
`source_file_access_blocks_cat_rs_file_after_heredoc_segment` (a real pipe after a closed
heredoc is still scanned — the case a careless fix breaks) and
`source_file_access_allows_cat_heredoc_with_source_ext_in_content`.

Gate: `cargo fmt` clean, `cargo clippy --all-targets -- -D warnings` clean, `cargo test`
**4069 passed, 0 failed, 45 ignored**.
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

N/A — closed. Fixed on `experiments` at **`4fad1aa4`** and archived 2026-08-17 after wire
verification.

**No pending-master-SHA line, deliberately.** `git rev-list --left-right --count
master...experiments` returns `0 955` — a zero on the left means `master` is a strict
ancestor, so the promotion path is **fast-forward**, not cherry-pick: `master` will move
onto this exact commit and `4fad1aa4` already *is* the master SHA. Writing the
pending-SHA line here would send a later session hunting for a second SHA that will never
exist.

(The cherry-pick that *did* happen was branch → `experiments`, one level down, and is
recorded in *Fix*. That one did mint a new SHA, which is why the citations were re-pointed
from `0a955491` to `4fad1aa4`.)

**Wire-verified after `cargo rb` + `/mcp`**, four cases:

| command | before | after |
|---|---|---|
| `true <<'EOF'` / `x \| head -1 foo.rs` / `EOF` | blocked | **exit 0** |
| `cat src/main.rs <<< x` | **allowed** | blocked |
| `cat <<'EOF'` … `EOF \| cat src/main.rs` | blocked | blocked |

Row two is the bypass this fix closed — it was permitted before. Row three is the guard
that a careless heredoc-stripping fix would break.

One open descendant: `docs/issues/2026-08-17-source-gate-does-not-split-on-newlines.md`,
surfaced by these tests and deliberately not fixed here.
## References

- `docs/trackers/codescout-usage-frictions.md` — U-45 (this friction), U-44 (the hook's
  bounded-LHS list), U-22 (same shape, fixed on the hook side, compound decomposition
  deferred).
- `docs/issues/2026-08-17-source-gate-treats-relative-paths-after-cd-as-in-project.md` —
  sibling defect in the same gate, found the same session.
- `src/util/path_security.rs` — `check_source_file_access`, `split_outside_quotes`,
  `segment_reads_project_source`.
- `src/tools/run_command/inner.rs:308` — the call site.
