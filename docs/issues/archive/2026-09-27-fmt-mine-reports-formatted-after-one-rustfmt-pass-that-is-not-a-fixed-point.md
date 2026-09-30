---
id: 17bee02774495475
kind: bug
status: fixed
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

Deterministic, reproduced 2026-09-27 in a scratch file holding only this function inside a `mod m { … }`, at 4- and at 8-space base indentation. **The `mod m { … }` wrapper is required, and the snippet below is shown unwrapped:** re-derived 2026-09-30, the bare function at column 0 reaches a fixed point in one pass (`--check` rc 0 after pass 1); wrapped in `mod m { … }` the same function gives `--check` rc 1 after pass 1 and rc 0 after pass 2.

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

rustfmt 1.9.0-stable (8bab26f4f6 2026-07-14); `experiments` at `00286ae6`; observed in `src/librarian/tools/append_entry.rs` while implementing residual `96b2b1b9a25bb1b0`.

## Root cause

Two parts. rustfmt's non-idempotence on an over-long literal inside a returned macro call in a match arm is upstream behaviour and not ours to fix. Ours is that `fmt-mine.sh` treats one pass as done: it reports `formatted` without the `--check` it already knows how to run at stage 1, so its success line asserts a fixed point it never observed. `docs/issues/2026-09-19-file-provenance-answers-at-session-grain-so-sibling-subagents-are-one-writer.md` also states "rustfmt is idempotent" as a premise; this reproduction falsifies that premise for this construct.

## Fix

Fixed in `5487b52f` (patch-id `d29ab890e40ccc067186143c5f880acf1987839b`, `git show <sha> | git patch-id --stable`). `scripts/fmt-mine.sh` now runs `rustfmt` then `rustfmt --check` in a loop bounded at 3 passes. It prints `formatted` only after a clean check, and otherwise prints `NOT a fixed point after N pass(es)`, names the files still dirty, and exits 1. rc > 1 from `--check` reads as still-dirty, which can only withhold the word `formatted`.

## Tests added

`tests/fmt-mine.sh` case 10 (the two-pass construct, wrapped in `mod m`; asserts a fresh `rustfmt --check` on the resulting bytes is clean) and case 11 (a fake `rustfmt` on PATH that never settles; asserts exit 1, `NOT a fixed point`, the file named, and no `formatted N file(s)` line). Suite 50 passed, 0 failed. Mutations, one per guarded site, run with `scripts/mutation-probe.sh`: `FMT_MAX_PASSES=3` to `1` killed case 10; the `--check` verdict replaced by `true` killed case 10 and all four case-11 assertions; the failure branch's `exit 1` to `exit 0` killed case 11 `exits 1`.

## Fix provenance

- **SHA:** `5487b52fda1210de7f2efeab5e8c9e7878e217f3` (`experiments`)
- **patch-id:** `d29ab890e40ccc067186143c5f880acf1987839b`

## References

- `scripts/fmt-mine.sh` — the single pass and its success line
- `docs/issues/2026-09-19-file-provenance-answers-at-session-grain-so-sibling-subagents-are-one-writer.md` — states the idempotence premise
