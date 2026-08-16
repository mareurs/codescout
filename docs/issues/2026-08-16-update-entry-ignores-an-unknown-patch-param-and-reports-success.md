---
status: fixed
opened: 2026-08-16
closed: 2026-08-16
severity: medium
owner: marius
related: []
tags: [librarian, silent-failure, api-ergonomics, tool-quirk]
kind: bug
---

# BUG: `artifact(action="update_entry")` silently no-ops when the patch arrives under the wrong param name

## Summary

`update_entry` takes the patch as **`fields`**; its sibling `append_entry` takes
the entry as **`entry`**. Passing `entry` to `update_entry` is not rejected — the
unknown key is dropped, `fields` defaults to an empty object, and the call
returns a **success** envelope. Nothing is written. The only signal is
`changed_fields: []`, which reads as "this patch changed nothing" rather than
"your patch never arrived".

The write path this replaced was flagged for being silent
(`02a87a83` — "stop the wholesale replace being silent"). This is a second,
narrower silence in the replacement.

## Symptom (Effect)

Observed while filling the `outcome` of ledger row A-22 in
`docs/trackers/prompt-hamsa-audit-log.md`:

```
artifact(action="update_entry", id="59ebeebb6ed05c89",
         entry_collection="audits", entry_id="A-22",
         entry={"outcome": "<~1.4 KB of text>"})
->
{
  "entry_id": "A-22",
  "artifact_id": "59ebeebb6ed05c89",
  "changed_fields": [],
  "entries_total": 22
}
```

Exit is success. Re-reading the row shows `"outcome": ""` — unchanged. The
identical call with `fields=` instead of `entry=` returns
`"changed_fields": ["outcome"]` and does write.

## Reproduction

At `26ce904b`, against any augmented artifact declaring an `entry_collection`:

1. `artifact(action="update_entry", id=<id>, entry_collection=<coll>, entry_id=<row>, entry={"someField": "x"})`
2. Observe success with `changed_fields: []`.
3. Re-read the row — `someField` is unchanged.
4. Repeat with `fields={"someField": "x"}` — now it writes.

## Environment

codescout `experiments`, HEAD `26ce904b`. `update_entry` landed the same day in
`02a87a83`.

## Root cause

Two independent contributors, both cited at `path:line`:

1. **Param-name asymmetry between siblings.** `append_entry` names its payload
   `entry`; `update_entry` names its patch `fields`
   (`src/librarian/catalog/augmentation.rs:251-257`). Nothing about the action
   name signals which noun applies, so `entry` on an entry-update reads as the
   obvious choice.
2. **An empty patch is a legal success, and unknown keys are not rejected.**
   `update_entry` guards `fields` being a non-object
   (`augmentation.rs:258-262`), rejects an `id` key (`:263-268`), and errors on
   a missing collection (`:286-310`) or unknown `entry_id` (`:332-333`) — but
   an *empty* object passes every guard and completes having touched nothing.
   Combined with the MCP layer dropping the undeclared `entry` key rather than
   erroring, a typo'd param becomes an empty patch becomes a reported success.

The mechanism is verified, not inferred: measured 2026-08-16 by issuing both
calls back to back against the same row and reading the row in between —
`entry=` → `changed_fields: []`, row unchanged; `fields=` → `changed_fields:
["outcome"]`, row written.

## Evidence

### The two calls, same row, minutes apart

```
entry=  -> {"entry_id":"A-22","changed_fields":[],       "entries_total":22}   # no write
fields= -> {"entry_id":"A-22","changed_fields":["outcome"],"entries_total":22} # write
```

### Why `changed_fields: []` is a weak signal

It is genuinely ambiguous between three states: the patch was empty, the patch
named only fields already holding those values, and the patch never arrived.
Only the third is a caller error, and the envelope cannot distinguish them.

## Hypotheses tried

1. **Hypothesis** — the write landed but the read was served from a stale cache.
   **Test** — re-read the row via `artifact(action="get", entry_filter=...)`
   after the `entry=` call. **Verdict** — rejected; `outcome` was `""`, and the
   subsequent `fields=` call on the same row wrote immediately with no
   intervening cache action.

## Fix

**Implemented 2026-08-16 on `experiments`.** Fix 1 as proposed. **Fix 2 (accept
`entry` as an alias) was considered and deliberately rejected** — reasoning below,
since the plan above recommended it.

**Two guards, at the two layers where the two contributors live.**

1. *Empty patch* — `src/librarian/catalog/augmentation.rs`, beside the `id`-key
   guard. Any caller reaching the catalog with `{}` now gets a
   `RecoverableError` naming the `entry`/`fields` asymmetry, so the general case
   is covered however it arrives.

2. *Wrong param, by name* — `src/librarian/tools/update_entry.rs`, before
   deserialization. When `entry` is present and `fields` is absent, the error can
   say **which** parameter to use instead of the generic "nothing to patch":

   ```
   update_entry: `entry` is append_entry's parameter — this action takes `fields`
   hint: Re-send the patch as fields={...}. `entry` is the whole row for a NEW
         entry; `fields` is the subset to change on an existing one.
   ```

The schema description for `fields` now states the asymmetry as well, so it is
visible before the call rather than only after it fails.

### Why not the alias

Aliasing would make `entry` mean two different things across sibling actions: a
**whole row** on `append_entry`, a **partial patch** here. That is the confusion
which produced this bug, entrenched rather than removed — and the wrong reading it
invites is specifically "`entry` replaces the row", which is the wholesale-replace
semantics `update_entry` exists to retire
(`docs/issues/archive/2026-08-16-params-merge-patch-wipes-entry-arrays-with-no-guard.md`).
A caller holding that belief would be surprised in the *destructive* direction.

Rejecting costs one round trip and teaches the distinction once. Renaming `fields`
to `entry_fields` (or similar) would remove the trap outright and is the better
long-term shape, but it is a breaking change to a param that shipped hours ago —
worth doing deliberately, not as a bug fix.

### On `changed_fields: []` being ambiguous

§ Evidence is right that it cannot distinguish "patch was empty", "patch was a
no-op", and "patch never arrived". With both guards in place the first and third
are now errors, so the surviving meaning is narrow: the fields named already held
those values.
## Tests added

Written first and watched fail — all three reproduced the reported envelope
exactly (`Ok(... changed_fields: [], ...)`).

`src/librarian/catalog/augmentation.rs`:

- `update_entry_rejects_an_empty_patch`

`src/librarian/tools/update_entry.rs`:

- `call_rejects_the_entry_param_and_names_fields` — asserts the error names
  `fields`, **and** that the row was not written on the way to the error.
- `call_rejects_an_empty_fields_patch`

The alias test from the original plan is intentionally absent — see § Fix.

Gate: `cargo fmt` clean, `cargo clippy --all-targets -- -D warnings` clean,
`cargo test` 3879 passed / 0 failed / 45 ignored.
## Workarounds

Use `fields=` for `update_entry` and `entry=` for `append_entry`. Treat
`changed_fields: []` as a failed write and re-read the row before believing any
entry update — the envelope's `entry_id` echo does not mean anything was
written.

## Resume

**Fixed on `experiments`.** Verify live after the next `cargo rb` + `/mcp`: issue
the reported call shape (`entry=` on `update_entry`) and confirm it now returns a
`RecoverableError` naming `fields` rather than a success envelope; then confirm
`fields=` still writes.

The A-22 row in `docs/trackers/prompt-hamsa-audit-log.md` that surfaced this may
still have an empty `outcome` — worth re-checking and filling, since the original
write never landed.

One thing deliberately left: renaming `fields` to something that cannot be
confused with `append_entry`'s `entry`. That removes the trap rather than
reporting it, but it is a breaking change to a param that shipped the same day —
reopen as its own change if the guard proves insufficient.
## References

- `src/librarian/catalog/augmentation.rs:226-333`
- commit `02a87a83` — introduced `update_entry`
- `docs/issues/archive/2026-08-16-params-merge-patch-wipes-entry-arrays-with-no-guard.md`
  — the silent-write defect `update_entry` was built to fix
- `docs/trackers/prompt-hamsa-audit-log.md` row A-22 — where this surfaced
