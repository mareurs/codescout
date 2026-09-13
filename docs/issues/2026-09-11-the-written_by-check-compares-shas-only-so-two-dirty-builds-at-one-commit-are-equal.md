---
id: bfdfeebd4e5ca130
kind: bug
status: investigating
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

**⚠ `git_dirty` IS A POOR INPUT TO THIS PREDICATE, ON TWO COUNTS MEASURED 2026-09-12.** Row 3 above
treats it as "the code may differ from the commit". It does not mean that.

*Scope — demonstrated live.* `build.rs:44-45` computes the flag from a bare `git status --porcelain`
with **no pathspec**, so any dirty file anywhere sets it. Observed this morning: the running binary
reports `{"git_sha":"dffb89c2","git_dirty":true}` while `git status --porcelain -- src/ crates/
tests/ build.rs` is **empty** — the only dirt is markdown and an audit log. The compiled code IS
`dffb89c2`'s. So a doc edit puts an honest build into row 3 and any dirty-triggered warning is a
false positive, compounding the over-firing that killed the first prescription above.

*Freshness — named, not measured, and the mechanism is specific.* `build.rs:55-57` declares its
rerun triggers as `.git/HEAD`, `.git/index` and `.git/refs/heads/`. **None covers the working
tree**, so editing a source file does not by itself re-run `build.rs`; the stamped bit reflects
whenever it last ran. Git rewrites `.git/index` opportunistically when refreshing stat info, so in
practice the bit is *incidentally* fresh rather than reliably — which is worse than plainly stale,
because it is sometimes right. This is the concrete form of the staleness `0cd1fe818951b232` named
in the abstract. Someone should measure it before relying on the flag for anything load-bearing.

**Both counts strengthen the runtime-hash proposal below rather than weakening it**: a hash of the
running binary's own bytes has neither problem — no pathspec to scope wrongly, no trigger list to
go stale. It is also why `reading_binary_dirty` (the "smaller half" at the end of this section) is
worth emitting for *diagnosis* while being the wrong thing to *branch* on.

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
uncommitted content, which `git_sha` by construction does not.

**Proposed by `codescout-75` (sessionId b0b9bc40…), and it survives the objection that killed the
build.rs route: hash the ARTIFACT at runtime, not the source at build time.** Read the running
executable's own bytes, hash them, cache in a `OnceLock`. A value *derived from* the artifact has no
declared-input list to go stale and no rebuild to miss, so `0cd1fe818951b232`'s rerun-trigger
problem cannot reach it. Two builds from one dirty commit differ iff their bytes differ — exactly
the predicate `written_by` wants, true by construction rather than by a correctly-maintained trigger
list. It identifies the BINARY, not the source: two byte-identical builds from different dirty trees
hash the same, which is correct for the question this check asks and wrong for any provenance
question.

**MEASURED REFINEMENT — do NOT reach it through `std::env::current_exe()`, and this is the whole of
why.** On Linux `current_exe()` resolves via `/proc/self/exe`'s **readlink string**, which for an
unlinked binary comes back carrying a literal `" (deleted)"` suffix — a path that does not exist, so
the read fails. Measured 2026-09-11 on a copied `sleep` binary unlinked while running:

```
readlink /proc/<pid>/exe  ->  /tmp/…/sleepy (deleted)
read via the current_exe() path : FAIL
read via /proc/<pid>/exe        : OK      (sha256 9625c74169aa6585…)
```

**Independently reproduced** by `codescout-75` (sessionId b0b9bc40…) on a different probe binary:
`/proc/PID/exe` read 47,416 bytes, the readlink string read 0 and failed. Two runs, different
binaries, different methods — independent scopes rather than one instrument twice.

So the obvious spelling records `None` **exactly in the zombie-server case** — the case this whole
mechanism was built for (`2026-08-26-zombie-servers-on-deleted-binaries-stamp-stale-config-into-shared-state`,
and the reason `exe_deleted` exists at all). Reading the **inode** instead — `fs::read("/proc/self/exe")`
— succeeds on the deleted binary and yields a usable hash. Linux-only, which is where the zombie case
was measured; elsewhere `None`-and-stay-silent remains the honest fallback. The proposal's own cost
note said this failure mode "is one the struct is shaped for": right about the shape, backwards about
the consequence, and avoidable.

**Priced with the real number rather than an order of magnitude:** `target/release/codescout` is
**64,980,672 bytes** (62 MiB) — roughly 30-60 ms of BLAKE3 single-threaded, once per process, and it
must be lazy: computed on first sidecar write or comparison, never at startup.

**REJECTED — `fstat` on the opened `/proc/self/exe` fd, which avoids reading 62 MiB at all.**
`(dev, inode, size, mtime)` comes back for free and is the obvious way to dodge the hash cost.
Recorded with its refutation because it is precisely what the next reader reaches for. It fails in
**both** directions, which by now is this section's pattern for every non-content-derived repair:

- *Under-fires.* Inode numbers are **reused after deletion**, so a later build can inherit a dead
  build's identity — the zombie case again, one layer down, in the mechanism built to detect it.
- *Over-fires.* A relink that produces **byte-identical** output still gets a fresh `mtime`, so the
  tuple reports "different build" for the same build.
- *And it answers the wrong question.* A stat tuple identifies a **FILE**; the check asks about a
  **BUILD**. That substitution is exactly what this file's own `IC-9 → IC-14` retag was about, so
  taking the shortcut would re-commit the defect class being fixed, in the fix.

Paying ~40 ms to keep a **content-derived** identity is the right trade; the stat shortcut buys
speed by giving back the one property the mechanism exists for. (Alternative raised and refuted by
`codescout-75`, sessionId b0b9bc40…; the over-fire count is this file's.)

**Not attempted, and not prescribed as settled.** Two things want measuring first: whether two
consecutive `cargo rb` runs over identical source produce byte-identical binaries here (if not, the
hash reports "different build" on a harmless relink — arguably correct, certainly noisy), and the
non-Linux route. Recorded as a shape with a named failure surface, not a smaller one.

**The smaller half is worth doing regardless and is independent of all the above:** emit
`reading_binary_dirty` beside `reading_binary_sha` at `:774`. The writer's record carries
`git_dirty` and the reader's self-identification does not, in the same JSON object — a caller
comparing the two fields is handed a dirty-aware value on one side and a dirty-blind one on the
other, with nothing marking the difference. That asymmetry is a defect on its own terms whatever
happens to the predicate.

## Two constraints the Fix section above does not record (2026-09-13)

Found by reading the call site rather than the struct, while doing the "smaller half" below.

**1. The one repair immune to all three refutations is forbidden by the site's own documented
convention.** Every prescription above is a *predicate*, and each was refuted for over- or
under-firing. The obvious escape is to delete the predicate — report `written_by` unconditionally
and let the reader adjudicate — and there is a sibling site in this repo that does exactly that and
argues for it: `src/tools/config/mod.rs:448-456` reports the answering build's `(git_sha, git_dirty,
pid, exe_deleted)` on every `ProjectStatus` call, with the comment *"Unconditional, for the same
reason: a field that appears only when something is wrong cannot be used to confirm that things are
right."*

That argument applies here verbatim — and `index.rs` has ruled the other way, deliberately and in
writing. `src/tools/semantic/index.rs:750-752`: *"reported only when it was NOT this build —
presence-means-a-problem, same convention as `model_mismatch` and `last_sync_skipped` below and
above."* Three fields in one result object share that convention, and `format_index_status`
branches on `model_mismatch.is_object()`, so presence is load-bearing in the formatter too.

The two sites therefore follow **opposite, each-documented conventions**, and it is not drift: they
answer different questions (*which build is answering you* is always knowable; *who wrote this
sidecar* is a comparison). **This is why the bug is hard.** Not that nobody thought of reporting
unconditionally — that doing so puts `written_by` at odds with two neighbours in the same JSON
object. Whoever closes this either pays that cost knowingly or needs the content-derived build
identity, because there is no third predicate left. Do not find the sibling site and read it as the
fix; it is the fork.

**2. The branch has no test and cannot be given one at the call site.** It sits behind
`result["indexed"] == true` (`index.rs:731`), which requires a live Qdrant returning a non-zero
chunk count — so no local lane reaches it, and the default lane does not even compile
`server-stack`. Measured 2026-09-13: `reading_binary_sha` occurred **once in the entire tree**, at
its production site, with zero assertions anywhere on it. The mechanism this whole bug is about was
an alarm nothing could reach (§ *Testing Discipline*, *loudness is a property of a PATH*).

## Done 2026-09-13 — the "smaller half" only; the predicate is UNCHANGED

`written_by_report(w, reading_sha, reading_dirty) -> Option<Value>` extracted from
`IndexStatus::call`, same shape and same reason as `retrieval::sync::guard_stale_binary`. It adds
`reading_binary_dirty` beside `reading_binary_sha`, closing the asymmetry named at the end of
§ *Fix*. **The sha-only predicate is carried across untouched — this does not fix the bug**, which
stays open pending constraint 1 above and the two measurements § *Fix* asks for.

The reader's identity is a **parameter** rather than an `env!` read inside the function,
specifically so a fixture can express the *silent* branch: with the reading sha baked in, every
fixture sha differs from it by construction and the silence would be untestable.

**Still not measured, and it wants a decision rather than a drive-by.** Whether two consecutive
`cargo rb` runs over identical source produce byte-identical binaries here is the measurement the
runtime-hash proposal rests on — and taking it means **two release builds on a shared checkout,
each unlinking the binary four other live sessions in this tree are executing**. That is a
peer-affecting act, not a local probe; it needs the operator's say-so. The non-Linux route is open
too.
## Tests added

None. A regression test needs two builds at one sha with different content, so the cheap version
pins the *predicate's inputs* — that `dirty` participates — rather than the end-to-end scenario.

## Resume

Found during a post-rebuild reconnaissance, from a peer's self-correction rather than from the
code: `codescout-75` (sessionId b0b9bc40…) retracted half of a published sentence on the grounds
that a probe could not distinguish pre- from post-refactor, and kept the other half because it
rested on `reading_binary_sha`. Checking that second half is what surfaced this — the sha it rests
on omits the dirty bit, and the binary it names was dirty. Recorded in `bug-fix-session-log:F-135`.
