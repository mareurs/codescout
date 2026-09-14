---
kind: bug
status: fixed
tags:
- cluster/repro-env-diverges-from-gate-env
closed: 2026-09-14
opened: 2026-09-14
owner: marius
related: []
severity: medium
---

# BUG: the ledger hook reads stdin with the LOCALE encoding, so Windows parses `â€”` where the file holds `—`

## Summary

`scripts/pre-commit-ledger-counts.py` reads every `--fixture-*` corpus from `sys.stdin`, whose
encoding comes from the locale: UTF-8 on Linux, **cp1252 on Windows**. The ledger's headings carry
em dashes (`## IC-7 — ...`), so on Windows the parser is handed `â€”` where the file holds `—`.

The two derivations — this script and `tests/issue_clusters.rs` — then agree about every *rule* and
disagree only about the *bytes one of them was given*, which is reported as a rule disagreement.

## Symptom (Effect)

All three `windows-latest` CI jobs (default, no-features, local-embed) red on the same two tests
while every Linux lane is green:

    the_hook_script_agrees_on_the_index_section_scan      tests\issue_clusters.rs:2058
    the_hook_script_agrees_on_the_mechanism_basis_scan    tests\issue_clusters.rs:1269

    left:  ["## IC-7 — a class section left behind in the parent", ...]
    right: ["## IC-7 â€” a class section left behind in the parent", ...]

The failure message says the two sides *"disagree about which Index headings are class sections"*,
which is true of the output and wrong about the cause — a reader is sent to audit two parsers that
are both correct.

**The hook itself is affected, not only the tests.** A Windows checkout running the pre-commit hook
mis-parses every non-ASCII ledger line, and nothing reports it.

## Reproduction

Locale-independent, so the Windows read reproduces anywhere — `PYTHONIOENCODING` sets the
std-stream encoding on every platform:

```
printf '## IC-7 — a class section left behind in the parent\n' \
  | PYTHONIOENCODING=cp1252 python3 scripts/pre-commit-ledger-counts.py --fixture-index-sections
```

    before: ["## IC-7 â€” a class section left behind in the parent"]
    after:  ["## IC-7 — a class section left behind in the parent"]

## Environment

- Date: 2026-09-14
- Branch `experiments`, CI run 34825264309 at `9045c56a`
- Local: Python 3.14, Linux; CI: `windows-latest`

## Root cause

`sys.stdin` / `sys.stdout` / `sys.stderr` take their encoding from the locale, and unlike a file
read there is **no call-site parameter to pass one to**. Every file read in this script already
passes `encoding="utf-8"` explicitly (see `read`), so the file half was never wrong — only the
streams, which are the half a reviewer does not see a call site for.

Why no local instrument could catch it: the gate runs on Linux, where the locale is already UTF-8.
The check is correct, its measurement environment cannot express the failure. That is
`cluster/repro-env-diverges-from-gate-env`.

## Evidence

Two write-direction facts, measured because the obvious guess is wrong on both counts:

- `—` **is** in cp1252 (0x97), so a refusal naming a heading does not raise. It emits one cp1252
  byte where the reader expects UTF-8 — the message arrives mangled with nothing reporting it.
- A character cp1252 lacks (`→`, `✓`) raises `UnicodeEncodeError` on stdout, whose handler is
  `strict`. stderr's is `backslashreplace` and degrades instead.

The JSON path is immune either way (`json.dumps` escapes non-ASCII by default), so no fixture test
can observe the write half; it is verified by reading the streams' `.encoding` directly.

## Fix

Reconfigure all three std streams to UTF-8 once, at module top, before any read:

```python
for _stream in (sys.stdin, sys.stdout, sys.stderr):
    if _stream is not None:
        _stream.reconfigure(encoding="utf-8")
```

One site rather than seven, and it covers the write direction the fixture tests cannot reach.

**Fixed on `experiments` at `f4dc25f9`**, patch-id `f0aa110f73306c3f6e105a3c63277aa12577d8ea`.

## Tests added

`the_hook_script_reads_stdin_as_utf8_whatever_the_locale_says` (`tests/issue_clusters.rs`) —
feeds `INDEX_SECTION_FIXTURE` through the hook under `PYTHONIOENCODING=cp1252`, so the guard runs
in the local gate instead of waiting for a push.

**Observed red, on the production path:** deleting the `reconfigure` loop reds the test with
output byte-identical to CI's (`## IC-7 â€” ...`). Restored and re-verified green.

It also asserts `!INDEX_SECTION_FIXTURE.is_ascii()` — under cp1252 an all-ASCII fixture
round-trips perfectly, so an ASCII-ifying tidy-up would leave the test green and no longer
discriminating, which is the one change no other assertion here can catch.

Declared in `NOT_HOOK_OWED`: a test OF the hook, not a rule the hook enforces.

## Workarounds

`PYTHONUTF8=1` in the environment, per-checkout. Not used — it fixes the machine that sets it and
leaves the script wrong.

## References

- CI run 34825264309 — three `windows-latest` jobs, same two tests
- `.github/workflows/ci.yml` § `windows-gnu` — the lane comment already names IC-5 for wine
