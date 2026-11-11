---
kind: tracker
status: active
title: "Evidence — managed-artifact refusals lead with what is allowed and end in shell reads"
tags:
  - frictions
  - librarian
  - edit_file
  - read_file
---

# Evidence — managed-artifact refusals lead with what is allowed and end in shell reads

Ledger entry: FR-8 in [index.md](index.md). Window: 2026-10-12 to 2026-11-10 UTC. Corpus: `.codescout/usage.db`. Source: the `editfile`, `readfile` and `docmem` miners; counts re-checked by the lead.

## Scale

`err_family='librarian_managed_artifact'`, `id<298900`: `read_file` 110 (80 ledgers, 30 augmented; 68 sessions), `edit_file` 90 (40 sessions).

## `edit_file`

Row 291147 sent one `old_string`/`new_string` pair for body text of an issue file. The refusal opens:

> Reads and BODY edits are allowed directly, but only via a call that can prove it stays in the body … plain text grammar (old_string/new_string) … is refused here even with no frontmatter param.

The first clause says body edits are allowed. The last clause refuses the form the agent used. The guard cannot bound the extent of a plain edit, so it refuses every plain edit.

Next call after the refusal: `doc` success 30, `doc` error 8, `edit_file` success 19, `edit_file` error 14 (immediate retries, several on one path), `run_command`/`read_file`/`grep` 24. Only 5 of the 20 retries were checked for path identity.

## `read_file`

Next call after the 110 refusals: `doc(get)` 36 (recovered), the same `read_file` 21, a shell read 25. Example of the shell route: row 182655, `sed -n '60,300p' docs/trackers/codex-pilot-checks-2026-10-13.md`, which succeeded. The shell route makes the read refusal advisory.

The refusal hint does name the read route (`doc(action="get", id=…)`). This session met the same refusal on `docs/trackers/frictions/index.md` and followed the hint.

## Related

- `docs/trackers/codescout-usage-frictions.md` U-50 (agent habit).
- `docs/issues/archive/2026-10-26-librarian-guard-refusals-print-a-placeholder-id-and-a-ledger-read-has-no-read-route.md` (fixed).

## Idea (hypothesis, not tested)

For `edit_file`: lead with the allowed form (`doc(update, body_edits)` or the heading grammar) and state the refusal second. For `read_file`: no guard change is obvious, because the guard does not see the shell. The ledger read route is `doc(get, heading=…)`.

## Not verified

- Whether the 14 immediate `edit_file` retries were on one path (5 of 20 checked).
- Whether a shell read of a managed file is a problem or a fair escape. The refusal exists to protect writes; a read cannot corrupt a ledger.
