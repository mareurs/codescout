---
id: a0dd1c43aeef41de
kind: bug
status: open
title: mutation-probe.sh cannot verify a multi-file uncommitted change, and its NOTE states the condition without the consequence
owners:
- marius
tags:
- tooling
- mutation-testing
- cluster/hint-composed-without-the-request
opened: 2026-09-15
severity: low
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

Not implemented. Two candidates, neither costed:

1. Carry **every** dirty tracked source file into the worktree, not just `--file`.
   Closest to what a caller means by "test my change". Changes what the probe
   isolates against, so it needs thought: the point of building others at HEAD may
   have been to keep the mutation the only variable.
2. Keep the scope and move the consequence into the NOTE — if any other dirty file
   is in the same crate, say that the run will likely not compile, before arming.

Workaround used in the meantime, and it worked: create/reuse the worktree, copy the
full change in by hand, mutate there. The isolation property is preserved — no red
is published to the shared tree — which is the rule's actual purpose.

## Tests added

None.

## Workarounds

Commit the change first (the probe is designed for a committed tree), or stage the
full change into the worktree manually as above.

## Resume

Decide between the two fix candidates. Related: `docs/PROBES.md` indexes this script
and would need the limitation stated in its row.
