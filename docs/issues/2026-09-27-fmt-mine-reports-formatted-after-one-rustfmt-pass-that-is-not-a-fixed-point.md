---
id: d1eff909c0d8a73a
kind: bug
status: open
title: 'BUG: fmt-mine.sh reports "formatted" after one rustfmt pass that is not a fixed point'
tags:
- cluster/record-asserts-an-unchecked-completion
- fmt-mine
- rustfmt
- gate
---

# BUG: fmt-mine.sh runs rustfmt once and reports "formatted", but rustfmt can need a second pass to reach a fixed point

**Valid:** dated 2026-09-27

## Summary

`scripts/fmt-mine.sh` ends with one `rustfmt --edition 2021 $MINE` and prints `fmt-mine: formatted N file(s) written by this session.` It never re-checks. rustfmt 1.9.0 is **not idempotent** on at least one construct — a match arm `Err(e) => { return Some(format!(<over-long string literal>, …)) }` — so after that one pass the file still fails `rustfmt --check` with the same invocation. The script's own success line is therefore a completion it did not verify; the pre-commit `rustfmt --check (committed bytes)` hook is what caught it, and the gate's stage 1 (`cargo fmt -- --check`) would red on the next run.

## Symptom (Effect)

A session that ran `./scripts/fmt-mine.sh` (reported formatted), then committed, had the commit refused by the pre-commit rustfmt check on a hunk it had just formatted. The natural reading is "my edit broke formatting after I formatted", not "the formatter stopped halfway".

## Reproduction

Deterministic, reproduced 2026-09-27 in a scratch file holding only this function inside a `mod m { … }`, at 4- and at 8-space base indentation:

```rust
fn check_prose(root: &Path, r: &Recipe) -> Option<String> {
    let fm = match read_fm(&root.join(&r.target)) {
        Ok(fm) => fm,
        Err(e) => {
            return Some(format!(
                "{}: routes {}-N writes to `{}`, which {e} — archived or moved? Update the row.",
                r.at(),
                r.id_prefix,
                r.target
            ))
        }
    };
    None
}
```

`rustfmt --edition 2021 f.rs` (rc 0) → `rustfmt --edition 2021 --check f.rs` rc **1** → a second `rustfmt --edition 2021 f.rs` → `--check` rc **0**. Same result at both indentations. The first pass collapses the arm to `Err(e) => return Some(format!(…)),`; the check (a second formatting) re-expands it to the block form, which is then stable.

## Environment

rustfmt 1.9.0-stable (8bab26f4f6 2026-07-14); `experiments` at `00286ae6`; observed in `src/librarian/tools/append_entry.rs` while implementing residual `5820a75840dd2d52`.

## Root cause

Two parts. rustfmt's non-idempotence on an over-long literal inside a returned macro call in a match arm is upstream behaviour and not ours to fix. Ours is that `fmt-mine.sh` treats one pass as done: it reports `formatted` without the `--check` it already knows how to run at stage 1, so its success line asserts a fixed point it never observed. `docs/issues/2026-09-19-file-provenance-answers-at-session-grain-so-sibling-subagents-are-one-writer.md` also states "rustfmt is idempotent" as a premise; this reproduction falsifies that premise for this construct.

## Fix

Not started. Candidate: after `rustfmt --edition 2021 $MINE`, re-run `rustfmt --check --edition 2021 $MINE` and repeat the format up to a small bound (2–3 passes); report `formatted` only on a clean check, and name the file and the pass count otherwise. A test in `tests/` can pin it with the construct above as the fixture.

## Tests added

None yet.

## References

- `scripts/fmt-mine.sh` — the single pass and its success line
- `docs/issues/2026-09-19-file-provenance-answers-at-session-grain-so-sibling-subagents-are-one-writer.md` — states the idempotence premise
