---
kind: bug
status: fixed
tags:
- librarian
- silent-failure
- api-ergonomics
- follow-up
- cluster/guard-narrower-than-its-name
closed: 2026-08-16
opened: 2026-08-16
owner: marius
related: []
severity: low
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

**Measured 2026-08-16**, as this file's own Resume required — the mechanism below
was inferred from source when filed, and has now been executed. Against the live
queue tracker, sending both parameters with deliberately divergent payloads:

```
artifact(action="update_entry", id="9a892c2a5976e296", entry_collection="tasks",
         entry_id="BL-27",
         entry={"status":"done", "task":"SENTINEL-entry-was-applied"},
         fields={"status":"open"})
->
{"entry_id":"BL-27", "artifact_id":"9a892c2a5976e296",
 "changed_fields":["status"], "entries_total":30}
```

Success. `changed_fields` names only `status`, from **`fields`**. Reading the row
back confirms the sentinel never landed:

```
$.entries[0].task -> "update_entry's entry-param guard only fires when fields is
                      absent — send both and `entry` is silently dropped again"
```

Unchanged. So `entry`'s payload — both the `status: done` and the `task` rewrite —
was discarded with no signal, which is precisely the silence the parent bug was
filed to remove.
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

**Implemented 2026-08-16 on `experiments`.** One conjunct, as this file proposed.

`src/librarian/tools/update_entry.rs`:

```rust
- if args.get("entry").is_some() && args.get("fields").is_none() {
+ if args.get("entry").is_some() {
```

The existing error text already read correctly for the both-present case, so no
wording change was needed.

**Why the conjunct was there, and why that matters more than the line.** It
narrowed the guard to exactly the case that had been reported and tested. That is
the same defect shape as the bug fixed in the commit immediately before it
(`47abcb6d`), where `edit_file`'s librarian guard covered one write path out of
three — a guard scoped to the observed instance rather than to the condition it is
meant to enforce. Two in one day is worth naming as a pattern: **when writing a
guard, state the condition, not the reproduction.**
## Tests added

`call_rejects_the_entry_param_and_names_fields`
(`src/librarian/tools/update_entry.rs`) was widened from a single case to a table:

- `entry` alone — passed before this change
- `entry` alongside `fields` — **failed** before it

Both rows assert the error names `fields` *and* that the row was not written on
the way to the error. The first row is what makes the second meaningful: it proves
the table discriminates rather than refusing everything.

Run red before the fix, on exactly the reported shape:

```
entry alongside fields: `entry` must be refused:
  Object {"changed_fields": Array [String("status")], "entries_total": Number(2)}
```

Gate: `cargo fmt` clean, `cargo clippy --all-targets -- -D warnings` clean,
`cargo test --lib` 3742 passed / 0 failed.
## Workarounds

Never send `entry` to `update_entry`. Read `changed_fields` after every call and
confirm it names every key you patched — that check catches this and any future
variant.

## Resume

**Closed and verified 2026-08-16.** Fast-forward promotion, so the `experiments`
SHA is the master SHA — no second SHA to record.

Verified live after `cargo rb` + `/mcp` with the byte-identical call from
§ Symptom — same artifact, same entry, same divergent payloads:

```
artifact(update_entry, id="9a892c2a5976e296", entry_collection="tasks",
         entry_id="BL-27",
         entry={"status":"done", "task":"SENTINEL-entry-was-applied"},
         fields={"status":"open"})
->
update_entry: `entry` is append_entry's parameter — this action takes `fields`
hint: Re-send the patch as fields={...}. `entry` is the whole row for a NEW
      entry; `fields` is the subset to change on an existing one.
```

That call previously returned `{"changed_fields":["status"], "entries_total":30}`
with `entry`'s payload silently discarded. Refused now, and the sentinel still
never reached the row.
## References

- `src/librarian/tools/update_entry.rs` — the guard
- `docs/issues/2026-08-16-update-entry-ignores-an-unknown-patch-param-and-reports-success.md` — parent bug
- commit `47abcb6d` — the fix this follows up
