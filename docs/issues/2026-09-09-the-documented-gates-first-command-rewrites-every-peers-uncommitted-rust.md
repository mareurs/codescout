---
kind: bug
status: mitigated
tags:
- cluster/blast-radius-exceeds-visibility
closed: null
opened: 2026-09-09
owner: marius
related: []
severity: high
unverified: 'The guard exists and is CI-enforced, but CLAUDE.md''s gate still names bare `cargo fmt`, so a session following the documented path still reaches the unsafe command. Adoption is an operator call: the gate sentence is pinned byte-for-byte by a test and changing it changes the gate every session on this shared checkout runs.'
---

# BUG: the documented gate's first command rewrites every peer's uncommitted Rust

## Summary

`CLAUDE.md` § *Development Commands* opens the four-command gate with bare `cargo fmt`.
`cargo fmt` **writes**, unconditionally, to every `.rs` file in the workspace — including
files another session has open and uncommitted. Several sessions routinely share this
checkout. So the documented gate, **followed exactly**, reformats other sessions' work in
progress every time anyone runs it, and there is no check in the documented form to
defeat. Observed live on 2026-09-09: 7 hunks rewritten in a peer's untracked
`src/agent/build_check.rs`.

## Symptom (Effect)

`cargo fmt -- --check` exited 1 naming a peer's file; the gate's `cargo fmt` step then
rewrote it anyway:

```
Diff in /home/marius/work/claude/codescout/src/agent/build_check.rs:218:
Diff in .../src/agent/build_check.rs:414:
Diff in .../src/agent/build_check.rs:482:
Diff in .../src/agent/build_check.rs:515:
Diff in .../src/agent/build_check.rs:534:
Diff in .../src/agent/build_check.rs:577:
Diff in .../src/agent/build_check.rs:748:
FMT_CHECK exit=1
===== 1 FMT =====
FMT exit=0
```

The file was **untracked**, so `git checkout` could not have restored it. The only copy of
the pre-write bytes was the `--check` diff the same command happened to print.

## Reproduction

Have any session leave unformatted uncommitted Rust in the tree. Run the gate's first
command as documented: `cargo fmt`. Their file is rewritten. There is no flag, ordering or
exit code involved — this is what the command does.

## Environment

Shared checkout `/home/marius/work/claude/codescout`, `experiments`. Confirmed by two
sessions independently: `ad379a7c-a0cf-4c61-bcdb-f0696fea8c30` (caused it) and
`5399543d-22d6-4ed9-9ebb-876be459989f` (owned the file, and separately reached the same
conclusion about the documented form).

## Root cause

**`cargo fmt` has no scope narrower than the workspace.** It takes no pathspec, no
"only my files" mode, and no `--staged`. Its blast radius is every `.rs` in the tree,
which on this checkout is strictly wider than the set of files the running session
authored — the defining shape of `cluster/blast-radius-exceeds-visibility`.

**The protection that exists is documented for the other shared resource.** `CLAUDE.md`
already reasons carefully about a shared resource being corrupted by a correctly-followed
gate — that is the whole argument for ending on the default lane, and for chaining the two
test lanes with `;` rather than `&&`, so that "following the gate cannot arm the trap for
anyone else." That argument is about the shared `target/`. **The identical hazard on the
shared *worktree* is not covered**, and the `;`-not-`&&` rule actively points away from it:
written as one line, the natural move is `;` throughout, and at the `fmt` step that is
exactly wrong, because `--check` exists precisely to be a conditional.

**Why nobody sees it.** The author of the write is formatting *their own* change and gets
a clean result. The victim gets a diff they did not make in a file that may be untracked,
usually discovers it much later, and cannot attribute it — `cargo fmt` leaves no record of
who ran it. The party who could notice is not the party who acts.

## Evidence

### The damage is real but cheap; the recoverability is what is not

rustfmt is semantics-preserving, and in this instance the file's owner verified after the
write that `cargo fmt -- --check` exits 0 and all 26 of their tests pass, and declined a
reverse diff. So the *cost* here was an interruption, not lost work. That is a property of
this instance, not of the class: the same write against an untracked file mid-edit, with
an editor buffer open, is a last-writer-wins race with no git copy to fall back on.

### Two ways to reach it, and only one is a user error

The session that caused it ran `cargo fmt -- --check` **first**, precisely because
`CLAUDE.md` warns that peers hold uncommitted Rust — then defeated its own guard by
chaining the whole gate with `;`. That is a procedure bug. But the documented gate does
not contain the `--check` at all, so a session following the instructions literally
reaches the same write with nothing to defeat.

## Hypotheses tried

1. **Hypothesis:** this is a separator bug in one session's command line.
   **Test:** read the documented gate's first command.
   **Verdict:** rejected as the whole story. `CLAUDE.md`'s step 1 is bare `cargo fmt`.
   The separator is a *second* route to the same write, not the cause.

## Fix

**Option 2 BUILT, 2026-09-09: `scripts/fmt-mine.sh`, guarded by `tests/fmt-mine.sh`
(29 assertions) in its own CI job `fmt-mine-tests`.** Status is `mitigated` and not
`fixed` on purpose — see § Resume: the tool exists and the documented gate still names
the unsafe command, so anyone following `CLAUDE.md` literally still reaches bare
`cargo fmt`.

What it does: stage 1 is `cargo fmt -- --check`, which the gate pays for anyway, and on a
tree needing no reformatting it exits there having spent nothing extra. Only when something
WOULD be rewritten does it pay for stage 2, the transcript scan. It then formats only the
files `scripts/file-provenance.py` attributes to this session and refuses the rest, naming
the owner's sessionId, their `[LIVE]` marker and the `uds:` socket to reach them.

`SHARED` and `UNKNOWN` are refused alongside `PEER`. `UNKNOWN` in particular is never read
as "safe to format": absence is a statement about coverage, and reading it as "nobody owns
this" is the same substitution the provenance tool's own docstring forbids in the opposite
direction. There is deliberately **no `--force`** — a flag that reformats a peer's file on
your say-so re-admits the defect under a spelling that reads as deliberate, and the honest
escape already exists: run `cargo fmt` yourself, which is the same act minus the false
assurance that a guard sanctioned it.

**It shipped one defect, and how it was caught is the transferable part.** The first
partition used `sed -n 's|^\(SHARED\|PEER\|UNKNOWN\)...|'` — where `|` was both the
`s///` delimiter and the intended alternation. sed reads an escaped delimiter as a
literal, so the pattern matched the string `SHARED|PEER|UNKNOWN` and never fired.
`NOT_MINE` was always empty and every refusal fell through to a branch reporting *"nothing
attributable to this session needs formatting"* for a file a live peer owned.

**It still refused, and still wrote nothing.** Exit code correct, bytes correct. A suite
asserting on outcomes alone is green against it — verified by mutation: restoring that
`sed` reds **8** assertions, every one of them about the MESSAGE (`names the owning sid`,
`names the socket`, `names the LIVE marker`, `says why there is no --force`), while
`PEER exits 1` and `PEER left the file unwritten` stay green. That is § *Testing
Discipline*'s remedy-text law with a measurement attached, met inside the fix for a bug
about the same law.

Four mutations, four kills: the `sed` defect (8), dropping `SHARED` from the not-mine set
(1), gutting the no-`--force` rationale (1), removing the clean-tree fast path (2).

The suite's own non-vacuity control was vacuous first: the stub announced itself on stderr,
which the script captures into `$PROV` and parses, so the marker never reached the output
being asserted on. A control that asserts on a channel the subject CONSUMES observes
nothing. Replaced with a marker file the script cannot swallow.

### The two options not taken

**Not applied — these need a ruling, not a patch.** The repair touches `CLAUDE.md`
§ *Development Commands*, whose gate sentence is pinned byte-for-byte by
`claude_md_gate_lists_its_four_commands_in_the_load_bearing_order` (`src/prompts/mod.rs`),
and changing a project instruction because a session decided to is not this session's
call. Options, cheapest first:

1. **Document the guarded form:** `cargo fmt -- --check && cargo fmt`. Exit 0 makes the
   write a proven no-op; a non-zero exit stops before touching anything. It is TOCTOU —
   a peer can dirty the tree between the two — but it closes the case that actually
   happened. Requires moving the pinning test with the sentence.
2. **Make the safe path the only path**, per § *Observer Blindness* position 3: a
   `scripts/fmt-mine.sh` that formats only paths this session authored
   (`scripts/file-provenance.py` already answers that), so compliance cannot reach a
   peer's file.
3. **Do nothing and say so at the refusal site**, which the corpus prefers to a silent
   limitation — but there is no refusal site here, because there is no refusal.

## Tests added

None. The defect is in a documented shell command, not in a code path; a test that
asserts `CLAUDE.md`'s first command is not bare `cargo fmt` is possible and belongs with
whichever option above is chosen, not before.

## Workarounds

Run `cargo fmt -- --check` and only run the writing form if it exits 0 — with `&&`, not
`;`. If it exits non-zero, read which files it names before writing: they may not be
yours.

## Resume

**One step left, and it is not a session's to take.** `scripts/fmt-mine.sh` exists, is
tested and is CI-enforced, but `CLAUDE.md` § *Development Commands* still opens the gate
with bare `cargo fmt` — so the documented path still reaches the unsafe command and this
stays `mitigated`. Making it gate step 1 edits a project instruction whose sentence is
pinned byte-for-byte by `claude_md_gate_lists_its_four_commands_in_the_load_bearing_order`,
and it changes the gate every other session on this checkout runs. Six sessions were live
here when this was written. That is an operator call, not a session's.

If it is taken: move the pinning test with the sentence rather than deleting it, per that
section's own instruction, and announce it — a session mid-gate whose step 1 changed under
them reads it as a broken tool.

Original ruling text, superseded by the above:
Take a ruling on the three options in § Fix. If option 1 or 2, move
`claude_md_gate_lists_its_four_commands_in_the_load_bearing_order` with the sentence
rather than deleting it, as § *Development Commands* instructs.

## References

- `CLAUDE.md` § *Development Commands*, `docs/conventions/gate-ordering.md`
- `docs/conventions/shared-checkout-commit-sequence.md` — the same shared-tree discipline
  for the index, which has guards where this has none
