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
unverified: 'the -- <path> non-scoping is measured via `cargo fmt --check -v` target enumeration, not by observing an actual cross-session rewrite; no damage occurred in the observed instance because no .rs file was dirty at that moment. RE-DERIVED INDEPENDENTLY 2026-09-10 at bf0a5241 by a second session that had not seen the first measurement: same 36 entry points both ways, and the re-run found the claim slightly UNDERSTATED (scoped = bare + 1 argument, not equal). Published as a denominator per CLAUDE.md''s instrument-the-doubt law, not absorbed as a catch.'
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

## The documented escape hatch has a narrowed form that LOOKS scoped and is not (2026-09-10)

This file's mitigation is `scripts/fmt-mine.sh` plus the documented escape hatch: when it refuses,
*"ask the named owner, or if you have decided it is safe, run `cargo fmt` yourself."* That hatch is
correct and it has a trap one step in.

Observed: an SDD implementer hit the refusal (`src/server.rs` classified `UNKNOWN` — subagent writes
are unattributable, tracked separately), did everything the hatch asks — reported the refusal text
verbatim, confirmed real formatting drift existed **in its own added code** via
`cargo fmt --check`, decided it was safe — and then ran:

```
cargo fmt -- src/server.rs
```

and reported it as *"scoped to that one file only."* **It is not scoped at all.** Measured the same
day with `cargo fmt --check -v -- src/server.rs`, which enumerates what it will actually hand to
rustfmt:

```
[custom-build] build.rs
[lib]          crates/codescout-embed/src/lib.rs
[test]         crates/codescout-embed/tests/ollama_probe_installs_its_own_crypto_provider.rs
[example]      examples/activate_leak_probe.rs
[bin]          src/bin/sync_project.rs
[lib]          src/lib.rs
[bin]          src/main.rs
[test]         tests/audit_doc_refs.rs
... every remaining tests/*.rs target
```

Every target in the workspace, and rustfmt follows `mod` declarations from each entry point — so
the blast radius of the "scoped" form is **strictly WIDER than the bare form, never equal to it**.
That correction is the sharper root cause and it was measured, not reasoned:
`cargo fmt --check -v` prints the actual `rustfmt` invocation as its last line, and the two forms
differ by exactly one argument.

```
scoped:  rustfmt --edition 2021 src/server.rs --check <36 workspace entry points>
bare:    rustfmt --edition 2021              --check <the same 36>
```

**Cargo APPENDS the pathspec to rustfmt's argv; it does not filter the target list.** So
`-- src/server.rs` adds a 37th file to a set that already spans `build.rs`,
`crates/codescout-embed/`, `examples/` and every `tests/*.rs`. The whole workspace is enumerated
*before* any `mod` is followed, which is why "rustfmt follows `mod`" — true, and the form this
section first carried — understates it: `mod`-following widens an already-total set rather than
being the reason the set is total. There is no argument position in which this form narrows
anything.

This repo's own `CLAUDE.md` already says `cargo fmt` *"takes no pathspec"*; what is new is that the
natural attempt to add one **succeeds silently**: exit 0, no warning, no mention of the other 30-odd
targets it just formatted.

**Why this is worse than the bare form and belongs in this file rather than a new one.** The bare
form is documented as dangerous and a careful reader hesitates. The `-- <path>` form is what that
same careful reader reaches for *because* they hesitated — it is the shape of narrowing the hatch,
it reads as compliance with the warning, and it produces a written claim ("one file only") that a
reviewer then has no reason to check. Same act, a slightly WIDER radius, plus a false assurance the bare form
never offered. That is this file's own claim about the guard, one layer out: the mitigation moved
the hazard rather than removing it.

**No damage this time, and the reason is luck rather than the scoping.** Verified: no `.rs` file was
dirty in the worktree at that moment, and the two Rust files a peer had in flight
(`tests/e2e/eval_common/proc.rs`, `tests/result_caps.rs`) were committed before that run. Also
note what would NOT have detected damage: the implementer's evidence was `git status`, and a
reformat of an already-dirty file does not change its dirty status — it silently alters the content.
So *"git status shows only unrelated files"* is not evidence of no cross-session write, and this
file's central blind spot is exactly that.

**What is actually owed.** Either `scripts/fmt-mine.sh`'s refusal text should name the correct
narrowed form (`rustfmt <file>` formats one file; `cargo fmt` cannot), or `CLAUDE.md`'s escape-hatch
sentence should say that the hatch has no narrowed form. The refusal text is the better site: it is
read at the moment the decision is made, by the party making it. That is a change to a served
surface, so it is recorded here as the finding rather than applied.
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

### The instruction and the script are separately versioned, and only one arrival order announces itself

Adopting this created a two-part change: `CLAUDE.md`'s step 1 (`02a86104`) and
`scripts/fmt-mine.sh` itself (`2caf55c5`). They are separate commits, so on a checkout with
worktrees they can arrive in either order, and the two orders fail very differently.

**Instruction without script** — you read the new step 1, run `./scripts/fmt-mine.sh`, and
get `no such file`. Loud, self-diagnosing, and impossible to proceed past by accident. This
is the one I warned peers about.

**Script without instruction** — the file is sitting in your tree and your `CLAUDE.md` still
says `cargo fmt`, so you go on running the whole-tree command with the scoped one already
available, and **nothing anywhere says so**. Reported by
`b0015a98-e290-46de-8ed1-3c94bc73a987`, who was in exactly that state: `2caf55c5` had reached
their worktree via a rebase while `02a86104` had not, verified as
`merge-base --is-ancestor 2caf55c5 HEAD` → yes, `grep -c fmt-mine CLAUDE.md` → 0.

It is the same asymmetry this corpus has now hit in five instruments: **a stale name fails to
resolve and a stale pid resolves silently.** The direction that keeps working is the dangerous
one. Worth stating because the natural warning to write — the one this file's author wrote —
covers the loud half, and covering one failure mode implies by omission that the rest are
handled (§ *Observer Blindness* position 3).

No mechanism is proposed here. The honest note is that a two-part change to a shared
instruction has a window in which half of it is live, the window is per-checkout, and the
half that arrives first determines whether anyone notices.

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

## Fix provenance

- **SHA:** `2caf55c5` (on `experiments`) — positional; does not survive a rebase of `experiments`.
- **patch-id:** `35f0ccc37590baafad49fcc4c8f5519877b23853` — content hash of the diff; survives rebase and cherry-pick.

`feat(scripts): fmt-mine.sh formats only this session's Rust and refuses the rest` — adds
`scripts/fmt-mine.sh` and `tests/fmt-mine.sh`. Status is `mitigated`, not `fixed`, and this
pointer does not change that: `unverified:` names the reason, and CLAUDE.md's gate has since
been changed to call the script, which is a separate commit and a separate decision.
