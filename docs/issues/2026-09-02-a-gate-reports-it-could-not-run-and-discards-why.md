---
kind: bug
status: open
tags:
- cluster/hint-composed-without-the-request
- testing
- cargo
- diagnostics
closed: null
opened: 2026-09-02
owner: marius
related: []
severity: low
---

# BUG: `packaged_includes`' gate reports that cargo could not answer, and throws away cargo's answer about why

## Summary

`every_escaping_include_str_survives_cargo_package` correctly refuses to read a
failed `cargo package --list` as a pass. But `packaged_files()` discards cargo's
stderr at the point of failure, so the gate's message names the package and not
the cause — and the reader has no route back to it, because the failing
invocation is inside the test.

## Symptom (Effect)

```
thread 'every_escaping_include_str_survives_cargo_package' panicked at tests/packaged_includes.rs:207:5:
`cargo package --list` failed for: codescout. The gate could not be evaluated, which is not the same as passing — re-run once cargo can list these packages.
```

Exit 101 on `cargo test --workspace --no-default-features`. Observed 2026-09-02
on `experiments` in a checkout shared by seven sessions.

## Reproduction

**Not reproducible on demand — and that is the point of the report.** The
failure was transient: it appeared twice, then stopped, and by the time I looked
the exact invocation the test makes —

```
cargo package --list --allow-dirty --offline -p codescout
```

— exited 0 three times in a row, emitting 510 lines. Because the gate kept no
stderr, **there is nothing to go back to**: the one artifact that would have
named the cause was produced and dropped.

## Root cause

`tests/packaged_includes.rs:164-181`. `packaged_files()` returns
`Option<BTreeSet<String>>` and collapses every failure mode into `None`:

```rust
let out = Command::new(cargo)
    .args(["package", "--list", "--allow-dirty", "--offline", "-p", pkg])
    .current_dir(repo_root())
    .output()
    .ok()?;                 // spawn failure -> None
if !out.status.success() {
    return None;            // cargo's stderr is in `out` and is dropped here
}
```

`out.stderr` is populated and never read. Read from the code 2026-09-02, and
confirmed against the observed message, which carries no cargo text.

## Hypotheses tried

1. **Hypothesis:** a stray file with a literal newline in its name
   (`head.\naab0c4ef'"s`, my own shell-quoting accident, present in the tree at
   the time) made cargo's file enumeration fail.
   **Test:** isolated repro — `cargo new --lib`, `git init`, commit, then
   `touch -- $'head.\nfoo'` and run `cargo package --list --allow-dirty
   --offline`.
   **Verdict: rejected.** Exit 0 both with and without the file.
   I had already stated this cause aloud before testing it; it was wrong.

2. **Hypothesis:** the dirty working tree (7 modified files across four
   sessions).
   **Verdict: rejected** — the invocation passes `--allow-dirty`. A bare
   `cargo package --list` *does* refuse on a dirty tree, which is what made this
   hypothesis look confirmed; it is a different command.

3. **Hypothesis:** a concurrent peer `cargo` process holding the build-directory
   lock. `Blocking waiting for file lock on build directory` was observed from
   this session minutes earlier, and six other sessions build in this tree.
   **Verdict: deferred** — plausible, unverified, and **not distinguishable from
   any other cause with the evidence the gate keeps**, which is the finding.

## Fix

Plan: keep the `Option` contract, add the cause. Return
`Result<BTreeSet<String>, String>` — or push `(pkg, stderr_tail)` onto
`unanswered` — so the assertion message can carry cargo's own words. The gate's
honesty about not-a-pass is already right and should not change; only the
diagnostic is missing.

Not fixed here: this was noticed while finishing an unrelated task, and the
change touches a gate other sessions depend on.

## Tests added

None yet. A regression test would assert that a forced failure's message
contains cargo's stderr — cheap to write once the signature changes.

## Workarounds

Run the invocation by hand at the repo root and read the error directly:
`cargo package --list --allow-dirty --offline -p <pkg>`.

## Resume

Change `packaged_files` (`tests/packaged_includes.rs:164`) to carry the error:
either `Result<_, String>` capturing `String::from_utf8_lossy(&out.stderr)`, or
keep `Option` and take a `&mut Vec<(String, String)>` for the causes. Then widen
the `unanswered` assertion message at `tests/packaged_includes.rs:207` to print
them. Confirm by forcing a failure (e.g. a nonexistent `-p`).

## References

- `tests/packaged_includes.rs:163-215`
- `docs/adrs/2026-08-27-negative-results-name-their-scope.md` — the same
  principle one layer up: a negative result must name the scope it examined.

**Cluster: `IC-22`, and the evidence is this session's own wrong conclusion.**
The message's prescription — *"re-run once cargo can list these packages"* — is
composed from the response's **shape** (a failure occurred) rather than from the
**cause**, because the discarded stderr is what held the cause. I followed it.
It went green. I then reported the stray newline-named file as the cause, and an
isolated repro refuted that. That is IC-22's claim exactly: a plausible route
that *reads as progress rather than as an error*, leaving the caller no closer —
here, worse than no closer, because the green re-run manufactured a false cause.
The discarded `stderr` is the mechanism; the uninformed prescription is the harm.

**`IC-21` was considered and rejected**, though the remedy is superficially the
same move (*add the field the reader needs to the surface that already holds
it*). Its claim requires that **nothing errors** and that the expense *"stays
invisible until it is already large"*. This errors, loudly, at exit 101 — and
both its members turn on magnitude in bytes. Recorded so a later audit does not
re-litigate it from scratch.
