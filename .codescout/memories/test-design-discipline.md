# Test-Design Discipline (review lenses)

Craft-shaped lessons from entry-graph Stage 2 (2026-07-17), where every real defect was an
untested-seam DISCOVERY problem a green suite didn't reveal. Terse codescout-local echo;
full doctrine lives in the `testing-snow-leopard` buddy. Apply as standing review lenses.

## Assert on the cause, not error-presence (discriminating tests)

A test asserting only "an error occurred" (`err.downcast_ref::<X>().is_some()`, `is_err()`)
is NON-discriminating when >1 code path raises the same error type — deleting the code under
test can leave the test green. Stage-2 example: a worktree-cites guard test passed even with
the guard deleted (the error then came from cite-resolution failure). Assert on the specific
cause (message substring / error variant / field), and ask: "would this test still pass if
the code it targets were deleted or inverted?"

### Corollary: a composite fixture cannot pin the one character that matters

2026-08-08 (F-21's sibling, PR #10). `tee_path_is_safe` allowlists the characters permitted in
a tee target that is then interpolated **single-quoted** into a shell command. That quoting is
unescapable only while `'` is excluded — so `'`-exclusion is the whole invariant. The suite's
only `'`-bearing fixture was `"/tmp/x'; rm -rf /; echo '"`, which also carries `;` and a
space: admit `'` to the allowlist and all eleven assertions stay green, because that string is
still rejected on `;`. The PR under review was the one widening that allowlist.

Rule: when a test rejects a *class*, add one fixture per member whose individual exclusion is
load-bearing, isolated from every other reason to reject.

## One test per branch; both sides of every condition

Each new `if` / `match`-arm / `Some`-vs-`None` branch needs a test that REACHES that branch.
Stage-2: `resolve_cite_ref` shipped with 2 of 3 resolution branches untested; a read path
gated on slug-present had its slug-None side unexercised. Function-level "is it called by a
test" is NOT enough — a well-tested function can hide a dead branch (coverage tools mark the
function covered; only branch coverage or mutation testing sees the gap).

### Corollary: a minimal fixture never reaches a cap, truncation, or ordering branch

When a change adds a **cap, truncation, aggregation, or ordering**, the fixture must EXCEED
the cap. A one-element fixture proves only the empty and single-element cases, and unit
fixtures are built to be minimal — which is exactly what hides this class.

2026-08-07 (F-12, `docs/trackers/release-promotion-session-log.md`): `grep`'s new
`completeness_warning` shipped with seven tests, clippy clean, 3522 green, and CI 15/15 on
attempt 1 — and its output was useless. Every fixture created exactly ONE hidden entry, so
`if more > 0` was never executed. The real repo has 16, alphabetical ordering put `.github/`
twelfth, and the cap of 5 cut the single entry the feature existed to surface. The
`both sides of every condition` lens above catches it; it was applied to the `Option` two
lines up (three tests pin the `None` side) and not to the truncation two lines down. Having
the lens is not the same as sweeping every new branch with it.

Pairs with W-12: for any change to tool-facing OUTPUT (warnings, hints, summaries, rendered
text), call it once against the real repository and read the bytes. That is a step distinct
from the gate and from `cargo rb` + reconnect — the latter only establishes that the new code
is running, not that what it says is useful.

### Corollary: an invariant test proves the SHAPE, never the ASSIGNMENT

When a bug is "these numbers don't add up", the obvious test is the arithmetic — and it is
the one that cannot catch the bug's twin. An identity that holds for *any* assignment
verifies the shape and says nothing about whether each item is in the right place.

2026-08-07, measured rather than reasoned. `audit_doc_refs`'s three summary counters covered
7 of 10 `Verdict` variants, so 2,426 of 47,094 refs were counted in `n_refs_found` and in no
bucket. The fix routed every verdict through one exhaustive `match`, and got two tests:
`summary_counters_partition_every_verdict` (one finding per verdict; buckets must sum to
found) and `resolved_basename_counts_as_resolved_everywhere` (that verdict specifically must
land in `resolved`). Mutating `bucket()` to put `ResolvedBasename` in `broken` **killed the
second and left the first green** — because summing correctly is true under every possible
assignment. Writing only the obvious test would have let the mutation live.

So: for any partition, grouping, routing table, or dispatch map, write **two** tests — one
for the invariant, one for at least one specific member's placement, chosen as the member
whose misplacement would be most consequential. And prefer an **exhaustive `match` with no
wildcard arm** over any test: adding a variant then fails to compile until it is handled. A
test asserts a partition; an exhaustive match *is* one.

**It recurred within 48 hours, with the halves swapped.** 2026-08-08: `doctor`'s new
`abs_path_outside_managed_roots` cap built `by_check` above the truncation and read `"total"`
from `all_violations.len()` below it, so the two disagreed in the same `summary` object
whenever the cap fired — 311 vs 30 on the live catalog. That cap shipped WITH a member test
(`by_check == 25`), which stays green under the defect; the invariant twin was the missing one
this time. Fixed in `6f261da9` by taking both numbers above the truncation and adding
`summary_total_partitions_by_check`, which also asserts `total > shown` so the fixture cannot
silently stop exercising the truncation branch (the cap corollary above). Two occurrences in
one repo in two days: treat "a count and its breakdown in the same object" as a standing
review trigger.

### Corollary: a fixture spelled the way the code spells it cannot discriminate

2026-08-08 (F-21). `resolve_git_bash_never_selects_the_wsl_launcher` asserted that a
`bash.exe` under `%SystemRoot%\System32` — the WSL launcher — is never selected. It passed,
and the exclusion it tested never fired on any real host.

The production code built the excluded directory as `%SystemRoot%`.join("System32"); the test
injected `PATH=C:\Windows\System32`, the same spelling; `Path` equality folds case only on the
drive-letter prefix, so the byte-exact compare matched. Windows setup writes that PATH entry
as `C:\Windows\system32`, lowercase, where it does not.

The fixture was derived from the code instead of from the environment — precisely the input
that cannot discriminate. This generalises well past paths: any test whose input is produced
by the same expression as the code under test (a shared constant, a helper both sides call, a
value copied out of the implementation) verifies self-consistency, not behaviour.

Ask: **where does this fixture's value come from, and would the real caller produce it?** For
a normalizing comparison specifically, feed every spelling the environment actually emits, and
add the positive control — "never selects the wrong one" and "never selects one at all" are
the same assertion until something pins the accepting case.

### Corollary: a `#[cfg]`-gated test is not evidence on the platform that never compiles it

2026-08-08 (W-16). `src/platform/windows.rs` is `#[cfg(windows)]`, so its tests are not built
by `cargo test` on Linux at all: a type error surfaces as a red CI leg minutes out, and a
logic error surfaces only on the one leg of four that runs it. That is the substrate which let
F-21's fixture survive.

Two cheap responses, both applied there:

- Put the pure, platform-independent half of the logic in the facade module that compiles
  everywhere (`platform/mod.rs`) and test it there. `windows_dir_eq`'s case-folding is
  asserted on all four legs; only the resolver consuming it is asserted on one.
- Before pushing `#[cfg]`-gated test code, run `cargo check --target <target> --tests` — ~6 s
  warm, and `rust-toolchain.toml` already pins `x86_64-pc-windows-gnu`.

Clippy consequence worth knowing: a helper whose only caller sits inside `#[cfg(windows)]` is
dead code on Linux under `-D warnings`. `pub` rather than `pub(crate)` is the resolution the
module already uses for `posix_tokenize` and `shell_path_str`.

### Corollary: a test whose discriminating power comes from the environment passes everywhere you run it

2026-08-08 (F-32). The `cli_artifact` round-trip tests parse the CLI's `--json` stdout — the
right assertion, and the one that eventually caught a real defect. They could only ever fail
on a host with **no** Qdrant listening on 6334, because `qdrant-client`'s compatibility probe
`println!`s onto stdout only when it cannot reach a server. Every developer machine running
the stack passed them; CI, which runs no Qdrant, was the sole place they failed — and the
feature gate meant CI had never compiled them either.

This is a fourth way a test can be unable to fail, and the one that reading the test cannot
reveal: the assertion is correct in isolation. The other three are *deleted with its consumer*,
*asserts a subset under a name claiming all*, and *never compiled by any lane*.

Ask: **what in the environment, rather than in the code, decides whether this test can fail?**
A service that happens to be up, a file that happens to exist, a clock, a locale, a network
that resolves. Then pin that thing in the fixture — here, one line pinning
`CODESCOUT_QDRANT_URL` to an unreachable port, which is also what made the fix
mutation-verifiable. Pin toward the *failing* condition, not away from it: forcing
`CODESCOUT_ARTIFACT_BACKEND=sqlite-vec` would have made the tests equally hermetic and
permanently incapable of catching the bug.

2026-08-13 adds a blunter instance of the same family, worth knowing **before** touching
retrieval. `src/retrieval/qdrant.rs:422` marks the **only** real-Qdrant test `#[ignore]`, so
CI never runs it; sqlite's `real_vec0_*` tests (`:421,:482,:565`) are neither ignored nor
feature-gated and do run, because sqlite-vec needs no daemon. So the backend most
contributors actually run has no automatic coverage, and the one they mostly don't is
verified — an asymmetry that is easy to invert in your head and get backwards.

The consequence is a design constraint, not just a testing note: put every decision that
can be wrong in a **pure function** that compiles and runs everywhere, and leave each
backend holding only a mechanical, mirrored translation. `#[ignore]` is a third way a test
cannot fail where you are, beside `#[cfg]`-never-compiled and
environment-supplies-the-condition — and unlike those two it is invisible in a green
`cargo test` summary except as a number nobody reads.

### Corollary: a first measurement is a warm-up artifact until a second one agrees

Not a test-design rule strictly, but it fails the same way — a number that looks legitimate
because it has units and is internally consistent. Three instances on 2026-08-07 (W-14):
sparse embed read 146.8 ms cold and **16.8 ms** warm (8.7×, and it fed a shipping decision);
the `audit_doc_refs` tally migrated by up to 69 refs cold and was byte-identical warm; and
`resolve_file_symbol` returned `SymbolMissing` for symbols that exist when the server answered
before finishing indexing — the same trap promoted from a latency error into a false claim
about the code. In a benchmark, discard the first iteration and **say in the write-up that you
did**; a mean over a run that started cold folds a one-off into every per-item number.

## Round-trip completeness (writer shape ↔ reader surfacing)

For any writer/reader pair, every distinct shape the WRITER can emit must be reachable and
correctly surfaced by the READER — test the writer's whole shape-space, not just its
happy-path output. Stage-2: the writer produced id-keyed `dst_ref` for non-tracker targets,
but the reader gated the whole block on slug-present, so id-keyed backlinks were invisible;
both tests shared the incidental precondition "target has a slug," masking it. Watch for
shared incidental preconditions between writer and reader tests.

## The mechanical backstop

These three are semantic (a green suite hides them), so the durable catch is
`cargo mutants --in-diff <range>` scoped to the diff at the pre-ship boundary — the only
mechanism that flags a test reaching code without discriminating it. See `docs/RELEASE.md`
Standard Ship Sequence.

### Corollary: a surviving mutant is a hypothesis, not a finding — prove it with a throwaway probe

`cargo mutants` (or a hand-applied mutation) tells you a mutant **survived**. It does not tell
you *why*, and the two causes need opposite responses:

- **a real coverage gap** — the behaviour differs and nothing observes it, or
- **an equivalent mutant** — the behaviour does not differ at all, so no test can ever catch it.

Reporting the second as the first sends someone hunting for a test that cannot exist. Dismissing
the first as the second ships the gap. Reading the code harder does not separate them, because
the whole reason the mutant survived is that the difference is not where anyone was looking.

**The probe:** write a throwaway test that would only pass if the behavioural difference is real.
Run it against HEAD — it must **pass**. Apply the mutation — it must **fail**. Then delete the
probe and report either the demonstrated gap (handing over the probe as the fix) or the
equivalent mutant with the argument for why no observable difference exists. If you cannot
construct a probe that separates the two states, that is itself the answer.

Measured 2026-08-19, guide-ledger Phase C Task 3. Nine mutations, eight killed. The survivor
swapped the re-arm predicate's comparand from `AgentInner::default_workspace_root` to
`Agent::project_root()` — the F-52 trap, which the task brief had named in advance as "the
easiest way to get this task wrong". The full suite stayed green (33/33, 35/35, 59/59), which
proves only that nothing looked. The reviewer then wrote a probe that focus-switched to a
sub-project and re-activated the repo root by full path: **passed on HEAD, failed under the
mutation**, while every shipped test stayed green throughout. That converted "a mutation
survived" into "here is the behaviour nothing observes, and here is the test that observes it" —
and it arrived with the fix already written and verified in both directions.

Two things the same episode showed about *which* mutants deserve a probe:

- **The dangerous survivors are the self-inflicting ones.** `ctx.agent.project_root()` is the
  more obvious-looking call, so a future refactor reaches for it naturally; the failure is a
  spurious re-arm on every root re-activation while a sub-project is focused — silent, and
  exactly the waste the change existed to remove. A survivor whose mutation is *more* idiomatic
  than the shipped code is worth a probe before anything else.
- **Verify the mutation is on disk before believing the result.** Earlier in the same phase an
  edit silently failed to land and the suite reported GREEN — indistinguishable from a real
  coverage gap, and it points at writing a test for a gap that does not exist. `grep` a marker
  inside the mutated region before running. A mutation you did not verify landed is not a
  mutation you ran.

Related: `docs/trackers/bug-fix-session-log.md` W-48 — the same phase's finding that a review
strong enough to leave zero surviving mutants in the *code* still under-counted a
documentation-defect class by 55%, because mutation testing has no mutant for "this comment's
stated reason is now false."
