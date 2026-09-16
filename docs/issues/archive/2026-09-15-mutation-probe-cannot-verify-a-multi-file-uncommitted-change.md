---
id: c617b7bbbf85fa0c
kind: bug
status: fixed
title: mutation-probe.sh cannot verify a multi-file uncommitted change, and its NOTE states the condition without the consequence
owners:
- marius
tags:
- tooling
- mutation-testing
- cluster/hint-composed-without-the-request
closed: 2026-09-16
opened: 2026-09-15
severity: med
---

# BUG: mutation-probe.sh cannot verify a multi-file uncommitted change, and its NOTE states the condition without the consequence

## Summary

`scripts/mutation-probe.sh` copies the **mutated file's** working-tree content into
the isolated worktree and builds every other file at HEAD. When the change under
test spans several uncommitted files — the normal shape for a slice that alters a
type and its call sites — the worktree does not compile, and the run ends
`INCONCLUSIVE` after paying the full build.

The script does emit a NOTE naming the condition:

```
mutation-probe: NOTE — 4 other .rs file(s) are dirty in the shared tree and
  are NOT carried into the isolated worktree, which builds them at HEAD.
```

It names the **fact** and not the **consequence**. Nothing says *"if any of those
files must compile against the one you are mutating, this run cannot produce a
verdict."* The run then proceeds to `ARMED`, compiles ~330 lines of dependencies,
and reads as progress the whole way.

## Symptom (Effect)

Observed 2026-09-15 while verifying a five-file change (`BackgroundJob` type +
its call sites). Mutating `src/tools/output_buffer.rs` produced:

```
error[E0308]: mismatched types
   --> src/tools/run_command/inner.rs:131:53
131 |     let ref_id = ctx.output_buffer.store_background(log_path);
    |                     expected `BackgroundJob`, found `PathBuf`
mutation-probe: INCONCLUSIVE — no test-count line in the output
```

`INCONCLUSIVE` is honest and the script's own text lists "the mutation did not
COMPILE" first among its three causes, so the diagnosis is reachable. The cost is
the wasted cold build and the fact that the reader learns it at the end rather than
at the NOTE.

## Reproduction

1. Make a change spanning ≥2 source files where one depends on the other's changed
   API, leaving all of them uncommitted.
2. `./scripts/mutation-probe.sh --file <one of them> --find … --replace … -- cargo test --lib <filter>`
3. Observe the NOTE, then `ARMED`, then a compile error, then `INCONCLUSIVE`.

Verified once, end to end, on the change described above.

## Environment

2026-09-15, branch `experiments`, worktree HEAD `fd15aedc`. Linux.

## Root cause

The worktree is seeded from HEAD and then given the single `--file` argument's
working-tree content. That is correct for the single-file mutation the script was
built for and silently insufficient for any wider change. The NOTE is computed from
the dirty-file list alone; whether those files are *needed* to compile the mutated
one is never consulted, though the answer is available the moment `cargo` runs.

## Evidence

`scripts/mutation-probe.sh`; the run transcript above. Baseline control: after
manually copying all five dirty files into the same worktree, `cargo test --lib
tools::output_buffer::tests::` returned **70 passed, 0 failed**, and three separate
mutations were then each KILLED — so the worktree and the tests were both fine and
the file-carrying scope was the only problem.

## Hypotheses tried

That the mutation had failed to apply — ruled out: the script reported
`mutant applied: 1 occurrence(s)` and asserts single-occurrence itself.

## Fix

**FIXED 2026-09-16** in `d8268215` — patch-id `1893534592560b09a10ed5b3babe1cdc2ca5b776`.

**Candidate 1 shipped** — carry every dirty path, not only `--file`. Candidate 2 (keep the
scope, move the consequence into the NOTE) was rejected: it would have left the probe unable
to verify the ordinary shape of real work, while explaining the failure more clearly.

**The "needs thought" attached to candidate 1 does not survive inspection, and naming why
matters more than the change.** It read: *"the point of building others at HEAD may have been
to keep the mutation the only variable."* It was not. The script **already** copied `--file`'s
uncommitted content — case 6 pins exactly that — so the mutation was never the only variable.
Building the others at HEAD was a consequence of `git worktree add HEAD` plus one `cp`, not a
design decision anyone made. The caller's baseline is their own working tree; matching it is
what makes the verdict mean anything.

## What is carried, and why it takes two mechanisms

git reports the two populations separately and neither covers the other:

- **tracked** edits, deletions and renames — `git diff HEAD --binary` applied in the worktree,
  which handles all three in one operation rather than parsing porcelain status codes;
- **untracked** files — copied. A **new module** is the commonest way a slice adds a call site
  and it has no HEAD version to fall back to, so omitting it is a compile error rather than a
  stale build.

Preceded by `git clean -fdq`, so a previous run's leftovers cannot be read as this run's tree.
**No `-x`:** `target/` is ignored and must survive, or the 87 s cold build is paid every time
and the probe stops being cheaper than the shared tree — which is the whole argument for
isolation.

## Two properties that are deliberate and easy to mistake for oversights

**A failed patch application REFUSES rather than falling back.** Building at HEAD after a
failed apply would produce a verdict about a tree that is neither HEAD nor the caller's — code
nobody has — which is the silent-wrong-answer class this script exists to prevent.

**On a shared checkout this carries PEERS' in-flight work too.** Deliberate, not tolerated: it
makes the isolated tree match the one the caller's own `cargo test` would compile, so an
INCONCLUSIVE means their real run would have failed too, rather than that the probe is lying
to them. Isolation of the **mutation** is untouched — nothing is published to the shared tree,
which is the property the script exists for.

## The remedy text had to change with the predicate

The INCONCLUSIVE diagnostic named *"the test is in a file this worktree built at HEAD because
it is uncommitted — only the mutated file is carried across"* as a cause. This fix makes that
cause impossible, so the text now names the causes that remain and states explicitly that an
uncommitted test is no longer one of them. **Case 12 asserted on the old wording**, so the
suite forced the update rather than letting a guard keep sending readers to commit a test that
is already carried — which is the one half of a guard that `CLAUDE.md` § *Testing Discipline*
notes is usually untested by construction.

## Tests added

Cases **19** and **20** in `tests/mutation-probe.sh`, plus a rewritten case 12. Suite goes
**42 → 46 assertions, 18 → 20 cases** (derived 2026-09-16: `grep -cE '^(has|eq) '` for
assertions, distinct leading integers in the labels for cases — the header comments cover
ranges like `15-18`, so counting those returns 22 and is the wrong instrument).

- **19** — a dirty SIBLING file's edit reaches the worktree. Two assertions: the mutated
  file's own edit (case 6's property, restated as a direct observation) and the sibling's.
- **20** — an UNTRACKED new file reaches it. Separate from 19 because the two are carried by
  different mechanisms, so one case cannot guard both.
- **12** — now asserts the remedy names a cause that is still real, and explicitly retires the
  one this fix removed.

**The observation is direct, not inferred.** The test command runs *inside* the worktree, so
the fixtures `cat` the files and assert on the content. Inferring carriage from a build outcome
would conflate *"not carried"* with *"carried and still broken"* — which is precisely the
distinction under test.

## Dogfood — the fix verified by using it on itself

The strongest available check, because it is **structurally impossible under the old
behaviour**. The change under test spans two uncommitted files, and the suite that catches the
mutation lives in the *second* one:

```
./scripts/mutation-probe.sh --file scripts/mutation-probe.sh \
    --find 'cp "$ROOT/$f" "$TREE/$f"' --replace 'true' -- <wrapped suite>

mutation-probe: carried the working tree into the isolated worktree —
  11 tracked change(s), 9 untracked file(s).
mutation-probe: KILLED (rc=1, 45 test(s) ran) — a test caught it.
```

The killer is **case 20, which exists only in the uncommitted sibling file**. At HEAD that case
does not exist, so the old single-file copy could not have produced this red — it would have
reported SURVIVED, a false negative manufactured by the very defect being fixed and
indistinguishable from a real survival.

## Workarounds

Commit the change first (the probe is designed for a committed tree), or stage the
full change into the worktree manually as above.

## Resume

N/A — fixed. `docs/PROBES.md`'s row is updated in the same commit: the count moved to **46
assertions across 20 cases** with its derivation method stated, and the row now describes what
is carried, the shared-checkout consequence, and the refuse-rather-than-fall-back behaviour.
The old limitation is not restated as history — per `CLAUDE.md` § *Parsers Over a Namespace*,
a superseded fact in prose is indistinguishable from a live one, and `git log` holds it.
