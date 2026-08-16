---
status: open
opened: 2026-08-16
closed:
severity: low
owner: marius
related: []
tags: [librarian, silent-failure, api-ergonomics, follow-up]
kind: bug
---

# BUG: `update_entry`'s `entry`-param guard only fires when `fields` is absent

## Summary

`47abcb6d` closed the silent no-op where a patch sent as `entry=` was dropped and
the call reported success. The guard it added is

```rust
if args.get("entry").is_some() && args.get("fields").is_none() { … }
```

so it fires only when `fields` is **absent**. Send both `entry` and `fields` and
`entry` is silently ignored again — the original defect, in a narrower window.

Impact is genuinely small: the accepted parameter wins, so the write that lands
is the intended one. It is filed because the guard exists specifically to stop a
silent ignore, and it still has one.

## Symptom (Effect)

Not yet observed in a session — found by reading the guard while reviewing
`47abcb6d`. Predicted shape:

```
artifact(action="update_entry", …, fields={"status":"done"}, entry={"owner":"x"})
-> {"entry_id": "...", "changed_fields": ["status"], "entries_total": N}
```

`owner` is never written and nothing says so. `changed_fields` naming only
`status` is the sole hint, and it reads as a correct report of a one-field patch.

## Reproduction

Not yet reproducible against a live call — the prediction above is read from
`src/librarian/tools/update_entry.rs` (the `args.get("entry").is_some() &&
args.get("fields").is_none()` condition) and has not been executed. Best lead:
issue the call above against any augmented tracker and read `changed_fields`.

## Environment

codescout `experiments` at `148aabe6`; guard introduced in `47abcb6d`.

## Root cause

Inferred from `src/librarian/tools/update_entry.rs` — **not measured**. The guard
is written as a disambiguation ("you meant `fields`, you said `entry`") rather
than as a rejection of an unknown key, so it steps aside as soon as the accepted
key is also present. `serde` then deserialises `Args` and drops `entry` as
undeclared, which is the same mechanism as the parent bug
(`docs/issues/2026-08-16-update-entry-ignores-an-unknown-patch-param-and-reports-success.md`).

## Evidence

### The guard, verbatim

```rust
if args.get("entry").is_some() && args.get("fields").is_none() {
    return Err(RecoverableError::with_hint(
        "update_entry: `entry` is append_entry's parameter — this action takes `fields`",
        …
    ));
}
```

The `&& args.get("fields").is_none()` conjunct is the hole.

## Hypotheses tried

1. **Hypothesis** — sending both is unreachable in practice, so the conjunct is
   harmless. **Test** — none run. **Verdict** — deferred. It is certainly rare;
   "rare" is why the parent bug survived to be filed by a user rather than a test.

## Fix

Drop the `&& args.get("fields").is_none()` conjunct so `entry` is refused
whenever present. The existing error text already reads correctly for the
both-present case — it names `fields` as the accepted parameter, which is still
the right instruction.

The alternative, rejecting all undeclared top-level keys on the librarian
surface, is the general form and was already noted as the larger change in the
parent bug's Resume.

## Tests added

None yet. One row added to the existing table in
`src/librarian/tools/update_entry.rs`'s tests: `entry` **and** `fields` both
present must be a `RecoverableError`, not a partial write.

## Workarounds

Never send `entry` to `update_entry`. Read `changed_fields` after every call and
confirm it names every key you patched — that check catches this and any future
variant.

## Resume

Delete the `&& args.get("fields").is_none()` conjunct in
`src/librarian/tools/update_entry.rs`, add the both-present test row, run
`cargo test --lib -- update_entry`. Before doing so, run the call once and record
the real output in *Symptom* — this file's mechanism is currently inferred from
source and has not been executed, which is exactly the premise a later session
should not inherit unchecked.

## References

- `src/librarian/tools/update_entry.rs` — the guard
- `docs/issues/2026-08-16-update-entry-ignores-an-unknown-patch-param-and-reports-success.md` — parent bug
- commit `47abcb6d` — the fix this follows up
