---
kind: bug
status: fixed
tags:
- run_command
- dangerous-command-gate
- heredoc
- false-positive
- cluster/addressing-without-an-escape-hatch
closed: 2026-08-31
opened: 2026-08-31
owner: marius
related:
- docs/issues/archive/2026-07-28-il3-gate-matches-pipes-inside-heredoc-text.md
severity: low
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

**Confirmed at the source 2026-08-31**, replacing the inference this section previously
carried. `strip_heredoc_bodies` (`src/util/path_security.rs`) is called by
`commit_message_backtick_hazard`, `detect_il3_violation` and `check_source_file_access`, and
**not** by `is_dangerous_command`. The callers (`run_command/inner.rs:298`,
`run_command/interactive.rs:45`) pass the raw command, so nothing strips upstream either.

The inference was right about the mechanism and **wrong about what follows from it**, which
is the part worth recording:

**1. The absence is deliberate, and already pinned.**
`quoted_dangerous_text_is_flagged_and_the_raw_pass_did_it_first` asserts this exact false
positive, and its doc comment says why it exists: *"asserted so it is a recorded property
rather than a surprise — and so that a future attempt to remove it has a test to change."*
This report was that attempt. Position-blindness is what catches quote evasion — `grep 'rm'
'-rf'` matches only once normalization rejoins the tokens — and a flag is not a refusal.

**2. The sibling carve-out does not transfer, and applying it would open a hole.**
The IL-3 and source-file gates analyse **syntax**: a pipe in a body is not a pipeline stage,
a `.rs` in a body is not a filename argument. A body is unconditionally data to them. This
gate asks what will **execute**, and a heredoc body executes whenever the command consuming
it is an interpreter:

```
bash <<'EOF'
rm -rf /
EOF
```

Caught today, because the gate scans everything. Under the fix this file proposed, the body
would have been stripped and this command would pass **unflagged** — a real deletion hidden
by a fix written for a false positive.

So the defect is not "the gate looks in the wrong place". The gate cannot know whether a
body is data without knowing the consuming command, and it is right not to guess.
## Evidence

The only occurrence of `rm` in the failing command was inside the quoted heredoc body,
which `sh` treats as literal data. The command's executable parts were `cat` and `wc`.

## Hypotheses tried

1. **Hypothesis:** the gate fired on something else in that command. **Test:** re-read the
   command; `cat`, `wc`, and a `$S` expansion are the only executable elements, and the
   reported reason names `rm` specifically. **Verdict:** rejected.

## Fix

**Fixed on `experiments` at `c3474f43`** — patch-id
`68b457eced239386eb1dd9b34bbf090cf047473d`.

Not the fix this section originally proposed. That one ("reuse whichever heredoc-body
exclusion the IL-3 gate gained") is rejected on the grounds in *Root cause* — it would have
silenced `bash <<'EOF' … rm -rf / … EOF`.

The section's other instruction held and is what pointed the way: *"Do not 'fix' this by
loosening the `rm` pattern; the false positive is about where the gate looks, not what it
matches."* Right about the diagnosis; the available move was neither loosening the pattern
nor moving where the gate looks, but **changing what it says**.

The flag stays and becomes discriminating. When a pattern matched only inside a heredoc
body, the reason now:

- says the match was in a body, never in executable position;
- names the case in which that is *not* safe — a body consumed by an interpreter;
- quotes the **opener line verbatim**, so the reader can see which case they are in.

That answers the harm this file actually named — that firing on prose *"trains the reader to
acknowledge without reading, which is the one habit the gate depends on not forming"* —
without weakening a single catch. The one round trip remains; what changes is that
acknowledging is now a judgement rather than a reflex.

**The match decision never consults the stripped text.** The two views are built separately
and the stripped one is read only *after* a match, to describe it. Structuring it the other
way — strip first, then decide — is the unsafe version wearing this fix's clothes.

The opener line is quoted rather than "the consuming command" named: naming it needs a
first-token parse that `env FOO=1 bash <<'EOF'` and `time bash <<'EOF'` both defeat, and a
confidently wrong command name is worse than none when the note's whole job is to let a
reader judge.
### Tests added

Three, in `src/util/path_security.rs`, RED first.

- `a_heredoc_only_match_is_flagged_and_the_reason_says_where_it_matched` — the behaviour.
- `an_interpreter_heredoc_is_still_flagged_because_its_body_executes` — **the security
  guard.** This is the command the originally-proposed fix would have silenced, so it is the
  test that stops it being re-proposed. It asserts the flag survives *and* that the note
  names the interpreter case, since a note that read as an all-clear here would be worse
  than none.
- `a_match_in_executable_position_carries_no_heredoc_note` — non-vacuity. Without it the
  first test is satisfied by appending the note to every reason unconditionally, which would
  restore the exact reflex the note exists to prevent while passing. Its second case is a
  real `rm -rf` sharing a command with an *unrelated* heredoc — the case a naive
  `command.contains("<<")` check gets wrong.

That middle test passed *before* the fix as well as after, and that is not a defect in it:
it asserts a property the fix must not break, so its job is to fail against the rejected
fix, not against the pre-fix code. Worth stating, because a test that was green throughout
otherwise looks vacuous.

Counts: lean 3409 → **3412**, default 4972 → **4975**. Both lanes, because
`path_security.rs` carries no feature gate.

Zero-failures rests on `cargo test`'s exit code 0 in both lanes rather than on a buffer
grep — the `@cmd_*` temp files had already been reaped when the grep ran, and it printed a
`No such file` to stderr rather than a count.
### Confirmed live 2026-08-31

Against the rebuilt binary — server `git_sha` `e25850d6`, pid 4024625 (up from 3786819),
so a fresh process. Re-running this file's own *Reproduction* returns:

```
reason: "rm with --force or --recursive — matched ONLY inside a heredoc body, never in
         executable position. A body is inert data unless the command consuming it is an
         interpreter (bash, sh, zsh, ssh, python …), in which case it runs and this flag
         is real. Opened by: cat > /tmp/…-ad2…. This gate cannot tell those two apart, so
         it flags both rather than stripping bodies the way the IL3 and source-file gates
         safely can. Read the opener, then acknowledge."
```

The flag is unchanged — still a `pending_ack`, still one round trip. What changed is that
the reason now answers the question the reader had, which is the whole fix.

The interpreter case is **deliberately not** exercised live. It would return a
`pending_ack` without executing, so a probe is safe in principle, but it is fully covered
by `an_interpreter_heredoc_is_still_flagged_because_its_body_executes` and there is no
reason to type a destructive command to watch a gate refuse it.

**One edge observed while verifying, recorded rather than filed.** The opener is truncated
at 80 characters, and this reproduction's opener was a long scratch path, so the trailing
`<<'EOF'` was cut. The note still worked, because the decision-relevant token is the
**leading command** — `cat` here — and truncation from the right always preserves it. Worth
knowing before someone "fixes" it by truncating the middle, which would trade a harmless
loss for the one part that matters.
## Workarounds

Acknowledge the `@ack_*` handle, having read the command. Or write the message with
`Write`/`create_file` instead of a heredoc.

## Resume

Read the dangerous-command matcher, confirm it lacks the heredoc carve-out the sibling
gates have, and reuse theirs.

## References

- `docs/issues/archive/2026-07-28-il3-gate-matches-pipes-inside-heredoc-text.md` — same defect, adjacent gate, fixed
- `docs/trackers/reconnaissance-patterns.md` — `R-144`, on gates and tests that assert on the wrong substrate
