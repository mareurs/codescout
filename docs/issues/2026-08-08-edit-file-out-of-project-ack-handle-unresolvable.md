---
id: a99388a299352d21
kind: bug
status: open
title: edit_file's out-of-project ack handle does not resolve — the hint it prints cannot be followed
owners:
- marius
tags:
- tooling
- write-guard
- ack
- misleading-error
opened: 2026-08-08
severity: low
---

## Summary

`edit_file` on a path outside the project root returns a `pending_ack` envelope whose
hint says to re-invoke as `edit_file(path="@ack_xxx")`. Doing exactly that fails with
`No such file or directory (os error 2)`. The documented escape hatch cannot be used,
so an out-of-project edit has no working path through `edit_file` at all.

Notably `run_command`'s ack works — `run_command("@ack_e131cd0d")` executed a held
`rm -rf` correctly in the same session. So this is specific to `edit_file`'s
out-of-scope-write ack, not to ack handles in general, which is what makes the hint
credible enough to follow twice.

## Symptom (Effect)

Measured 2026-08-08, `experiments` @ `8f724171`, MCP over stdio:

```
edit_file(file_path="/home/marius/.config/librarian/workspace.toml", old_string=…, new_string=…)
→ { "pending_ack": "@ack_e143ffbb",
    "reason": "'/home/marius/.config/librarian/workspace.toml' is outside the project root",
    "hint": "edit_file(path=\"@ack_e143ffbb\") to write it and approve /home/marius/.config/librarian for this session" }

edit_file(file_path="@ack_e143ffbb", old_string=…, new_string=…)
→ No such file or directory (os error 2)
```

Retried once with a fresh handle (`@ack_e1443a3c`, also with `replace_all=true`) —
same result. An intermediate attempt passing only `file_path` returned
`missing 'old_string' parameter`, which shows the handle IS reaching parameter
validation; it fails later, at path resolution.

## Reproduction

```
edit_file(file_path="<absolute path outside the active project>",
          old_string="<something present>", new_string="<anything>")
# note the @ack_ handle in the response, then:
edit_file(file_path="@ack_<handle>", old_string="<same>", new_string="<same>")
```

## Environment

Linux, codescout `experiments` @ `8f724171`, release binary built from `9cbe4002`.
Target file was a user config outside any project root
(`~/.config/librarian/workspace.toml`).

## Root cause

Unknown — not investigated. The observation only localises it: parameter validation
sees the handle (it complains about a missing `old_string` first), and the failure is
a filesystem `ENOENT`, so the handle is most likely being passed through to a path
open rather than being looked up in the ack store the way `run_command` looks up its
own. `src/tools/core/write_ack.rs` is the place to start.

**Inferred from two observed responses — not measured against the code.**

## Evidence

The `missing 'old_string' parameter` response is the useful one: it proves the call
was dispatched with the handle as `file_path` and got past argument checking. A
handle rejected up front would not have produced a parameter-specific complaint.

## Hypotheses tried

1. **Hypothesis:** the handle needs the other parameters resent alongside it.
   **Test:** resent `old_string` + `new_string` + `replace_all=true` with the handle.
   **Verdict:** rejected — same `ENOENT`.
2. **Hypothesis:** the first handle had expired.
   **Test:** triggered a fresh handle and used it immediately.
   **Verdict:** rejected — same `ENOENT`.

## Fix

Not investigated. Either resolve `@ack_*` to the stored target path in `edit_file`'s
path handling the way `run_command` does, or — if out-of-project writes through
`edit_file` are meant to be refused outright — change the hint, because a hint that
names an unusable call is worse than a plain refusal. It cost two extra round-trips
here and would cost more to anyone who trusts it.

## Tests added

None. Wanted: a test that takes the `pending_ack` handle from an out-of-project
`edit_file` and feeds it straight back, asserting the write succeeds. The absence of
that test is why the hint and the behaviour could drift apart — nothing exercises the
documented recovery path end to end.

## Workarounds

Use `run_command` with a bounded editor (`sed -i` on explicit line numbers, after
`cp` to a `.bak-<date>` and an `awk` printout to confirm the target lines). That is
what was done for the config edit this was found on; see `927d75c0`'s sibling work.

## Resume

Read `src/tools/core/write_ack.rs` and compare how `run_command` resolves an `@ack_*`
handle with how `edit_file` resolves `file_path`. Decide between resolving the handle
and dropping the hint. Low priority — a workaround exists and out-of-project edits
are rare.

## References

- `src/tools/core/write_ack.rs` — ack storage/lookup
- `docs/issues/archive/2026-08-08-memory-dir-for-project-materializes-any-id.md` — the work
  this was found during
