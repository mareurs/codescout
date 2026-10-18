---
status: open
opened: 2026-10-17
closed:
severity: medium
owner: marius
related: [docs/issues/2026-10-16-write-allowlist-holds-the-temp-dir-uncanonicalized.md]
tags:
- cluster/unclassified
kind: bug
---

# BUG: `classify_write_path` canonicalizes only the immediate parent, so a symlink leaf or a symlinked directory with a missing subdirectory below it is classified as inside the root

## Summary

`canonicalize_write_target` (`src/util/path_security.rs`) resolved symlinks in the target's
immediate parent only, then joined the file name. Two shapes therefore kept their literal,
in-root spelling while the OS would resolve them outside the root. Measured at the classifier, and
for the second shape at a write tool (it wrote outside the root). **Shape 2 is fixed (`2c71a473`).
Shape 1, a leaf that is itself a symlink, was classified in-root; as of 2026-10-18 `classify_write_path_in`
follows the leaf (`resolve_leaf_link`) and asks the deny list and the roots of the link AND of where it lands
(fix `0b0ab92f4`, see Fix and Tests added). The write tools that reach `atomic_write` (`edit_file`,
`create_file`, `edit_code`) never wrote through a leaf link. `edit_file` on a link to a deny-listed path
disclosed that file (fixed earlier at the resolvers by `38a492b4`; the classifier now refuses it too). The
librarian's `doc(update)` and `doc(create)` also followed a link, but they consult no write classifier, so
fixing the classifier fixed neither; they were separate issues, both now `fixed` (see Resume). The issue stays
open, at medium, for what this session did not run: see the 2026-10-18 status in Resume.**

## Class fit

No existing class names a containment check that resolves one path component and trusts the
rest. Filed unclassified because it was found while fixing the temp-dir root
(`2026-10-16-write-allowlist-holds-the-temp-dir-uncanonicalized`) and is a different fault in a
different function; the root being compared against is not involved, so that bug's class does not
carry over. Revisit if a second containment check shows the same one-component shape.

## Symptom (Effect)

Both are classified `Allowed`, with the in-root spelling as the resolved path:

1. A symlink leaf inside an allowed root pointing at a file outside it: `<root>/leaf -> /elsewhere/victim.txt`;
   target `<root>/leaf`. The parent `<root>` canonicalizes, the name `leaf` is joined unresolved.
2. A symlinked directory inside an allowed root, then a path through a directory that does not exist
   yet: `<root>/esc -> /elsewhere`, target `<root>/esc/new/x.txt`. The parent `<root>/esc/new` does
   not exist, so `best_effort_canonicalize` returns it unchanged, and it starts with `<root>`.

Measured at the write tools on 2026-10-17, with throwaway links under this worktree and victims
under `$HOME/.cache`, all removed afterwards:

- Shape 2 IS an escape. `create_file(path=".esc-link/newdir/x.txt")`, with `.esc-link` a symlink to a
  directory outside the project, returned `ok` and wrote `<outside>/newdir/x.txt` containing the
  content, creating `newdir` outside the root.
- Shape 1 did NOT escape through `edit_file`: the link was replaced by a regular file and the
  victim kept its content, so that tool's atomic write does not follow a leaf link.
- Shape 1, measured 2026-10-17 on branch `worktree-agent-a26646b8b970ff11a` (see Fix): `create_file` with
  `overwrite` and `edit_code` (replace) against an in-project leaf link to an outside file ALSO do not
  escape. Both reach `atomic_write`, whose rename replaces the link with a regular file; the victim keeps its
  bytes. A DANGLING leaf link did not make `create_file` create the file it names. So shape 1 is not an
  out-of-root WRITE through any tool that routes through `atomic_write`. It is not safe overall: the librarian
  writes with plain `std::fs::write` and the deny list is applied to the spelling, not the resolved path. See
  the three issues under Resume, each measured the same day.

## Reproduction

Scratch test, not committed. Unix. `temp`, `outside`, `project` are three `tempdir()`s:

- `symlink(outside/victim.txt, temp/leaf)`, then
  `classify_write_path_with_temp(temp/leaf, project, default, [], temp)`
- `symlink(outside, temp/esc)`, then
  `classify_write_path_with_temp(temp/esc/new/x.txt, project, default, [], temp)`

## Environment

Linux, codescout worktree branch cut from `experiments` at 299395e5.

## Root cause

`canonicalize_write_target` canonicalized `path.parent()` best-effort and joined `file_name()`.
It did not walk up to the deepest existing ancestor when the parent was missing (shape 2, fixed), and
it does not resolve the leaf if the leaf is itself a symlink (shape 1, open). The allowed roots were a
separate matter and not "the same function": the project root, the cwd, the extra roots and the session
roots went through `best_effort_canonicalize`, which keeps a missing path literal, so a root that did not
exist yet under a symlinked ancestor never matched a target resolved through its existing ancestors
(fixed in `abdb0b03`: every root, and the deny-list entries, now use
`canonicalize_through_existing_ancestor` too).

## Evidence

Measured 2026-10-17 with the scratch test above:

```
leaf:            Allowed("<temp>/leaf")
nonexist-subdir: Allowed("<temp>/esc/new/x.txt")
```

## Hypotheses tried

None needed for the classification result. The write-tool consequence was measured for shape 2
(escape) and, for shape 1, through `edit_file`, `create_file`, `edit_code` (no write-through), `doc(update)`
(escape), `doc(create)` on a dangling link (escape) and `edit_file` onto a deny-listed target (disclosure).

## Fix

Shape 2 FIXED in `2c71a473` (a commit on the branch carrying `4915b443`): `canonicalize_write_target` now
resolves the target's parent through its deepest existing ancestor
(`canonicalize_through_existing_ancestor`) and appends the rest verbatim, `..` included. It was
needed by the temp-dir fix too: once the temp root was canonical, a write into a not-yet-created
subdirectory of a symlinked temp dir or project root was refused, which went green -> red against
`299395e5` for `classify_in_project_is_allowed` and `create_text_file_creates_parent_dirs` with a
symlinked TMPDIR.

Shape 1 (a leaf that is itself a symlink), as first measured 2026-10-17 with the classifier NOT changed (fixed 2026-10-18, next section): for the tools that reach `atomic_write`,
measured instead of fixed: they do not write through the leaf (see Symptom), because `atomic_write` renames over
the link. Decision, written into
`atomic_write`'s doc and pinned by tests (`b493599d`, patch-id `4c9425528fab3aae53dac1ce079719843750e93c`, which
also fixes `2026-10-17-atomic-write-follows-a-planted-staging-symlink...`): a target that is a symlink is
replaced, never written through. For those tools the classifier answering `Allowed(<root>/leaf)` is safe only
because of that property of the writer, not because the classifier resolved the link: a writer that opens the
target directly (`OpenOptions::open(path)`, `fs::write(path)`) reopens it, and the librarian is exactly that
(next paragraph). The `symlink_metadata` check in the classifier was then done (2026-10-18, next section); it was needed for the
deny-list case below.

Not every writer routes through `atomic_write`. Measured 2026-10-17 (throwaway tests, removed): `doc(update)` on a
catalogued in-root symlink rewrote the outside file; `doc(create)` on a dangling link created the file outside
the root; `edit_file` on a link to a deny-listed path read it (error text contained the file's lines) and could
copy it into the project. Each is filed separately and NOT fixed here.
`MemoryStore` writes use `tempfile::NamedTempFile::persist` (`src/memory/write_transaction.rs`), not
`atomic_write`; not run against a leaf link.

**Shape 1 FIXED 2026-10-18 at the classifier** (branch `fix/classifier-leaf-2026-10-18`, fix `0b0ab92f4`,
patch-id `e3ec71d2788c1429536362fedec0804cd2b923c2`; red tests `329f78326`, patch-id
`b05bd17bcd430e36414e75f196fe3b1745704bf3`). `classify_write_path_in` now calls `resolve_leaf_link` on the
parent-resolved target. It follows the leaf with `symlink_metadata` + `read_link` (never `metadata`), resolves a relative hop
against the directory of the link that names it, sends every hop back through `canonicalize_write_target` (so a hop that
crosses a symlinked directory, or names a file below a missing directory, i.e. a dangling link, is resolved through the
deepest existing ancestor), and stops after `MAX_LEAF_LINK_HOPS` (40, the kernel's `MAXSYMLINKS`) links. The deny list and the
roots are then asked of BOTH the link's own location and where the chain ends, and the stricter answer wins:

- a chain that ends in the deny list: `Denied` ("protected location"), whatever roots are approved; no ack is offered.
- a chain that ends outside every root and outside the deny list: `OutsideRoot { resolved: <the landing> }`, so the
  `approve_write` directory is the landing's, and after that approval the leaf is `Allowed`. Decision, and its reason: the
  classifier answers for a link exactly what it answers for the file the link resolves to. A link into an approved or
  configured root is as writable as that file; a link out of every root is as refused as that file. A leaf link out of the
  roots used to be writable and is now an `@ack_*` flow, which is the point of an allowlist.
- a link whose own location is outside the roots or in the deny list stays refused for that, even if it points inside: the
  writers (`atomic_write`) overwrite the link itself, so the link's location is what is written. The answer is the
  stricter of the two, never the landing alone (`a_link_spelled_outside_the_roots_stays_refused_when_it_points_inside`).
- a loop, a chain longer than the limit, or a link `read_link` cannot read: `Denied` (a hard denial; `approve_write` cannot
  grant it); a landing with an unresolvable `..`: `Denied`, like a first-hand target.
- `Allowed` still carries the link's own spelling, NOT the landing. The writers replace a link with a regular file
  (`a_target_that_is_a_symlink_is_replaced_not_written_through`); handing them the landing would turn \"replace the link\" into
  \"rewrite the target\". Pinned by `a_leaf_link_to_a_file_inside_the_root_is_allowed_under_the_link_spelling`; the mutant
  that returned the landing was killed by it (and by five other cases).

`fs::validate_leaf_link_target` (the resolver look added by `38a492b4`) is KEPT, unchanged apart from its comments
(`75d829611`, patch-id `81a291995ea95b01f056cb1e5051be030569a270`). It is not shown exactly redundant: its `Writable` policy
refuses a dangling link outright, where the classifier now judges the path the link names, and the `ReadableByTools`
read deny list is not shown to be a subset of the classifier's two-form deny list on every filesystem shape. It also looks
again at the point of use. No line was removed, so no mutant was needed for a removal.

## Tests added

Shape 2, all in `src/util/path_security.rs` (unix):
`symlink_out_of_the_temp_dir_is_refused_even_below_a_missing_subdirectory`,
`symlinked_temp_dir_allows_a_write_below_a_missing_subdirectory`,
`project_root_behind_a_symlink_allows_a_write_below_a_missing_subdirectory`.
Shape 1, characterization (green before and after the atomic_write fix; they pin the property the safety rests on):
`a_target_that_is_a_symlink_is_replaced_not_written_through` (`src/util/fs.rs`, also a dangling leaf),
`create_file_overwrite_on_a_leaf_symlink_replaces_the_link_and_spares_the_victim` (`src/tools/edit_file/tests.rs`,
also a dangling leaf), `edit_code_replace_on_a_leaf_symlink_replaces_the_link_and_spares_the_victim`
(`src/tools/symbol/tests.rs`). A mutation replacing `rename` with `copy` in `atomic_write` (which writes through
the link) killed all three (mutation-probe, 7 tests red in total).

Shape 1, the classifier fix (2026-10-18), all green after `0b0ab92f4`; `LeafTree` gives each case its own canonical tree with the
temp dir, home and cwd injected, and every case calls `classify_write_path_in` with a `WriteEnv`. Red before the fix (11 of
the 14 `a_*` cases; the other three are characterisations that must not change), in `src/util/path_security.rs`:
`a_leaf_link_to_a_deny_listed_file_is_refused_whatever_roots_are_approved`,
`a_leaf_link_into_the_deny_list_through_a_symlinked_directory_is_refused`,
`a_leaf_link_to_a_file_outside_every_root_is_outside_root_at_its_target`,
`a_dangling_leaf_link_is_judged_at_the_path_it_names`, `a_leaf_link_chain_is_followed_to_its_end`,
`a_relative_hop_resolves_against_the_directory_of_the_link_that_names_it`,
`a_leaf_link_loop_is_refused_and_does_not_hang` (runs on a thread with a 20 s deadline),
`a_leaf_link_chain_is_followed_for_exactly_the_hop_limit`,
`a_leaf_link_whose_target_sits_under_a_symlinked_directory_resolves_through_it`,
`a_leaf_link_into_an_approved_root_is_allowed_and_only_that_root` (session root and `extra_write_roots`),
`a_leaf_link_whose_target_has_an_unresolvable_dotdot_is_refused`. Characterisations, green before and after:
`a_leaf_link_to_a_file_inside_the_root_is_allowed_under_the_link_spelling`,
`a_link_spelled_outside_the_roots_stays_refused_when_it_points_inside`, `a_leaf_that_is_not_a_link_is_classified_as_before`,
`the_root_profile_still_classifies_a_leaf_link_as_itself`. In `src/tools/core/write_ack.rs` (ambient environment; the outside
target is a path under `/var` that is never created, since the temp dir is itself a write root):
`a_leaf_link_to_an_unapproved_outside_path_captures_an_ack_for_the_targets_directory` and
`a_leaf_link_into_an_approved_directory_is_written_after_the_ack_is_replayed` (red before), and
`a_leaf_link_to_a_deny_listed_path_is_refused_and_offers_no_ack` (a regression pin: the resolver already refused it when
`/etc/gshadow` exists, so it was green before). In `src/fs/mod.rs`:
`resolve_write_path_for_refuses_a_leaf_link_out_of_the_write_roots` (red before).
Mutation record (`scripts/mutation-probe.sh`, isolated worktree, one mutant per guarded site): every guard in
`resolve_leaf_link` and in the three new checks of `classify_write_path_in` was killed. Two sites survived and are
unreachable without a race, not untested: the `read_link` failure arm of `resolve_leaf_link` (the `lstat` just said
"link") and its `cur.parent()` `None` arm (a link always has a parent).

| Mutant (one per run, isolated worktree) | Killed by |
|---|---|
| `resolve_leaf_link`: `is_link = false` | the 11 red cases |
| `symlink_metadata` -> `metadata` | the same 11 |
| `hops == MAX` -> `hops > MAX`; -> `hops + 1 == MAX` | `a_leaf_link_chain_is_followed_for_exactly_the_hop_limit` (each) |
| delete `hops += 1` | the hop-limit case and `a_leaf_link_loop_is_refused_and_does_not_hang` (20 s deadline) |
| `MAX_LEAF_LINK_HOPS` 40 -> 39; 40 -> 2 | the hop-limit case (each; the literal-40 assertion kills 39) |
| relative hop taken as written (`Some(_) => link`) | 7 cases |
| relative hop joined to the FIRST link's directory (`target.parent()`) | `a_relative_hop_resolves_against_the_directory_of_the_link_that_names_it` |
| `cur = next` (no re-canonicalization of a hop) | 4 cases (symlinked directory in the target, deny-list through one, relative hop, outside root) |
| `Err(why)` treated as the link's own spelling | loop and hop-limit cases |
| `has_parent_dir(&landing)` -> `false` | `a_leaf_link_whose_target_has_an_unresolvable_dotdot_is_refused` |
| `is_denied(&landing, ..)` -> `false` | 3 cases (deny-listed file, through a symlinked directory, chain) |
| `!admitted(&landing)` -> `false`; `OutsideRoot { resolved: landing }` -> `{ resolved }` | 6 cases each |
| `Allowed(resolved)` -> `Allowed(landing)` | 6 cases, incl. `a_leaf_link_to_a_file_inside_the_root_is_allowed_under_the_link_spelling` |
| `!admitted(&resolved)` -> `false`; `is_denied(&resolved, ..)` -> `false` | `a_link_spelled_outside_the_roots_stays_refused_when_it_points_inside` (each; for a non-link the landing check repeats these, so only a link spelled outside separates them) |
| `admitted`'s session-root `p != root` dropped; temp branch -> `false` (refactor into a closure) | existing `a_session_root_never_admits_a_file_that_is_the_root_itself`; 6 existing temp-root cases |
| `read_link` failure arm -> `Ok(cur)`; `None => link` -> `PathBuf::new()` | SURVIVED, unreachable without a race |

The mutants ran on the test bytes of `0b0ab92f4`, except the two constant mutants, which ran on `7249a9a3e` (it adds the literal-40
assertion and nothing else to the tests).

## Workarounds

Shape 1: do not keep symlinks to files outside the project root, or to credential files, inside a project root;
and do not keep `.md` links that point outside the root in a repository that uses the librarian.

## Resume

**Status 2026-10-18 (later), branch `fix/classifier-leaf-2026-10-18`, cut from `experiments` `0a8af94d7`.** The classifier
question is closed: `classify_write_path_in` judges where a write through a leaf symlink lands (fix `0b0ab92f4`; what it
answers in each case is under Fix, the pinning tests and the mutation record under Tests added). Verified by running it on
this branch (`scripts/with-slot.sh`): `cargo test --lib util::path_security` 350 passed; `--lib fs::` 262 passed, 1 ignored;
`--lib tools::core::write_ack` 9 passed; whole `cargo test --lib` 7417 passed, 12 ignored, 0 failed; `--test
issue_frontmatter` 12, `--test safety_comments` 3, `--test env_mutation_isolation` 5 passed; `--test result_caps` 71 passed
(it failed once on the new `MAX_LEAF_LINK_HOPS` constant, which needs a `cap-class:` line; fixed in `f4ae1a2d5`); `cargo fmt --all -- --check` and `cargo clippy --workspace --all-targets --features local-embed -- -D warnings`
clean. The issue
stays OPEN, at medium, for what that run did not cover:

- `edit_markdown` (reaches `atomic_write`, read from the caller, not run against a leaf link) and the memory writers
  (`MemoryStore`, `tempfile::NamedTempFile::persist`, no write classifier) were NOT run against a leaf link by this session.
- The classifier follows a dangling or unreadable (EACCES) DIRECTORY link only as far as `fs::canonicalize` does
  (`canonicalize_through_existing_ancestor`), so a leaf link whose target sits below a dangling directory link is judged at
  the spelling `canonicalize` leaves it. Measured for a dangling directory link with `create_dir_all` on 2026-10-17: no
  escape. Not re-measured for a leaf hop.
- The check and the write are separate syscalls: a link re-pointed between the classifier and the write still wins (the
  resolver's second look narrows the window, it does not close it).
- When the refusal is for the link's TARGET, `validate_write_path`'s text still reads "'<raw>' is outside the project root",
  where `<raw>` is the link; the directory it tells the caller to approve is the target's (`WritePathDecision::OutsideRoot`
  carries only the path, so the message cannot say which of the two it is without changing the public enum).

**Status 2026-10-18, on `experiments` `1b9c4dab7`.** Two statements below are out of date.

- The `edit_file` deny-list disclosure is fixed by commit `38a492b4`: the leaf-link target is now run
  through the read and write validators at the shared write resolvers (record:
  `docs/issues/2026-10-17-edit-file-on-a-leaf-symlink-to-a-deny-listed-path-reads-and-copies-the-file.md`,
  status fixed; the in-root twin of that record is closed too). The fixing tests ran green on this tree.
  The classifier question stays open: `classify_write_path_in` still answers allowed for
  `<root>/leaf -> ~/.ssh/id_rsa`, and the refusal comes from the resolvers, not from the classifier.
- The first follow-up bullet below (`validate_approve_path` accepts a directory above the home, such as
  `/home`) is closed by `abbd3ec8` and `0d205e1f`: `approve_write` now refuses a filesystem root, a
  home and any ancestor of one, and a path with an unresolved `..`
  (`docs/issues/2026-10-17-write-allowlist-home-and-tmpdir-gaps.md`,
  `docs/issues/2026-10-17-approve-write-accepts-an-unresolved-dotdot-that-collapses-onto-the-home.md`).
  The full gate ran both commits' tests green on the landing tree (cfa24995); this session did not re-run that bullet's `/home` shape by hand.

Shape 1: measured 2026-10-17. `edit_file`, `create_file` (overwrite) and `edit_code` do not write through a leaf
link (they reach `atomic_write`). Severity stays medium on the strength of the `edit_file` deny-list case alone,
which is a classifier question and so belongs here. Two further behaviours escaped but are NOT classifier
faults (each states in its own Class fit that no write classifier is consulted), so they do not count toward
this issue's severity and were filed on their own:

- `docs/issues/2026-10-17-doc-update-writes-through-a-catalogued-symlink-and-rewrites-a-file-outside-the-root.md` (high)
- `docs/issues/2026-10-17-doc-create-on-a-dangling-symlink-creates-a-file-outside-the-root.md` (medium)
- `docs/issues/2026-10-17-edit-file-on-a-leaf-symlink-to-a-deny-listed-path-reads-and-copies-the-file.md` (high; the one that belongs to this issue)

Still open here (2026-10-17 wording, superseded by the 2026-10-18 status above for the classifier): whether the classifier should resolve the leaf (the deny-list case wants it), and the other
tools that write (`edit_markdown`, memory writes) were not run against a leaf link; `edit_markdown` reaches
`atomic_write` (read from the caller, not run), memory writes do not (see Fix). The follow-ups below stay open.

Other follow-ups from the review of this branch, none fixed here:

- `validate_approve_path` still compares the approved directory with `$HOME` by equality
  (`best_effort_canonicalize(&h) == resolved`). It rejects `/` and `$HOME` themselves, but accepts a
  directory ABOVE the home such as `/home`, and `approve_write` of `/home` then admits `$HOME/.bashrc`
  (the deny list still holds; read from the code, not run). The cwd root refuses an ancestor of the canonical home;
  `validate_approve_path` does not, so that gap remains. For the TEMP root the gap is closed from the other
  side: the temp root is kept even above the home but admits nothing inside it (`temp_root_admits`), so
  `write_ack`'s suggestion of the temp dir's parent no longer undoes anything for the temp case. It does
  for the cwd/approve case.
- A path with a DANGLING symlink or an unreadable (EACCES) component is not followed by
  `canonicalize_through_existing_ancestor`: `fs::canonicalize` fails for it, the walk stops above it and
  keeps it as written. Measured for a dangling directory link with `create_dir_all`: it returned
  `AlreadyExists` and created nothing outside, so no escape came of it; the EACCES case was not run.
- A deny entry that is itself a symlink (`~/.kube/config -> config-prod`, a symlinked `~/.npmrc`) was
  not denied under its own name once its directory was approved, because the entry resolved through its
  last component while the target keeps its leaf as written. Pre-existing; fixed in `97401a1b` by holding
  each entry in two forms (fully resolved, and parent resolved with the leaf as written).
- `atomic_write` USED TO follow a planted `<target>.tmp` symlink (pre-existing, independent of the allowlist);
  fixed in `b493599d`. See
  `docs/issues/2026-10-17-atomic-write-follows-a-planted-staging-symlink-and-overwrites-a-file-outside-the-root.md`.

## References

- `docs/issues/2026-10-16-write-allowlist-holds-the-temp-dir-uncanonicalized.md`
