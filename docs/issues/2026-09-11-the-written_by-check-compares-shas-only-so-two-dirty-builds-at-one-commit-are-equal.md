---
id: bfdfeebd4e5ca130
kind: bug
status: open
title: 'BUG: the written_by check compares shas only, so two different dirty builds at one commit compare equal and the warning is suppressed'
tags:
- cluster/guard-narrower-than-its-name
---

## Summary

`src/tools/semantic/index.rs:767-777` reports "a different build wrote this sidecar" by comparing
shas only:

```rust
if let Some(w) = st.written_by.as_ref() {
    if w.git_sha != env!("CODESCOUT_GIT_SHA") {        // <- the whole predicate
        result["written_by"] = json!({
            "git_sha": w.git_sha,
            "git_dirty": w.git_dirty,                  // <- collected, display-only
            "pid": w.pid,
            "exe_deleted": w.exe_deleted,
            "reading_binary_sha": env!("CODESCOUT_GIT_SHA"),   // <- no dirty companion
        });
    }
}
```

**Two different builds from the same commit compare EQUAL**, so the report is suppressed exactly
when the two binaries differ by uncommitted content. On a shared checkout with several live
sessions that is the normal case, not the edge one.

The dirty bit is right there — `w.git_dirty` is read on the very next line — but it sits *inside*
the branch the sha comparison gates, so it can never widen the predicate that decides whether
anyone sees it.

## Symptom (Effect)

Asymmetric within one JSON object: the **writer's** record carries `git_dirty`, the **reader's**
self-identification (`reading_binary_sha`) does not. A caller comparing the two fields is given a
dirty-aware value on one side and a dirty-blind value on the other, with nothing marking the
difference.

## Reproduction

Live, this session, 2026-09-11:

```
./target/release/codescout version
  -> {"version":"0.15.0","git_sha":"78e89c80",
      "git_sha_full":"78e89c80ef7173aa6b5fa68911220c3d10193d74","git_dirty":true}
```

The running binary reports `git_dirty: true` at `78e89c80`. `git log` shows **zero** code commits
since that sha, and three peer-owned source files were dirty at build time. So a second session
running `cargo rb` minutes later — same HEAD, different uncommitted content — produces a binary
whose `env!("CODESCOUT_GIT_SHA")` is byte-identical to this one's. Every `written_by` comparison
between the two reports nothing.

`codescout version` is the control that makes this concrete rather than argued: the binary already
knows it is dirty and says so on demand. The check simply does not ask.

## Environment

`experiments`, 2026-09-11, binary linked 20:50:14 from a dirty tree at `78e89c80`.

## Root cause

`build.rs` bakes all three values — `CODESCOUT_GIT_SHA`, `CODESCOUT_GIT_SHA_FULL`,
`CODESCOUT_GIT_DIRTY`. The sha reaches this site; the dirty flag does not, because `env!` is
called for the sha alone.


**The struct's doc comment is NOT wrong, and saying so precisely matters.**
`src/retrieval/index_state.rs:115-117` reads: *"a sidecar stamped with a `git_sha` different from
the reading binary's own `env!("CODESCOUT_GIT_SHA")` **proves a different build wrote it**, on
every platform, with no `/proc` walk."* That is true — it is the **converse** direction, and it
holds. The defect is entirely in the silence: nothing anywhere claims that equal shas prove the
same build, and nothing needs to, because a guard that only ever speaks on mismatch is read as
covering the property it is named for. Do not "fix" the comment.

**IDENTITY vs ANCESTRY — the distinction this file initially collapsed, and the reason it is
recorded here rather than quietly corrected.** From `git_dirty: true` I concluded *"the sha names a
commit the build is not"*, and then that any claim resting on the sha was unsupported. The first
clause is right and the inference is one step too strong. Uncommitted edits are an **additive**
delta over the committed tree, so:

- *"the build IS `78e89c80`"* — an IDENTITY claim. `git_dirty: true` refutes it outright.
- *"the build CONTAINS `4629a95b`"* — a LOWER-BOUND claim. `git merge-base --is-ancestor 4629a95b
  78e89c80` returns yes, so the committed tree holds the fix, and the sha supports the claim with
  one named residual: it fails only if the uncommitted delta REVERTED it.

That residual is representable and here it was not idle — `4629a95b` touched `src/server.rs`, which
was one of the files dirty at build time. So **the sha bounds the COMMITTED content and the dirty
flag marks an unbounded delta on top**; a lower-bound claim survives it and an identity claim does
not. Correction owed to `codescout-75` (sessionId b0b9bc40…), whose own error was the mirror of
mine: using identity-grade language for a lower-bound fact.
## Why this is a NEW instance, not the archived one

`docs/issues/archive/2026-08-16-usage-db-records-a-sha-that-need-not-describe-the-built-code.md`
(`0cd1fe818951b232`, fixed) is the same mechanism at a **different site** — `src/usage/mod.rs`'s
`write_record`, recording into `usage.db`. Its own § *References* names
*"`src/main.rs:367-374` — the only consumer of `CODESCOUT_GIT_DIRTY`"*, and that is still true:
the `version` subcommand remains the sole consumer, so the fix for that bug did not generalise the
flag's reach.

This is the repo's own *"mutate once per guarded SITE, not once per feature"* law holding in the
defect direction: closing the recorder said nothing about the comparator.

## Why this class

`cluster/guard-narrower-than-its-name`. The guard's name — and its only reason to exist — is *"a
different BUILD wrote this sidecar"*. Its implementation covers *"a different COMMIT wrote this
sidecar"*, a strict subset. The uncovered remainder (same commit, different uncommitted content) is
protected by nothing, and the guard's silence is what conceals it — which is IC-14's claim clause
for clause.

**Not `cluster/assertion-satisfiable-by-accident` (IC-9), and the near-miss is recorded because the
next reader will reach for it too.** IC-9 reads as the obvious home — a check that passes when it
shouldn't — but its Claim requires the haystack to embed **environment-controlled text** (a path, a
tempdir name, a hostname, a timestamp) so the pass is a *coincidence*. Nothing here is coincidental:
two shas are equal because the commit genuinely is the same, and the predicate is simply weaker than
the proposition it stands for. IC-9's own file records **two tags already withdrawn** for exactly
this substitution, matched from titles that read *"a test that passes when it shouldn't"* — *"true of
this class and true of a wider one"*. This file was tagged IC-9 on creation and retagged before it
landed; had it stayed, it would have been the third withdrawal in the file that documents the first
two.

**Not `cluster/assertion-that-cannot-fail` (IC-16) either**, which is where IC-9's two withdrawn
tags went: that claim requires *no input* to exist that would make the assertion fail. This one
fails correctly whenever the shas differ — it is narrow, not vacuous.
## Impact

Bounded and real. The check exists to warn that another build populated the store; it is blind in
precisely the configuration this repo runs (one checkout, several concurrent sessions, each
rebuilding). It cannot produce a wrong value, only a missing warning — which is why nothing
downstream fires.

## Fix

Not attempted — filed on notice. **And the first prescription written here was wrong; it is kept
below with its refutation, because it is the one a reader would otherwise re-derive.**

**What the check can and cannot conclude.** Three cases, and only the third is the defect:

| writer vs reader | conclusion | today |
|---|---|---|
| shas differ | **different build** — sound | reported ✓ |
| shas equal, both clean | same build | silent ✓ |
| shas equal, either dirty | **unknown** | silent ✗ |

The correct report for row 3 is *"cannot establish sameness"*, never *"different"* — two dirty
builds are unidentifiable rather than provably distinct, and a message claiming the latter is this
same defect inverted.

**REJECTED — my first prescription: compare `(sha, dirty)` and treat `dirty` on either side as
"cannot establish sameness".** Refuted by `codescout-75` (sessionId b0b9bc40…): a dirty build
reading **its own** sidecar lands in row 3, so that rule warns on every ordinary single-session run
with a dirty tree — which is the normal state of this checkout. Over-firing here is worse than
silence, because the warning's whole content is *"something unexpected wrote this"*. Note also that
naive pair equality does **not** work either, in the opposite direction: `(X, true) == (X, true)`,
so two genuinely different dirty builds at one commit still compare equal. The two obvious repairs
fail on opposite sides.

**`pid` / `exe_deleted` narrow it and do not close it.** They are already in `WriterProvenance` and
they do discriminate the common self-comparison — but `pid` is the identity of a PROCESS, not of a
BUILD: restarting the same binary changes it. So `pid` supports a positive *"definitely the same
build"* (same live pid) and yields no sound negative. It converts a guaranteed false positive into
an occasional one.

**What would actually close it is a content-derived BUILD identity** — something that varies with
uncommitted content, which `git_sha` by construction does not. A `build.rs`-baked build id is the
obvious shape and `build.rs` already computes the dirty bit in the same function. **Flagged, not
prescribed:** the archived `0cd1fe818951b232` records that the `build.rs` stamp *"can be stale"*
because its rerun triggers are declared rather than universal, so a build id minted there inherits
exactly the staleness it is meant to detect. That needs measuring before anyone builds it.

**The smaller half is worth doing regardless and is independent of all the above:** emit
`reading_binary_dirty` beside `reading_binary_sha` at `:774`. The writer's record carries
`git_dirty` and the reader's self-identification does not, in the same JSON object — a caller
comparing the two fields is handed a dirty-aware value on one side and a dirty-blind one on the
other, with nothing marking the difference. That asymmetry is a defect on its own terms whatever
happens to the predicate.
## Tests added

None. A regression test needs two builds at one sha with different content, so the cheap version
pins the *predicate's inputs* — that `dirty` participates — rather than the end-to-end scenario.

## Resume

Found during a post-rebuild reconnaissance, from a peer's self-correction rather than from the
code: `codescout-75` (sessionId b0b9bc40…) retracted half of a published sentence on the grounds
that a probe could not distinguish pre- from post-refactor, and kept the other half because it
rested on `reading_binary_sha`. Checking that second half is what surfaced this — the sha it rests
on omits the dirty bit, and the binary it names was dirty. Recorded in `bug-fix-session-log:F-135`.
