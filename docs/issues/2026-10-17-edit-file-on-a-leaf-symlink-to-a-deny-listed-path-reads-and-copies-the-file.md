---
status: fixed
opened: 2026-10-17
closed: 2026-10-17
severity: high
owner: marius
related: [docs/issues/2026-10-17-write-allowlist-canonicalizes-only-the-parent-so-a-leaf-symlink-or-a-missing-subdir-escapes.md, docs/issues/2026-10-17-doc-update-writes-through-a-catalogued-symlink-and-rewrites-a-file-outside-the-root.md]
tags:
- cluster/unclassified
kind: bug
unverified: 'edit_code rename: the new check in the rename plan is unit-tested at validate_leaf_link_target but its call in do_rename is not integration-tested (no mock-LSP rename harness) and survives a mutation. Linux only for the hermetic /etc/shadow tests; macOS and Windows not run. The fix runs the target through the existing validators at the two write resolvers; it does not change path_security.rs, whose classifier still answers "allowed" for <root>/leaf -> ~/.ssh/id_rsa.'
---

# BUG: `edit_file` on an in-root leaf symlink to a deny-listed path reads the file the read tools refuse, echoes it in its not-found error, and can copy it into the project

## Summary

The write classifier checks the deny list against "resolved parent + leaf as written"
(`src/util/path_security.rs`, `classify_write_path_in`), so `<root>/leaf -> ~/.ssh/id_rsa` classifies as an allowed in-root
write. `edit_file` then read the file through the link (`read_edit_target`, `src/tools/edit_file/mod.rs:789`) although
`read_file` refuses the same link. Measured: a near-miss multi-line `old_string` returned the file's lines inside the
error, and a unique match succeeded, which replaces the link with a regular in-project copy of the secret.

## Class fit

This is the leaf-symlink shape (shape 1) of the write-allowlist issue, but with the READ side as the escape: the
classifier answers correctly for the spelling it is given and the deny list is applied to that spelling, not to
what the OS resolves. Filed unclassified for the same reason as that issue: no class names a containment check
that resolves the parent and trusts the leaf. It is separate because the consequence is disclosure of a
deny-listed file, not an out-of-root write (which `atomic_write` prevents by renaming over the link).

## Symptom (Effect)

With `leaf.txt` a symlink inside the project to a deny-listed file, `edit_file` on `leaf.txt`:

- with an `old_string` that nearly matches over several lines, fails with a message that contains the file's lines
  (the not-found "nearest content" hint);
- with a unique `old_string`, succeeds, the victim is untouched, and `leaf.txt` becomes a regular file holding the
  victim's content with the edit applied. The copy is an ordinary in-project file, so `read_file` then reads it.

## Reproduction

Measured 2026-10-17 on `worktree-agent-a26646b8b970ff11a` (throwaway test, fake `$HOME`, fake key). Re-measured on
`worktree-agent-a66600865e063ffd2` by the permanent test, with the new check neutralised (the unfixed state), whose child
process records what it saw. The test re-executes its own binary with `HOME` set for the child only, so no process
environment is mutated (`docs/conventions/test-env-isolation.md`); the key is `fake/home/.ssh/id_rsa` holding three
marker lines:

```
{"read_file_refused":true,"key_readable":true,"near_miss_error_leaks":true,
 "unique_match_succeeded":true,"leaf_is_still_a_link":false,"no_copy_in_project":false}
```

That is the issue as measured, now by `edit_file_on_a_leaf_link_to_a_readable_deny_listed_file_leaks_and_copies_nothing`:
`read_file` refuses the link, the near-miss error leaks the key's lines, the unique match succeeds and the link is replaced
by an in-project copy. (The exact text of the leaking error is still asserted by `contains`, not printed, to avoid dumping the
fake key into logs; that it comes from the not-found hint is inferred from the message shape.)

## Environment

Linux; fake `$HOME` for the child process only.

## Root cause

`path_security.rs` `classify_write_path_in` compares deny entries with "resolved parent + leaf as written"; for `<root>/leaf`
that is `<root>/leaf`, which is allowed. `read_edit_target` reads the path (following the link) before any further check.
`read_file` refuses because `validate_read_path` canonicalizes the full path.

## Evidence

Output above, plus the mutation runs under Tests added. `read_edit_target` is the single read for all three `edit_file`
grammars and `perform_edit` (batch `edits[]`, `insert`, single `old_string`); `edit_markdown` and `create_file` share the
resolver; `edit_code` symbol edits use a second resolver (`resolve_write_path_for`).

## Hypotheses tried

1. **Hypothesis:** fix it in `path_security.rs` (classify the resolved leaf too). **Verdict:** deferred by the brief (that
   file is being edited by another session). The same effect is reachable from outside it, below.
2. **Hypothesis:** refuse every edit through a leaf link. **Verdict:** rejected: a link to a harmless file is edited today by
   REPLACING the link with the edited copy and sparing the victim (pinned by
   `edit_file_on_a_leaf_link_to_a_harmless_file_still_replaces_the_link_and_spares_it`), which is not an escape, so refusing it
   is a behaviour change with no security payoff.
3. **Hypothesis:** the read half is the only exposure for `atomic_write` tools, the write half for raw `fs::write`. **Verdict:**
   confirmed by reading `atomic_write` (rename over the link) and `edit_code`'s rename plan (raw `fs::write`).

## Fix

Branch `worktree-agent-a66600865e063ffd2`, commit `38a492b4`, patch-id `79c3ad0ae83b55383b42cdbee092a2f273f3b7ee`.

`src/fs/mod.rs` `validate_leaf_link_target(resolved, root, security, session_roots, policy)`: when the write target is a leaf
symlink, run what it RESOLVES to through the existing public validators, with two policies:

- `LinkTarget::ReadableByTools` for tools whose write replaces the link (`atomic_write`): `validate_read_path(target)`, i.e.
  the same deny-list as `read_file`. A link to a harmless file, inside or outside the project, is untouched by this.
- `LinkTarget::Writable` for a raw `fs::write` that follows the link (`edit_code`'s multi-file rename plan):
  `validate_write_path(target)`, so the target must be somewhere a write could be approved; a dangling link is refused (a write
  would create its target somewhere unchecked).

Wired into the two resolvers every edit tool shares: `write_ack::resolve_write_or_capture` (`edit_file` in all grammars,
`edit_markdown`, `create_file`) and `fs::resolve_write_path_for` (`edit_code` symbol edits), and into `edit_code`'s rename plan.
The check follows the classifier's `Allowed`; the `OutsideRoot`/approve flow is unchanged, and an approved replay passes through
the same resolver, so it is checked then.

**Behaviour change per tool (exact):** `edit_file` (single, batch, insert), `edit_markdown` (heading grammar), `edit_code`
symbol edits and `create_file(overwrite=true)` on a path that is a leaf symlink whose resolved target is deny-listed now fail
with `access denied: '<target>' is in a protected location — reached through the symbolic link '<link>'` before any read or
write, as a recoverable error in the two write-ack tools and as the validator's error from `resolve_write_path_for`. Nothing else
changes: ordinary files, links to harmless files, and dangling links under `ReadableByTools` behave as before. `edit_code`
rename now refuses (rather than writes through) an in-root link whose target is outside the write roots or dangling.

No change to `src/util/path_security.rs` was needed. **If the other session wants the central fix**, the equivalent change
there is: in `classify_write_path_in`, after the deny-list check on the resolved path, also test
`canonicalize(resolved)` when `resolved`'s leaf is a symlink against `denied` (both resolver forms it already builds), and return
`Denied`; `validate_leaf_link_target`'s `ReadableByTools` branch then becomes redundant for the deny-list and can be deleted, its
`Writable` branch stays (it asks about roots).

## Tests added

`src/tools/edit_file/tests.rs`: `edit_tools_refuse_a_leaf_link_to_a_deny_listed_file_on_every_grammar` (Linux, `/etc/shadow`,
single / batch / insert / heading-addressed markdown and `create_file`; asserts the deny-list message, because against the unfixed
code a non-root read fails with `Permission denied`, an error but not this refusal),
`edit_file_on_a_leaf_link_to_a_readable_deny_listed_file_leaks_and_copies_nothing` (fake-`$HOME` child process, the measured
probe), `edit_file_on_a_leaf_link_to_a_harmless_file_still_replaces_the_link_and_spares_it` (control). `src/fs/mod.rs` `tests`:
`a_leaf_link_to_a_deny_listed_file_is_refused_under_both_policies`, `a_leaf_link_check_leaves_ordinary_targets_alone` (control),
`writable_policy_refuses_a_link_that_leaves_the_write_roots_and_a_dangling_one`,
`resolve_write_path_for_refuses_a_leaf_link_to_a_deny_listed_file`.

Mutation probe, one run per guard site: predicate never fires KILLED (5 tests); policy forced to read KILLED; policy forced to
write KILLED; dangling-under-read refused KILLED (by the control); `resolve_write_path_for` unwired KILLED; read validator skipped
KILLED (4 tests); `write_ack` unwired KILLED (both edit_file tests); **`edit_code` rename plan unwired SURVIVED** (the wiring is not
integration-tested, see `unverified`).

## Workarounds

Do not keep symlinks to credential files inside a project root.

## Resume

N/A. Candidate follow-ups: a mock-LSP rename test so the `edit_code` rename wiring is killed by a mutation; the central
`path_security.rs` change described above.

## References

- `docs/issues/2026-10-17-write-allowlist-canonicalizes-only-the-parent-so-a-leaf-symlink-or-a-missing-subdir-escapes.md`
- `docs/issues/2026-10-17-doc-update-writes-through-a-catalogued-symlink-and-rewrites-a-file-outside-the-root.md` (site table)
