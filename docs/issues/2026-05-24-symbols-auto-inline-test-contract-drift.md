---
status: open
opened: 2026-05-24
closed:
severity: low
owner: marius
related: []
tags: [tests, symbols, ci, contract-drift]
kind: bug
---

# BUG: `symbols_no_body_start_line_without_include_body` asserts old contract that auto-inline change superseded

## Summary

`tests/symbol_lsp.rs:1129` asserts that calling `Symbols` without an
explicit `include_body: true` returns a result with no `body` field.
A feature shipped earlier — `feat(symbols): auto-inline body when result
is small and caller did not opt in` (commit `13e6d482`) — changed the
contract: small results now get `body` auto-inlined regardless of the
include_body argument. The test was not updated when the behavior
changed; now fails consistently. Surfaced once CI started running tests
again on 2026-05-24 after 6 weeks of dormant push trigger.

## Symptom (Effect)

```
thread 'symbols_no_body_start_line_without_include_body' panicked at tests/symbol_lsp.rs:1150:5:
body should not be present without include_body
```

Fails consistently under both default features and `--no-default-features`.

## Reproduction

```bash
git rev-parse HEAD
# any commit on experiments since 2026-05-24

cargo test --test symbol_lsp symbols_no_body_start_line_without_include_body
# → FAILED, 1 test
```

Input is `src/lib.rs` containing 22 characters: `#[test]\nfn target() {}\n`.
The auto-inline threshold (whatever it is) trivially fires; body is
attached even though `include_body` wasn't passed.

## Environment

- OS: Linux 7.0.9-zen1-1-zen and CI ubuntu-latest
- Branch: experiments
- Rust: stable

## Root cause

`feat(symbols): auto-inline body when result is small and caller did not
opt in` (commit `13e6d482`) intentionally changed the contract: small
results return a `body` field even without `include_body=true`. The
documented justification is reducing the "two calls to get a body"
round-trip for tiny symbols. The test file's assertion was not updated
to match — it still encodes the old "no include_body → no body" contract.

## Evidence

- The test file's docstring on the test: *"symbols without include_body
  should NOT have body_start_line."* — old contract.
- The commit message of `13e6d482` (in `git log`) explicitly states the
  new behavior.
- Both default and `--no-default-features` runs fail identically.

## Hypotheses tried

N/A — root cause clear from git log + test source.

## Fix

Three plausible options:

1. **Update assertion to match new behavior** — accept that `body` may
   be present for small results; possibly assert the body content
   matches expected.
2. **Force auto-inline to skip via explicit `include_body: false`** —
   if the tool respects an explicit false to opt out of auto-inline,
   change the test's call to include it.
3. **Use a larger fixture** — if the auto-inline threshold is by body
   size, craft an input that exceeds it; the original "no body without
   include_body" contract still holds above the threshold.

Recommend option 2 if explicit `false` opt-out exists, else option 1.

## Tests added

The test itself is the test. Fix updates it in place.

## Workarounds

None needed at the user level — auto-inline is a pure feature, not a
bug. The test is the only thing that breaks.

## Resume

1. Read `src/tools/symbols.rs` (or wherever the auto-inline logic
   lives) to find the threshold + whether explicit `include_body=false`
   opts out.
2. Apply option 2 if opt-out exists; else option 1.
3. Run `cargo test --test symbol_lsp` to verify green.

## References

- `tests/symbol_lsp.rs:1129` — the failing test.
- `commit 13e6d482` — the behavior change.
- `docs/issues/2026-05-24-tool-docs-manual-drift.md` — sister
  pre-existing rot uncovered by the same CI restart.
