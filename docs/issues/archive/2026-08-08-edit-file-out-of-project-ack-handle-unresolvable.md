---
id: a99388a299352d21
kind: bug
status: fixed
title: edit_file's out-of-project ack handle does not resolve — the hint it prints cannot be followed
owners:
- marius
tags:
- tooling
- write-guard
- ack
- misleading-error
closed: 2026-08-14
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

**The original diagnosis is falsified. The ack handle resolves correctly; it always did.**

Measured 2026-08-14 against the running MCP server (post `cargo rb` + `/mcp` reconnect),
both call shapes, on a fresh out-of-project directory:

| Call | Result |
|---|---|
| `edit_file(path="@ack_003a6918")` — the hint's exact form | `"ok"`, and `hello world` → `HELLO WORLD` landed on disk |
| `edit_file(path="@ack_003b6339", old_string=…, new_string=…)` — **this file's form** | `"ok"`, and the edit landed |

The handle carries the whole pending operation, so re-supplying the strings is optional
and harmless either way.

### The ENOENT was the target file, and it reproduces on demand

```
edit_file(path="/home/marius/.local/state/cs-ack-nonexistent/missing.toml", old_string=…)
  → pending_ack @ack_003c0d18
edit_file(path="@ack_003c0d18", old_string=…, new_string=…)
  → No such file or directory (os error 2)
```

Byte-identical to the symptom recorded above — produced by a **file that does not
exist**, with the ack handle resolving perfectly. The observation was real; the
attribution was not.

### Why the fix could not have been the cause

- `151cc9df feat(edit_file): out-of-scope write returns ack handle at all resolve sites`
  is dated **2026-06-27** and `git merge-base --is-ancestor 151cc9df 8f724171` → **true**:
  the mechanism this file calls broken was already present in the exact SHA the file
  cites.
- `git log --since=2026-08-07 -- src/tools/core/write_ack.rs src/tools/edit_file/ src/fs/`
  → **empty**. Nothing changed in this area between the filing and now, so "it must have
  been fixed since" is not available either.

The file's own evidence pointed here and was read the other way: *"passing only
`file_path` returned `missing 'old_string' parameter`, which shows the handle IS reaching
parameter validation."* The handle was resolving. That sentence is the refutation,
recorded under *Symptom* and then argued past.

One detail worth knowing for anyone re-testing: the ack **approves the whole directory
for the session**. A second attempt in the same directory does not produce a
`pending_ack` at all, so a fresh directory is required per probe — which is a plausible
way the original session's retries produced confusing results.
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

**The reported defect needed no fix. The defect that produced the misdiagnosis did, and
that shipped 2026-08-14 on `experiments`.**

All three `std::fs::read_to_string(&resolved)?` sites in `src/tools/edit_file/mod.rs`
now route through `read_edit_target`, which:

- **names the path** in every failure, ENOENT or otherwise;
- classifies a missing file as `RecoverableError` with a hint — correct per the
  repair-and-continue convention, since a wrong path is caller-fixable;
- **states explicitly that the `@ack_*` handle resolved correctly**, so the reading this
  bug arrived at is no longer the reasonable one.

The old message was `No such file or directory (os error 2)` — no path, no stage. After
passing a handle and getting that back, "the handle didn't resolve" is the obvious
inference: the handle is the thing you just typed, and the error names nothing else. The
report cost two extra round-trips and a bug file, and the message is why.

The alternative the original *Fix* offered — "change the hint, because a hint that names
an unusable call is worse than a plain refusal" — is moot: the hint names a call that
works. Verified twice above.
## Tests added

Two, in `src/tools/edit_file/tests.rs`:

- **`a_missing_edit_target_names_the_path_and_absolves_the_ack_handle`** — asserts the
  error names the file, is a `RecoverableError`, and mentions `@ack_` so the handle is
  explicitly cleared.
- **`a_non_enoent_read_failure_is_not_reported_as_a_missing_file`** — reads a *directory*
  (`IsADirectory`, not `NotFound`), asserting the path is still named and the message is
  **not** relabelled "no file to edit at". Guards against collapsing every io error into
  the friendly case.

**Mutation-verified, and the pair discriminates.** Forcing the `NotFound` branch to
`false`:

```
a_missing_edit_target_names_the_path_and_absolves_the_ack_handle ... FAILED
  a missing path is caller-fixable, so it belongs in RecoverableError;
  got: reading /tmp/.tmpbWKy2X/nested/absent.toml to edit it
a_non_enoent_read_failure_is_not_reported_as_a_missing_file      ... ok
```

The first caught it, the second correctly did not — and note the mutated message still
named the path, so only the *classification* assertion fired. The two assertions cover
different properties rather than restating one.

Gate: **3720 passed / 0 failed / 44 ignored** (3718 + these 2), `clippy --workspace
--all-targets -D warnings` clean.
## Workarounds

Use `run_command` with a bounded editor (`sed -i` on explicit line numbers, after
`cp` to a `.bak-<date>` and an `awk` printout to confirm the target lines). That is
what was done for the config edit this was found on; see `927d75c0`'s sibling work.

## Resume

N/A. Do **not** re-open to "fix the ack handle" — it works, measured in both call shapes
on 2026-08-14, and the code has not changed in that area since before this file was
written.

If `No such file or directory` shows up again after passing an `@ack_*` handle, the
message now tells you which file and that the handle is fine. Check whether the target
exists.
## References

- `src/tools/core/write_ack.rs` — ack storage/lookup
- `docs/issues/archive/2026-08-08-memory-dir-for-project-materializes-any-id.md` — the work
  this was found during
