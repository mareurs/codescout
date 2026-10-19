---
status: fixed
opened: 2026-10-18
closed: 2026-10-18
severity: high
owner: marius
related: [docs/issues/2026-10-18-opening-the-private-memory-store-copies-a-gitignore-symlink-targets-bytes-into-the-project.md, docs/issues/2026-10-14-pi-memory-write-guard-reencodes-invalid-utf8-before-backup.md]
tags:
- cluster/unclassified
unverified: 'Windows behaviour was not run. The walk is symlink_metadata per component; the final read now opens with O_NOFOLLOW and checks the descriptor (unix only), but a PARENT directory swapped for a link after the walk is not closed. OPEN DECISION, not changed: a store ROOT that is a link serves every *.md under its target (see Open decisions). anchors::hash_file still reads project paths named inside a sidecar and was not changed: it reveals a SHA-256, not content. The SQLite semantic memory database file was not examined for a leaf link. The onboarding protocol in src/prompts/source.md has no branch for the new unreadable state. The fix SHAs are on the review branch and may not survive a rebase: use the patch-ids.'
kind: bug
---

# BUG: `MemoryStore::read` follows a leaf (or parent) symlink, so `memory(action="read")` serves any file a memory topic links to

## Summary

`MemoryStore::read` (`src/memory/mod.rs`, 129-136 before the fix) gated on `path.exists()` and returned
`read_to_string(path)`, both following links. `WriteTransaction` already refuses every non-regular topic
(`src/memory/write_transaction.rs:68`, "memory target must be a regular file") and every linked parent
(`safe_topic_parent`), so the read side was the only direction left open. A repository can ship
`.codescout/memories/arch.md -> ~/.ssh/id_rsa` (git carries symlinks); `memory(action="read", topic="arch")`
then returns the key. The same holds for the private store, the dashboard `get_memory` handler and the
onboarding reader, which all call `read`.

## Class fit

The write path has the guard and the read path lacks it for the same shape of input, which is close to a
guard narrower than the operation it protects, but no existing class names a pair of directions where only
one is guarded. Filed unclassified rather than forced into a class whose trait it shows only in part.

## Symptom (Effect)

Reviewer evidence 2026-10-18: `read("arch")` on `arch.md -> home/.ssh/id_rsa` returned `Ok(Some("PRIVATE"))`.
Through the tool dispatch, before the fix: `Object {"content": String("PRIVATE")}`.

## Reproduction

Measured 2026-10-18 on branch `fix/memory-leaf-links-2026-10-18` at `0a8af94d7`, before the fix, by new tests
under `tempdir()` (no real home). Red lines (12 failing of the 16 tests added by this fix and run in the `memory::` filter;
the other four pin behaviour that was already correct):

```
memory::tests::read_refuses_a_leaf_symlink_to_a_file_inside_the_store: must be refused: Some("inside")
tools::memory::tests::read_refuses_a_topic_that_is_a_symlink_to_a_secret: a linked topic must not be served: Object {"content": String("PRIVATE")}
memory::tests::read_refuses_a_directory_at_the_topic_path: must be a RecoverableError, got: Is a directory (os error 21)
memory::anchors::tests::read_anchor_file_refuses_a_symlinked_sidecar: must be a RecoverableError, got: TOML parse error at line 1, column 16 | 1 | SECRET-BYTES = not toml
test result: FAILED. 197 passed; 12 failed; 0 ignored; 0 measured; 7218 filtered out
```

The anchor line shows a second leak: a linked `<topic>.anchors.toml` that is not TOML produces a parse error
that quotes the target's line.

## Environment

Linux, `rustc 1.97.1`, branch above.

## Root cause

`src/memory/mod.rs` `MemoryStore::read` (pre-fix 129-136): `path.exists()` follows the leaf link, and
`std::fs::read_to_string` follows it again. Nothing inspected the final component or any parent with
`symlink_metadata`. `src/memory/anchors.rs` `read_anchor_file` had the same shape (`read_to_string` of a
sidecar path), and a link there is parsed as the topic's anchor set. Measured 2026-10-18: red lines above.

## Evidence

Who reads a memory-store path, and whether each follows a leaf link (read from source at `0a8af94d7`; "run" means a
test drove it; the review of 2026-10-18 found rows 4, 5 and 11 missing from the first version of this table):

| # | Reader | Followed a leaf link at `0a8af94d7` | Now |
|---|---|---|---|
| 1 | `MemoryStore::read` via `memory(action=read)`, shared and private (run) | yes | refused (`RecoverableError`) |
| 2 | `MemoryStore::read` below a linked parent directory (run) | yes | refused |
| 3 | `read_anchor_file` (run); a non-TOML target also leaked a line through the parse error | yes | refused |
| 4 | MCP `memory://` resources, `src/mcp_resources/memory.rs`: `resources/list` listed link, dangling-link and directory entries, `resources/read` served the target (run through `ResourceRegistry`) | yes, and listed | not listed, read re-checks the type |
| 5 | `.codescout/system-prompt.md` into the `initialize` instructions, `src/agent/mod.rs` `project_status` (run) | yes | skipped with a warning; see the sibling issue |
| 6 | dashboard `get_memory`, onboarding `memory.read`, `migrate/memories.rs` re-embed (read from callers; all call `MemoryStore::read`) | yes | refused through row 1 |
| 7 | onboarding `gather_protected_memory_state`: mapped a refusal to `exists:false`, which the protocol reads as "create fresh" (run) | n/a | `unreadable:true` plus `error`, no `exists` key |
| 8 | `MemoryStore::list` (run, pinned by `list_does_not_report_or_enter_symlinks`) | no (walkdir without `follow_links`) | unchanged |
| 9 | `anchors::check_all_memories`: the walk does not follow, but the sidecar read did, and one refused sidecar returned `Err`, which `workspace status` swallows, dropping the WHOLE `memory_staleness` section | sidecar read: yes; walk: no | a refused sidecar is listed under `unreadable` and the other topics are still checked (run) |
| 10 | `MemoryStore::write`, `delete` (`WriteTransaction`, `write_transaction.rs:68`, `safe_topic_parent`) | no | unchanged; these use `bail!` (`isError: true`) where reads use `RecoverableError`, a documented difference |
| 11 | A store ROOT that is a link (`.codescout/memories -> <elsewhere>`): every read and list resolves through it | yes, by design on the write side (`docs/conventions/memory-write-history.md`) | unchanged; see Open decisions |
| 12 | `write_anchor_file` (`atomic_write`) | does not write through: the rename replaces the entry | unchanged |
| 13 | `anchors::hash_file` on project paths named in a sidecar | yes, but it yields only a SHA-256 | unchanged |
| 14 | the semantic memory SQLite database file | not examined | not examined |

0 symlinks are tracked under `.codescout/` (`git ls-files -s .codescout`, mode 120000, 2026-10-18); the only tracked
symlink in the repository is `.claude/skills/claude-traces`. No production code creates a memory symlink.

## Hypotheses tried

1. **Hypothesis:** a legitimate flow symlinks topic files to a file inside the project. **Test:**
   `git log` of `docs/conventions/memory-write-history.md`, `grep -i symlink` over `docs`, `src/memory`,
   `src/tools/memory`, `src/tools/onboarding.rs` and `tests/memory_write_history.rs`. **Verdict:** rejected for
   anything shipped; the only symlink handling there is refusal. Every leaf link is therefore refused,
   including one landing inside the store, to match `WriteTransaction`.

## Fix

Fixed on branch `fix/memory-leaf-links-2026-10-18` by `732178af8` (patch-id
`7113e260a6ec1d52b12efb5a160ed54c5cca292b`, `git show 732178af8 | git patch-id --stable`).

`MemoryStore::read` walks the topic path below the store root with `symlink_metadata`. A missing component is
`Ok(None)` as before. A final component that is not a regular file returns `RecoverableError`
"memory target must be a regular file: <path>"; an intermediate component that is not a directory returns
"memory parent must be a real directory: <path>". The store root itself may still be a link. `read_anchor_file`
refuses a non-regular sidecar the same way ("anchor sidecar must be a regular file"). Behaviour changes: a dangling
leaf link is a refusal instead of "no such topic"; an unreadable component (EACCES) is now an error instead of
`None`; a directory at the topic path is a `RecoverableError` instead of a raw io error.


**Second round, after the review of `a0a41e431`** (`aa934e25c`, patch-id `ba23f9d56125d799dd3b8a592d445a4af46c686b`;
`a3b9eca5c`, patch-id `c4d0c21573de1bacfc806ebc876ef76e9c2cc460`):

- `src/mcp_resources/memory.rs`: `entries()` lists only entries whose `DirEntry::file_type()` (which does not follow
  a link) is a regular file; `read_path_checked` re-checks with `symlink_metadata` at the read. It does not call
  `MemoryStore::read`, because `a.b.md` has stem `a.b` and `topic_path` would map that to `a.md`.
- `check_all_memories`: a refused sidecar goes to `unreadable` (`{topic, reason}`, key present only when
  non-empty) and the walk continues; a dangling sidecar link is `unreadable`, not `untracked`.
- `MemoryStore::read`: a component that cannot be inspected (EACCES, ENAMETOOLONG) is a `RecoverableError`
  naming the path (the base answered "not found"); a plain file where a directory belongs says it is not a
  directory instead of telling the user to replace a link; the final read opens with `O_NOFOLLOW | O_NONBLOCK` and
  checks the descriptor (`read_regular_file`). A swapped-in PARENT link is still not closed.
- onboarding: an unreadable protected topic is `{"unreadable": true, "error": ...}` with no `exists` key. The
  onboarding protocol in `src/prompts/source.md` has no branch for it and was NOT edited (another session owns
  `src/prompts`; `ONBOARDING_VERSION` is bumped only for the stored system-prompt surfaces, which are unchanged),
  so a reader of that protocol sees neither the create-fresh nor the merge case. See Open decisions.

## Tests added

`src/memory/mod.rs`: `read_refuses_a_leaf_symlink_to_a_file_outside_the_store`,
`read_refuses_a_leaf_symlink_to_a_file_inside_the_store`, `read_refuses_a_dangling_leaf_symlink`,
`read_refuses_a_directory_at_the_topic_path`, `read_refuses_a_topic_below_a_symlinked_directory`,
`read_through_a_symlinked_store_root_still_works`, `read_of_a_missing_nested_topic_is_none`,
`list_does_not_report_or_enter_symlinks`; `src/memory/anchors.rs`: `read_anchor_file_refuses_a_symlinked_sidecar`;
`src/tools/memory/tests.rs`: `read_refuses_a_topic_that_is_a_symlink_to_a_secret` (shared and private store through
the tool dispatch). Mutation: six guarded sites mutated one at a time, all killed (table in the session report).


Second round: `mcp_resources::memory::tests::the_registry_neither_lists_nor_serves_a_linked_or_non_file_topic`,
`read_refuses_an_entry_swapped_for_a_link_after_it_was_listed`;
`memory::anchors::tests::a_linked_sidecar_is_reported_unreadable_and_the_other_topics_are_still_checked`,
`a_dangling_sidecar_link_is_unreadable_not_untracked`;
`tools::config::tests::project_status_keeps_memory_staleness_when_one_sidecar_is_a_link`;
`memory::tests::read_below_a_plain_file_says_it_is_not_a_directory`,
`read_of_an_over_long_name_is_a_recoverable_error_naming_the_component`,
`read_below_an_unsearchable_directory_is_a_recoverable_error_naming_it`,
`the_final_read_refuses_a_link_and_a_directory_even_without_the_walk`;
`tools::onboarding::tests::a_protected_topic_that_cannot_be_read_is_unreadable_not_missing`. One mutant is
behaviourally equivalent and survives: widening the walk's final-component arm to `!is_dir`, because
`read_regular_file` refuses the same shapes with the same message (defence in depth).

## Workarounds

Do not read memory from a repository you did not author until on the fix; inspect `.codescout/memories` for links.

## Open decisions

**A store root that is a link serves every `*.md` under its target.** `.codescout/memories -> /somewhere` is
documented design for the WRITE side, but it is also a read exposure: a cloned repository can ship that link (git
carries symlinks), and `memory(action=read)`, `memory(action=list)`, the `memory://` resources and onboarding then
serve every `.md` file under the target directory. This is not changed here. Two options for the user:

1. Refuse a project-local store root that resolves outside the project root (a configured store outside the
   project, via `Workspace::memory_dir_for_project`, would be exempt).
2. Require an explicit opt-in (a `project.toml` key) before a store root that is a link is used.

The onboarding protocol has no branch for `unreadable`; wording one belongs to whoever owns `src/prompts/source.md`.

## Resume

N/A once fixed. Open residuals: see `unverified` above.

## References

`src/memory/mod.rs`, `src/memory/anchors.rs`, `src/memory/write_transaction.rs`,
`docs/conventions/memory-write-history.md`, the sibling `.gitignore` issue.
