---
kind: bug
status: fixed
tags:
- cluster/doc-contradicted-by-code
closed: 2026-09-09
opened: 2026-09-09
owner: marius
related: []
severity: medium
---

# BUG: a `SAFETY:` comment outlived both its `unsafe` block and its own rationale

## Summary

`Drop for LspClient` carried a five-line `// SAFETY:` block whose subject had been
gone for six months and whose central argument the codebase had explicitly rejected
three days earlier. It annotated a **safe** call, asserted an ownership fact that was
**never true**, and taught — verbatim, three lines above the call site — the exact cast
that `01b185d6` had just replaced with a refusal. Nothing in the toolchain could see
any of the three, because a `SAFETY:` comment is bound to its construct by
**adjacency**, and adjacency is not a binding: the construct can move out from under
it, be replaced by a safe call, or have its reasoning overturned, and the comment stays
where it is.

## Symptom (Effect)

No runtime symptom. The observable is the text itself, at `src/lsp/client.rs:1611` at
tree `f10eefe2`:

```rust
        // SAFETY: `pid` was captured from `child.id()` immediately after spawn and remains
        // valid for the lifetime of this `LspClient` (we hold the child handle). SIGTERM
        // (signal 15) is safe to send to a child process — it requests clean termination
        // without undefined behaviour. The `u32 as i32` cast is safe because Linux PIDs
        // are assigned from a range that fits in i32 (maximum 4,194,304 on 64-bit kernels).
        let _ = crate::platform::terminate_process(*pid);
```

Three independent defects in one comment:

1. **There is no `unsafe` beneath it.** `terminate_process` is a safe function.
2. **"we hold the child handle" was false the day it was written.** `LspClient` has
   never had a `Child` field.
3. **The cast paragraph teaches what the codebase now refuses.** There is no cast at
   this call site any more, and the reasoning it defends was classified as the bug by
   `01b185d6` three days before this file was written.

## Reproduction

At `f10eefe2`:

```
git show f10eefe2:src/lsp/client.rs | sed -n '1611,1616p'
```

The regression gate reproduces it mechanically — restore those five lines above the
`terminate_process` call in the working tree and run:

```
cargo test --test safety_comments
```

## Environment

Linux, Rust workspace `codescout v0.15.0`, branch `experiments`, tree `f10eefe2`.
Nothing environment-dependent; the defect is in committed text.

## Root cause

**A justification bound by adjacency, and three separate moves that broke the binding.**
Each is measured against the commit that made it.

**1 — false on arrival.** `3d462282` (2026-03-08, *"fix: enforce Rust coding standards
— safety, panics, dynamic dispatch"*) wrote the comment. Its own changelog bullet reads
*"Add `// SAFETY:` comment to `unsafe { libc::kill() }` in LspClient Drop impl"* — the
task was to produce a comment, and a comment was produced. The claim *"we hold the
child handle"* had no referent then and has none now.

  measured 2026-09-09: `git log -S'child: Child' -- src/lsp/client.rs` → **0 commits**;
  `-S'child: tokio::process::Child'` → **0**. Widening to `-S'Child'` returns **3**, and
  all three are something else: `299e1236` added `writer: Mutex<ChildStdin>`, `a4bf02f8`
  removed it, `05ee1b90` added a *comment* mentioning `Child`. The only "handle" the
  struct ever held was `ChildStdin` — a **pipe**, not a process — and holding a pipe
  does not reserve a pid.

  The widening is the load-bearing half of that measurement: two empty greps are also
  what a mistyped pattern returns, so the `-S'Child'` run is the control that makes the
  zeros a finding rather than a broken search.

**2 — the `unsafe` moved out and the comment did not follow.** `bedeb7c0` (2026-03-20,
*"refactor(lsp): use `platform::terminate_process` instead of `libc::kill` (C-8)"*)
replaced the `unsafe` block with a safe call. The `unsafe` and its `SAFETY:` argument
moved to `src/platform/unix.rs:131`; the comment stayed at the call site. From that day
the file contained a `SAFETY:` block annotating safe code — the shape this bug is named
for.

**3 — the rationale was overturned, by a fix that could not reach the text.**
`01b185d6` (2026-09-06, *"fix(platform): reject a pid that does not address a single
process"*, closing
`docs/issues/archive/2026-09-05-process-alive-reports-a-nonexistent-process-as-alive.md`)
judged the `u32 as i32` cast unsafe in exactly the direction the comment calls safe, and
replaced it with `addressable_pid` (`src/platform/unix.rs:124`), which **refuses rather
than casts** — its in-code note reads *"The cast would turn a single-process SIGTERM
into a group- or session-wide one."* Its file list is
`docs/issues/archive/2026-09-05-process-alive-reports-a-nonexistent-process-as-alive.md`,
`docs/trackers/issue-clusters/IC-6-addressing-without-an-escape-hatch.md`,
`src/platform/unix.rs`, `src/tools/rendezvous.rs` — **not** `src/lsp/client.rs`. The fix
was complete over the population it addressed. The caller-side comment was outside that
population and went on teaching the rejected argument.

**Why no instrument caught it.** The compiler does not read comments. Clippy's
`undocumented_unsafe_blocks` is **not enabled** here, and it runs the opposite
direction anyway: it finds `unsafe` without a comment, never a comment without
`unsafe`. And the party best placed to notice is the author of the refactor that moves
an `unsafe` elsewhere — for whom a comment sitting above the call site reads as
belonging to the call site. That is § *Observer Blindness*'s shape: care is the wrong
instrument, so the remedy is a check that runs when nobody is worried.

## Evidence

### The struct has no `Child`

`struct LspClient` (`src/lsp/client.rs:319`) fields: `writer`, `next_id`, `pending`,
`alive`, `reader_handle`, `workspace_root`, `capabilities`, `transport`,
`init_timeout`, `open_files`, `synced_sigs`, `stderr_lines`, `started_at`,
`init_completed_at`. `LspTransport::Process { child_pid: Option<u32> }` holds a **bare
integer**.

The `Child` is moved into the reader task, which awaits `child.wait()`
(`src/lsp/client.rs:581`). `Drop` **aborts that task first** and only then signals the
bare pid — so at the moment of the `kill`, nothing in the process holds the child open.

### What the pid argument should have said

The honest claim is narrower than the one the comment made, and worth writing down
because it is the reason this is a comment fix and not a code fix: aborting the sole
owner of the `Child` opens a pid-reuse window before the SIGTERM lands. Reaching a
collision needs the pid counter to wrap — roughly 4M intervening spawns — so the block
is **sound in practice and not by construction**. No live race is claimed here; that
distinction is the point of recording it.

## Hypotheses tried

1. **Hypothesis:** the comment is stale in the ordinary way — right when written, one
   later commit moved the code.
   **Test:** `git log -S` on each of its three claims separately.
   **Verdict:** rejected, and the rejection is the finding. Three *independent* causes
   with different dates and different authors, one of which predates any drift.
   **Evidence:** § Root cause 1–3.

2. **Hypothesis:** this is IC-2 `gate-keyed-on-unobservable-event` — a standard that
   checks for a comment's *presence* substituting for "the `unsafe` is justified".
   **Test:** read IC-2's claim; look for the gate.
   **Verdict:** rejected. IC-2's members are **executing gates**; here the substitution
   was made by a session applying a written standard by hand, and the mechanized
   check (`undocumented_unsafe_blocks`) is not enabled. Admitting this would widen the
   class past its own claim. The proxy observation survives as a note, not a tag.

3. **Hypothesis:** IC-11 `doc-contradicted-by-code` admits all three defects.
   **Test:** IC-11's admission discriminator — *"True when written … so it is drift
   rather than a wrong statement, which is what admits it here rather than excluding
   it."*
   **Verdict:** confirmed for defects 2 and 3, **rejected for defect 1**. The ownership
   claim was false on arrival and is therefore not drift. The file is tagged IC-11 on
   the drift majority, and defect 1 is recorded here as explicitly **outside** that
   class rather than folded in — a class quietly gaining a member its own
   discriminator excludes is how a class stops predicting anything.

## Fix

`src/lsp/client.rs:1602-1624` — the `SAFETY:` block is gone. The replacement says what
this site alone controls (the reader task holding the `Child` was aborted immediately
above) and states the soundness claim at its real strength, *in practice, not by
construction*. It owes no `SAFETY:` prefix because it justifies no `unsafe`:
`terminate_process` owns both the `unsafe` and the pid-addressability argument.

The `Drop` comment above it was corrected in the same pass — it called the SIGTERM a
*"safety net"*, which `strace` had already falsified on 2026-09-08: this `kill` lands
**first** and the `kill_on_drop(true)` SIGKILL arrives against a process already dead.

- **SHA** — `6ac00158` on `experiments`. Positional; dies when `experiments` is rebased.
- **patch-id** — `8f1541ef741edaced5768ed0e2a8c62a84392f53`
  (`git show 6ac00158 | git patch-id --stable`). A content hash of the diff; survives
  rebase and cherry-pick. Derived from a **complete** patch file rather than a pipe:
  a capped buffer yields a valid-looking wrong digest with no error.

## Tests added

`tests/safety_comments.rs` — `every_safety_comment_precedes_an_unsafe_construct`.

**The rule is structural, not a window.** The first draft asked whether `unsafe`
appeared within N lines; that is an *existence* assertion and existence assertions are
monotone under **widening** (§ *Testing Discipline*), so the guard's own tuning knob
would be a way to silence it. The shipped rule has nothing to widen: the next line that
is neither blank nor a line comment must contain `unsafe`.

Measured at `f10eefe2`: **16** `// SAFETY:` comments under `src/`, `crates/`, `tests/`
— **15** satisfy the rule, **1** does not, and the one is this bug. Zero false
positives.

**Observed RED, on the production path.** The five lines above were restored verbatim
into the working tree and the mutation confirmed present in the file before the run —
an unapplied mutation prints green exactly like an uncovered one:

```
test every_safety_comment_precedes_an_unsafe_construct ... FAILED
test the_safety_scan_is_not_vacuous ... ok
  src/lsp/client.rs:1622 -> next code line is 1627: let _ = crate::platform::terminate_process(*pid);
```

`the_safety_scan_is_not_vacuous` is the non-vacuity control, after
`tests/feature_lanes.rs` § `the_guard_is_not_vacuous` — renamed rather than copied, because
two identical test names in one lane's output cannot answer *"is this mine?"*, which is the
only question the gate ritual asks of them: the scan must find ≥10 comments, must reach
`src/platform/unix.rs` specifically, and every site must resolve a following code line —
so a broken root list, a walk that stops descending, or a match string that stops
matching all fail loudly instead of passing by finding nothing. It stayed **green**
through the mutation above, which is correct: it answers a different question, and a
control that also reds would be double-counting one signal.

**What this gate cannot see**, stated because the fix inherits the shape of the defect:
it checks that a `SAFETY:` comment *sits in front of* an `unsafe`, which is a proxy for
*justifies* it. A comment that is adjacent to an `unsafe` and wrong about it passes.
The gate catches orphaning — the mechanism that produced defects 2 and 3 — and is blind
to defect 1, the manufactured justification, which no scanner reaches.

That makes the remedy itself an `IC-2` shape one level up: the event the gate wants is
*"does this comment justify this `unsafe`"*, which is unobservable to a scanner, so
adjacency is substituted — and the substitution fails in that class's signature way, by
**passing**. Naming it here rather than leaving it to be inferred from a clean
three-of-three claim, which is what the file would otherwise read as. (Raised by sessionId
`c9ab2c8d-dd74-43f4-9940-25756379a312`, against my own summary of the gate's ceiling.)

## Workarounds

None needed; the text is corrected in tree.

## Resume

**Done, not owed.** `src/lsp/client.rs` cites this bug by path, so the archive move and
the citation update landed in one commit. Had they split, `audit_doc_refs` would have red
CI at `high`: ordinary backticks are not an escape, and this is a live citation rather
than a mention.

Open and deliberately not done here: `undocumented_unsafe_blocks` is not enabled, so
the opposite direction — an `unsafe` with no `SAFETY:` comment — stays unguarded.
Enabling it is a lint-policy change across the workspace and does not belong in a
bug fix as a drive-by.

## References

- `src/lsp/client.rs`, `src/platform/unix.rs`, `tests/safety_comments.rs`
- `docs/issues/archive/2026-09-05-process-alive-reports-a-nonexistent-process-as-alive.md`
  — the adjacent bug whose fix rejected the cast this comment defended
- `docs/issues/2026-09-08-drop-kills-child-process-passes-with-both-kill-paths-removed.md`
  — the same `Drop` impl, measured the day before; source of the SIGTERM-lands-first finding
- `docs/trackers/issue-clusters/IC-11-doc-contradicted-by-code.md`
- Commits: `3d462282` (wrote it), `bedeb7c0` (moved the `unsafe` out), `01b185d6`
  (overturned the rationale)
