---
id: 3a9eb5153e5311a8
kind: bug
status: fixed
title: 'BUG: the unpushed-ledger guard compares an un-canonicalized path against a canonicalized workdir, so it silently allows on macOS and Windows'
tags:
- cluster/guard-narrower-than-its-name
closed: 2026-09-06
opened: 2026-09-06
owner: marius
related: []
severity: medium
---

# BUG: the unpushed-ledger guard goes silent on a symlinked path

## Summary

`ledger_has_unpushed_commits` derives the repo-relative path with
`abs_path.strip_prefix(workdir)`. `repo.workdir()` is **canonicalized** by
libgit2; `abs_path` is whatever the caller held. When they differ in form the
strip returns `Err` — and because **every failure path in that helper allows by
design**, the guard reports *"no unpushed commits"* for a ledger that has them.

It fails in the **safe** direction, which is why it survived four days: a guard
that wrongly allows produces no error, no refusal, and no artifact.

## Symptom (Effect)

CI, since 2026-09-02, on `Test (macos-latest / default)` and
`Test (windows-latest / default)` — and **green on `ubuntu-latest / default`**:

```
librarian::tools::append_entry::tests::allocation_is_refused_while_the_ledger_has_unpushed_commits
  panicked: called `Result::unwrap_err()` on an `Ok` value: Object {"id": String("R-2"), …}

librarian::tools::append_entry::tests::unpushed_is_per_file_not_per_branch
  panicked: an unpushed commit on THIS ledger must be reported
```

The first is the shape worth reading: the test expected a refusal and got a
**successful allocation**. The guard did not error — it answered *no*.

## Reproduction

On Linux, with no Mac and no Windows box, by reaching the ledger through a
symlink:

```
cargo test --workspace --lib a_ledger_reached_through_a_symlink
```

Pre-fix, that reds on the symlink assertion while its control row passes.

## Environment

- Reachable wherever `abs_path` and libgit2's `workdir` differ in form.
  Measured: macOS (`tempfile::tempdir()` returns `/var/…`, a symlink to
  `/private/var/…`) and Windows (short-name vs long-name temp paths).
- **Not** reachable on Linux CI or on this dev machine: `/tmp` is not a symlink,
  so every local gate run has been green on this code since it shipped.

## Root cause

`src/librarian/tools/append_entry.rs`, pre-fix:

```rust
let Ok(workdir) = repo.workdir().ok_or(()) else { return false };
let Ok(rel) = abs_path.strip_prefix(workdir) else { return false };
```

Two facts compose:

1. **The two paths come from different sources.** libgit2 resolves `workdir`;
   `abs_path` arrives from the catalog row untouched.
2. **`return false` means "allow".** That is deliberate and documented on the
   function — *"EVERY FAILURE PATH ALLOWS… a repo with no remote has no second
   host, so refusing there is a false positive with no recoverable reading."*
   The contract is right.

The composition is what fails. A **path-comparison bug** enters through the same
door as the legitimate cases and becomes indistinguishable from them: "no
upstream configured" and "I could not compute the relative path" produce the
identical observable. An escape hatch designed for known-benign conditions
swallowed an unknown one.

*Measured 2026-09-06:* the born-red symlink test above; CI runs `34044613956`,
`34041942799`, `34029223220` all red on the same two tests, all green on
`ubuntu-latest`.

## Hypotheses tried

1. **Hypothesis:** CI has no configured upstream, so the guard correctly allows.
   **Test:** compare the matrix — `ubuntu-latest / default` passes with the same
   fixture and the same runner image family.
   **Verdict:** **rejected.** The fixture builds its own bare origin and tracking
   branch; if that were the cause, ubuntu would fail too.

2. **Hypothesis (first attempt at the repro):** my symlink fixture proves it.
   **Verdict:** **rejected on the first run** — the **control row** failed, not
   the symlink row. The fixture omitted a `std::fs::write` before `commit_path`,
   so the commit landed with no delta on the ledger path and the guard correctly
   reported nothing. Without that control I would have read a broken fixture as
   confirmation of the hypothesis it was built to test.

## Fix

Canonicalize **both** sides before comparing, falling back to the raw path when
resolution fails so the allow-on-failure contract is preserved:

```rust
let abs_c  = abs_path.canonicalize().unwrap_or_else(|_| abs_path.to_path_buf());
let work_c = workdir.canonicalize().unwrap_or_else(|_| workdir.to_path_buf());
let Ok(rel) = abs_c.strip_prefix(&work_c) else { return false };
```

On Windows this also aligns the `\\?\` extended-length forms, which is why one
change fixes both platforms.

SHA: `bae74d5a` (**`experiments`**)
patch-id: `1fac483176898b81e9d6cd4d7bf2cd5956848ea7`

**Verified on the platforms that actually failed**, not only by the Linux
reproduction — CI run `34048001721` (tree `4489d1b0`, which contains this fix):
`Test (macos-latest / default)` **success**, `Test (windows-latest / default)`
**success**, 17 of 18 jobs green. Both had been red since 2026-09-02.

The distinction is why this file was held unarchived for an hour after the fix
landed: *a Linux reproduction of the mechanism is evidence about the mechanism,
not about macOS* (`codescout-98`, sessionId
`8dba66b0-af4b-4cda-a333-54a0605b318e`). Gate-green plus a regression test is the
normal archive bar; it was not sufficient here, because the regression test runs
on the one platform where the bug was invisible.

Getting that verdict took a coordinated pause. Four sessions were pushing to one
branch, `cancel-in-progress` superseded every run, and of the preceding 12 runs
**9 were cancelled, 2 failed, 0 succeeded** — so no session could cite a CI
result at all. Three peers held pushes *and commits* while one matrix completed.

## Tests added

`a_ledger_reached_through_a_symlink_still_reports_its_unpushed_commits`
(`src/librarian/tools/append_entry.rs`), `#[cfg(unix)]`.

It reproduces macOS's mechanism **on Linux** by symlinking the work dir — so the
regression is guarded on the platform everyone develops on, not only on the two
where it was observed.

**Two rows, and both are load-bearing.** The symlink assertion is the defect.
The **direct-path control** is what stops the test passing against an
implementation hard-wired to `true` — which is the mutation that "fixes" the
symlink case by deleting the guard. The control also earned its keep
immediately: it is what caught the bad fixture in § *Hypotheses tried*.

The `std::fs::write` before `commit_path` is annotated on the fixture line,
because removing it makes the commit a no-op and reds the control for a reason
that has nothing to do with symlinks.

## Workarounds

None needed on Linux. On macOS or Windows, the guard simply does not fire — a
caller can still collide entry ids across hosts, which is what the guard exists
to convert into a visible failure.

## Resume

N/A — fixed here. If reopening: the same `strip_prefix`-against-libgit2-paths
shape may exist elsewhere; `repo.workdir()` has other callers that were **not**
audited as part of this fix.

## References

- `docs/issues/archive/2026-08-31-append-entry-high-water-mark-collides-across-hosts.md`
  — the bug this guard was built for.
- `src/librarian/adapter.rs` — already documents the mechanism in a test comment:
  *"a tempdir path is a symlink on macOS, and an id derived from the
  uncanonicalized form would simply never match."* Known in this codebase, in a
  different file, and not applied here.

### Cluster adjudication

`cluster/guard-narrower-than-its-name` (`IC-14`) on the remedy test: the fix
**widens** the guard's reach to paths it could not previously compare. The name
is literal — the function answers *"does this ledger have unpushed commits"* and
on two of three platforms always answered no.

**A second IC-14 instance filed by one session in one day**, after
`42769b490e11f106`. Stated because it is either a coincidence or a signal, and
the reader should get to decide: both were a **fallback arm swallowing a case it
was not designed for** — there, an unreachable store; here, an uncomparable
path. If a third arrives with that shape, the class may want splitting on
*whether the swallowed case is known-benign or unknown*.

`cluster/repro-env-diverges-from-gate-env` (`IC-5`) describes why it survived
four days — every local gate runs on Linux — but not what it is. Cited, not
tagged, for the same reason as `4a9ffbd8789df751`.
