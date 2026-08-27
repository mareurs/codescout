---
id: '015ab74ec0b16717'
kind: bug
status: open
title: 'BUG: edit_code(remove) can never remove an impl block — its own methods trip the sibling guard, and both escapes the hint offers are closed'
tags:
- edit_code
- tooling
- guard
- false-positive
closed: null
opened: 2026-08-27
owner: marius
related:
- docs/issues/archive/2026-08-27-unregistered-memory-tool-structs-read-as-the-live-tool.md
severity: low
unverified: Root cause is INFERRED from the error message and the reproduction; the guard's source was not read. Only `impl` blocks were exercised — whether `mod` blocks or nested types hit the same check is untested.
---

## Summary

`edit_code(action="remove", symbol="impl Trait for Type")` **can never succeed**. The
guard that protects against an overshooting LSP range treats the impl block's own methods
as "sibling symbols" that would be dropped — but an impl block's methods are *always*
inside its range, by definition. So the check fires on every well-formed removal.

The error text then misdiagnoses it as a stale range, and its hint sends the caller down
a route that also cannot work.

## Symptom

```
edit_code(action="remove", path="src/tools/memory/mod.rs",
          symbol="impl Tool for DeleteMemory")

→ edit_code remove('impl Tool for DeleteMemory') would have dropped sibling symbols:
    DeleteMemory/call, DeleteMemory/description, DeleteMemory/input_schema,
    DeleteMemory/name.
  The range overshot into adjacent code (likely a stale LSP range). File restored.
  hint: Try symbols(path) to refresh, then retry; or narrow the edit via edit_file
        with unique anchors.
```

Every name in that list is a **child** of the symbol being removed, not a sibling.

## Reproduction

Measured 2026-08-27 on `src/tools/memory/mod.rs`:

1. `edit_code(remove, symbol="impl Tool for WriteMemory")` → fails as above.
2. `symbols(path="src/tools/memory/mod.rs")` — ranges confirmed fresh and correct
   (`Object 214-263 impl Tool for DeleteMemory`, methods `215-217`, `218-220`, `221-233`,
   `234-262`, all strictly inside).
3. `edit_code(remove, symbol="impl Tool for DeleteMemory")` → **fails identically.**

Two different impl blocks, once before and once after an explicit refresh. Not a stale
range.

## Root cause

Not read. The behaviour is consistent with the removal guard collecting every symbol
whose span intersects the target's span, then rejecting if any of them is not the target
itself — correct for a genuinely overshooting range, wrong for any container symbol
(`impl` blocks, and presumably `mod` blocks and nested types too).

The discriminator the guard is missing is *containment*: a symbol strictly inside the
target's span is a child being removed on purpose; one that merely overlaps is evidence
of an overshoot.

## Both suggested escapes are closed

The hint offers two routes, and in this repo neither is open:

1. **"Try `symbols(path)` to refresh, then retry."** Step 2 of the reproduction did
   exactly this. The ranges were already correct, so the refresh changes nothing and the
   retry fails the same way. The hint describes a cause that is not the cause, so
   following it costs a round trip and teaches the wrong lesson.
2. **"Narrow the edit via `edit_file` with unique anchors."** `edit_file` is refused by
   the IL-2 gate whenever the edit spans a symbol definition — which removing an `impl`
   block necessarily does. So the fallback is blocked by a different guard.

The only route through was a raw `python3` script rewriting the file by verified line
range. That works, but it is precisely the unstructured edit `edit_code` exists to
replace, and it carries none of the range validation.

## Impact

Low frequency, total when it fires. Deleting a trait implementation is not a common edit,
but when it is needed there is no supported path — and the guidance actively points away
from the workaround. It also means `edit_code`'s `remove` action is unusable for the one
class of symbol where the blast radius is largest and its validation would be worth most.

## Fix

Make the guard test **containment**, not intersection: a symbol whose span lies entirely
within the target's span is a child and should be removed with it; only a symbol that
extends *beyond* the target's span is evidence of an overshoot.

Failing that, at minimum stop calling children "siblings" and stop attributing the refusal
to a stale range, since a caller who refreshes learns nothing and retries into the same
wall.

## Not established

Whether `mod` blocks, nested types, or any other container symbol hit the same guard —
only `impl` blocks were exercised. The guard's source was not read; the root cause above
is inferred from the message and the reproduction, not from the code.

