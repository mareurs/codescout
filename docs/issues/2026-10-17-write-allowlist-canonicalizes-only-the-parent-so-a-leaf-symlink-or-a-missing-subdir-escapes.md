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
Shape 1, a leaf that is itself a symlink, was classified in-root; as of 2026-10-18 `classify_write_path_in` follows the leaf
(`resolve_leaf_link`) and refuses a link whose END is in the deny list or cannot be established (a loop, an over-long chain, an
unreadable link, an unresolvable `..`). A link whose end is merely outside the roots stays `Allowed` as the link, because the
writers replace it (`edit_code` rename, which writes through, is gated by the kept second look in `fs`). That is a policy
decision, confirmed by the user on 2026-10-27 (see Fix); the alternative is under Fix (fixes `0b0ab92f4`, then `520f95fce`, see Fix and Tests
added). The write tools that reach `atomic_write` (`edit_file`,
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

**Shape 1 FIXED 2026-10-18 at the classifier, in two rounds** (branch `fix/classifier-leaf-2026-10-18`).
Round 1 (fix `0b0ab92f4`, patch-id `e3ec71d2788c1429536362fedec0804cd2b923c2`; red tests `329f78326`, patch-id
`b05bd17bcd430e36414e75f196fe3b1745704bf3`) followed the leaf and asked the deny list AND the roots of both the link and its
end. An independent review then found that round 1 over-granted (finding F1, below). Round 2 (red tests `3deeaf771`, fix
`520f95fce`, later commits listed in Resume) changed the policy to the one stated here; round 1's "roots are asked of the end"
rule no longer exists.

`classify_write_path_in` calls `resolve_leaf_link` on the parent-resolved target. It follows the leaf with
`symlink_metadata` + `read_link` (never `metadata`), resolves a relative hop against the directory of the link that names it
(`with_file_name`), sends every hop back through `canonicalize_write_target` (so a hop that crosses a symlinked directory, or
names a file below a missing directory, i.e. a dangling link, is resolved through the deepest existing ancestor), and stops
after `MAX_LEAF_LINK_HOPS` (40, the kernel's `MAXSYMLINKS`) links. Only the END of the chain is judged.

**Policy. DECIDED BY THE COORDINATOR, CONFIRMED BY THE USER ON 2026-10-27.**

- An end in the deny list: `Denied` ("'<link>' is a symbolic link to '<end>', which is in a protected location"), whatever
  roots are approved, asked BEFORE the roots so no `@ack_*` is ever offered (`a_link_outside_the_roots_to_the_deny_list_...`).
- A loop, a chain longer than the limit, a link `read_link` cannot read, or an end holding an unresolvable `..`: `Denied`
  (hard; `approve_write` cannot grant it).
- A link whose own location is outside the roots or in the deny list stays refused for that, whatever it points at
  (`a_link_spelled_outside_the_roots_stays_refused_when_it_points_inside`).
- An in-root link whose end is outside every root and NOT deny-listed: `Allowed(<the link's own spelling>)`, exactly as before
  this branch. Reason: every writer but one REPLACES the link (`util::fs::atomic_write`: `create_file`, `edit_file`,
  `edit_markdown`, `edit_code` replace/insert/remove) and never touches the end, so the roots have nothing to say about the
  end, and an `@ack_*` for the end's directory grants a session root for a write that does not happen there. Measured by
  the review: `create_file` (overwrite) on `pw -> /etc/passwd` returned
  `create_file(path="@ack_...") to write it and approve /etc for this session`; the replay replaced the link, wrote nothing in
  `/etc`, and left `["/etc"]` a session root, after which an unrelated `/etc/...conf` classified `Allowed`. Round 1 produced
  that offer; this session observed the offer in its red tests (`approve_write(path="/dev")` for a link to `/dev/null`, a
  `pending_ack` from `create_file`), and the grant is the existing `maybe_replay_ack` behaviour. It did NOT re-run the
  `/etc/passwd` probe itself.
- `edit_code` `rename` is the one writer that writes THROUGH a link (`std::fs::write`, `edit_code.rs` `do_rename`). It is
  gated by the kept second look, `fs::validate_leaf_link_target` with `LinkTarget::Writable`, which asks
  `validate_write_path` of the end as a regular file. Since `520f95fce` it is the ONLY check that the end is inside a write root,
  because the classifier asks the roots of the link alone (its dangling-link arm decides nothing at rename: the pre-image read
  fails on a dangling link first). Proved through the REAL rename path
  with a mock LSP edit (`MockLspClient::with_rename_edit`): a link to `/dev/null` is refused with the second look's own text
  ("reached through the symbolic link"); a link to an in-root file is renamed through and stays a link
  (`edit_code_rename_through_a_leaf_link_to_...`). Mutants that delete the second look or switch its policy were killed (table).

**The alternative, not implemented:** refuse ANY in-root link whose end is outside every root with a hard denial, "'<link>' is a
symbolic link to '<end>'; the write would replace the link", offering no approval. It is simpler to state and never lets a
link into a place the project does not own be replaced silently, but it refuses a write that is harmless for every writer but
rename (the link is replaced, nothing outside is touched) and it changes the behaviour of every project that keeps a link to a
shared file inside it. The coordinator chose the policy above. The user confirmed it on 2026-10-27: asked in session c78f48d5 whether to keep the current policy (the recommendation was to keep it), they answered "da". That answer settles the policy only. It does not cover what is still unverified (macOS, Windows, case-insensitive filesystems, the `/etc/passwd` probe, the check-versus-write race), so the issue stays `open`.

`fs::validate_leaf_link_target` (the resolver look added by `38a492b4`) is KEPT, and it is two different things by policy:

- `LinkTarget::Writable`, at `edit_code` rename: the ONLY check on the end's roots (the second review's M14, policy switched, and M8a, the
  `Writable` arm discarded, prove it; this session's equivalents are the two rename rows of the table). Tested through the real rename path.
- `LinkTarget::ReadableByTools`, at `write_ack::resolve_write_or_capture` (edit_file, create_file, edit_markdown) and at
  `fs::resolve_write_path_for` (edit_code replace/insert/remove, and the first look of rename): UNTESTED defence in depth. No input
  is known that the classifier lets through and the read deny list refuses, and a mutant that discards the result survives at
  EACH call site (the second review's M9 and M10; table). It is kept for the window in which a link is re-pointed between the
  classifier and the read. That window is real: the review's racing thread re-pointed an in-root link between a harmless file and
  `/etc/gshadow` for 8 s, 142628 flips, 4701 writes, 6768 refusals by the classifier and 3922 refusals by this second
  look ALONE; no non-racing input was found. A deterministic test needs a seam between the two looks, in production code, which
  this change does not add; so the two call sites stay unpinned and are listed as such, not as covered.
- Not shown redundant either way: nothing proves the read deny list is a subset of the classifier's two-form deny list on every
  filesystem shape.

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

Shape 1, the classifier fix (2026-10-18). Every classifier case calls `classify_write_path_in` with a `WriteEnv`; `LeafTree` gives
each its own canonical tree with the temp dir, home and cwd injected. In `src/util/path_security.rs`, after the policy change
(round 2, `520f95fce`): `a_leaf_link_to_a_deny_listed_file_is_refused_whatever_roots_are_approved`,
`a_leaf_link_into_the_deny_list_through_a_symlinked_directory_is_refused`,
`a_leaf_link_whose_target_sits_under_a_symlinked_directory_resolves_through_it` (relative, absolute and dangling hops below a
linked directory, all into the deny list), `a_dangling_leaf_link_is_denied_only_when_the_path_it_names_is_deny_listed`,
`a_leaf_link_chain_is_followed_to_its_end`, `a_relative_hop_resolves_against_the_directory_of_the_link_that_names_it`,
`a_leaf_link_loop_is_refused_and_does_not_hang` (deadline thread; every loop case in the file runs under it),
`a_leaf_link_chain_is_followed_for_exactly_the_hop_limit` (the constant, a literal 40 that must resolve and a literal 41 that
must be refused), `a_leaf_link_whose_target_has_an_unresolvable_dotdot_is_refused`,
`a_leaf_link_that_cannot_be_read_is_refused` (`/proc/1/cwd`: `lstat` works, `readlink` is EACCES for an unprivileged user; skips
SILENTLY as root or where readable, because the harness hides a passing test's output; `CODESCOUT_REQUIRE_UNREADABLE_LINK_TEST`
turns that skip into a failure, for a lane that must not run as root, and that guard itself was not exercised as root), `a_leaf_link_is_allowed_with_or_without_an_approval_of_its_end_and_never_past_the_deny_list`,
`a_leaf_link_to_a_file_outside_every_root_is_allowed_as_the_link_and_asks_for_no_approval` (the policy: a file, a file in the
home directory, a missing file below a missing directory), `a_link_outside_the_roots_to_the_deny_list_is_denied_not_offered_an_approval`,
`a_leaf_link_in_the_temp_root_or_the_cwd_root_is_judged_like_one_in_the_project`, `a_refusal_through_a_link_names_the_link_and_where_it_lands`
(the refusal texts), and the characterisations `a_leaf_link_to_a_file_inside_the_root_is_allowed_under_the_link_spelling`,
`a_link_spelled_outside_the_roots_stays_refused_when_it_points_inside`, `a_leaf_that_is_not_a_link_is_classified_as_before`,
`the_root_profile_still_classifies_a_leaf_link_as_itself`. Through the tools: `src/tools/symbol/tests.rs`
`edit_code_rename_through_a_leaf_link_to_an_outside_file_is_refused_by_the_second_look` and
`edit_code_rename_through_a_leaf_link_to_an_in_root_file_writes_the_target` (the real `EditCode::do_rename`, a mock LSP
answering a rename edit; the outside end is `/dev/null`); `src/tools/edit_file/tests.rs`
`create_file_on_a_leaf_link_to_an_outside_file_replaces_the_link_and_grants_nothing` (no `pending_ack`, no session root, the link
becomes a regular file, `/dev/null` is still the device); `src/tools/core/write_ack.rs`
`a_leaf_link_to_an_outside_path_is_written_as_the_link_and_mints_no_ack` and `a_leaf_link_to_a_deny_listed_path_is_refused_and_offers_no_ack`
(`/etc/gshadow`); `src/fs/mod.rs` `resolve_write_path_for_hands_back_a_leaf_link_out_of_the_write_roots_as_the_link`.
Round 1's `a_leaf_link_into_an_approved_directory_is_written_after_the_ack_is_replayed` was removed (it encoded the ack and never wrote
anything). Red at `3deeaf771` (against round 1's classifier), nine tests: `a_leaf_link_to_a_file_outside_every_root_is_allowed_as_the_link_and_asks_for_no_approval`,
`a_dangling_leaf_link_is_denied_only_when_the_path_it_names_is_deny_listed`, `a_leaf_link_chain_is_followed_to_its_end`,
`a_leaf_link_is_allowed_with_or_without_an_approval_of_its_end_and_never_past_the_deny_list`,
`a_leaf_link_in_the_temp_root_or_the_cwd_root_is_judged_like_one_in_the_project`,
`edit_code_rename_through_a_leaf_link_to_an_outside_file_is_refused_by_the_second_look` (round 1 said "outside the project root.
Call approve_write(path=\"/dev\")" for it), `create_file_on_a_leaf_link_to_an_outside_file_replaces_the_link_and_grants_nothing`,
`a_leaf_link_to_an_outside_path_is_written_as_the_link_and_mints_no_ack`,
`resolve_write_path_for_hands_back_a_leaf_link_out_of_the_write_roots_as_the_link`. The deny-before-roots, unreadable-link,
refusal-text, literal-41 and the loop-deadline cases were green on arrival and are pinned by the mutants below.

Mutation record, `scripts/mutation-probe.sh`, isolated worktree, one mutant per run, on the bytes of `fd5e34515`
(`cargo test --lib util::path_security`, 354 tests, unless noted):

| Mutant | Result: first tests that failed |
|---|---|
| `resolve_leaf_link`: `is_link = false`; `symlink_metadata` -> `metadata` | KILLED, 12+ cases each |
| `hops == MAX` -> `>`; -> `hops + 1 == MAX` | KILLED, the hop-limit case |
| delete `hops += 1` | KILLED, hop-limit, loop, temp/cwd-root and refusal-text cases (each under the 20 s deadline) |
| `MAX_LEAF_LINK_HOPS` 40 -> 39, 41, 2 | KILLED, the hop-limit case (39 by the literal 40, 41 by the literal 41, 2 by both) |
| relative hop taken as written | KILLED, 8 cases |
| relative hop read from the FIRST link's directory | KILLED, `a_relative_hop_resolves_against_...` |
| `cur = next` (hop not re-canonicalized) | KILLED, 5 cases |
| `read_link` failure arm -> `Ok(cur)` | KILLED, `a_leaf_link_that_cannot_be_read_is_refused` (it was reachable without a race; round 1 recorded it as unreachable, wrongly) |
| `Err(why)` -> the link's own spelling | KILLED, 5 cases |
| `has_parent_dir(&landing)` -> `false` | KILLED, the dotdot and the refusal-text cases |
| `is_denied(&landing)` -> `false` | KILLED, 10+ cases |
| landing deny asked only for links spelled in the project (the review's "deny after the roots" mutant) | KILLED, `a_link_outside_the_roots_to_the_deny_list_...` and the temp/cwd-root case |
| `Allowed(resolved)` -> `Allowed(landing)` | KILLED, 8+ cases incl. the in-root spelling case |
| `if !under_allowed_root` -> `false`; `is_denied(&resolved)` -> `false` | KILLED (many existing cases; and `a_link_spelled_outside_the_roots_stays_refused_when_it_points_inside` for the second) |
| session-root `resolved != *root` dropped; temp-root branch -> `false` | KILLED, existing `a_session_root_never_admits_...`; 7 temp-root cases |
| `edit_code` rename second look: `Writable` -> `ReadableByTools`; result discarded (`.ok()`) (`tools::symbol::tests::edit_code_rename`) | KILLED, `edit_code_rename_through_a_leaf_link_to_an_outside_file_is_refused_by_the_second_look` (each) |
| `write_ack::resolve_write_or_capture` second look (`ReadableByTools`): result discarded (`.ok()`), `cargo test --lib tools::` (4301 tests) | **SURVIVED** `tools::` (4301 tests) and the whole lib (7435 tests, re-run on `b58e7f4c5`); untested defence in depth (the second review's M9) |
| `fs::resolve_write_path_for` second look (`ReadableByTools`): result discarded | **SURVIVED** the whole lib (7435 tests, run on `b58e7f4c5`; the second review's M10), untested defence in depth |

Re-run on `b58e7f4c5` after the third review's test edits (the `/dev/null` assertions removed, the unreadable-link guard): the `read_link`
arm mutant and both rename-look mutants are still KILLED by the same single tests (`a_leaf_link_that_cannot_be_read_is_refused`;
`edit_code_rename_through_a_leaf_link_to_an_outside_file_is_refused_by_the_second_look`). No production byte changed in that round (comments only).

The roots closure of round 1 is gone, so the review's "temp-root branch judged on `resolved`" mutant has no site left; the
temp/cwd-root case instead pins what a link in those roots gets.

## Workarounds

Shape 1: do not keep symlinks to files outside the project root, or to credential files, inside a project root;
and do not keep `.md` links that point outside the root in a repository that uses the librarian.

## Resume

**Status 2026-10-18 (after the independent review), branch `fix/classifier-leaf-2026-10-18`, cut from `experiments`
`0a8af94d7`.** The classifier question is closed under the policy in Fix, which is a DECISION PENDING THE USER'S
CONFIRMATION (the alternative, a hard denial for any link whose end is outside the roots, is described there). Round 1 (`0b0ab92f4`)
over-granted; the review's findings and what became of each:

- F1 (medium): fixed by the policy (`3deeaf771` red tests, `520f95fce`). The offered `@ack_*` for a link's end was observed in the
  red tests; the `/etc/passwd` probe itself was not re-run.
- F2 (low): the refusal texts that called the in-root link "outside the project root" or "directly in the home directory" came
  from `OutsideRoot { resolved: <end> }`, an arm that no longer exists; the remaining link refusals name the link
  (`a_refusal_through_a_link_names_the_link_and_where_it_lands`). The home-directory and filesystem-root texts were not
  reproduced separately.
- F3, F4, F6 (test gaps): temp-root and cwd-root cases, deny-before-roots, a literal 41-link chain were added. The review's
  mutants for F3 and F4 were not run against round 1's bytes; the equivalent mutants on the current code are killed (Tests added).
  The F3 mutant (temp branch judged on `resolved`) has no site any more.
- F5 (low): confirmed reachable (`/proc/1/cwd`); test added, mutant killed, the two issue lines that called it unreachable corrected
  (Fix and Tests added no longer say so). The dead `None => link` arm is gone (`with_file_name`); the earlier run's survivor of that
  mutant (round 1) is the evidence that nothing relied on it.
- F7 (low): kept and documented as UNTESTED defence in depth (a code comment in `write_ack.rs` and Fix), at both call sites (`write_ack` and
  `resolve_write_path_for`; the second review's M10 is the second); the discard mutant survives
  4301 `tools::` tests.
- F8 (nit): comments and `docs/manual/src/concepts/security.md` reworded (rename writes through; the manual had said only the parent is
  canonicalized).
- F9 (nit): gone with the policy: `OutsideRoot` carries only the link's own spelling, so no approval is offered for a link's end or its
  parent.
- F10 (pre-existing, documented, not fixed): a deny entry that is a DANGLING symlink protects only its own name, and intermediate hops
  of a chain are never checked against the deny list. A write through `~/.kube/config -> config-prod` (missing) gives
  `OutsideRoot{.../.kube/config-prod}` under round 1; under the policy it is `Allowed` as the link when `config-prod` is not
  deny-listed. No new capability: rename's `Writable` check refuses a dangling chain, and every other writer replaces the link.
  (The round-1 answer is the reviewer's measurement; the answer under the policy is read from the code, not run.)
- F11 (documented): a hard link to a protected file stays `Allowed` (the classifier compares paths, not inodes); git cannot ship one.
- Not verified at all: macOS (`/var` -> `/private/var`), case-insensitive filesystems, Windows (`read_link`/junction semantics; the
  tests are `cfg(unix)`).
- `edit_markdown` DOES consult the classifier (`resolve_write_or_capture`, `edit_markdown.rs` line 1550; read from the code). The second
  review ran it, through `edit_file`'s heading and frontmatter path, against a leaf link: nothing was written through. This session
  did not run it. `MemoryStore::write` is reported by the reviewer's probe to refuse a leaf link but NOT a symlinked
  `.codescout/memories` directory (documented design, `docs/conventions/memory-write-history.md`); this session did not run it. The
  memory store's READ of a leaf link with no deny list (reviewer findings F12 and F13) is out of scope here and goes to another session.

- Correction to the brief of this work: `~/.config/codescout` and `.codescout/project.toml` are NOT in the deny list
  (`platform/unix.rs`, `denied_read_prefixes`). Probed 2026-10-19 with a throwaway test (not committed) over a temp home and project:
  a link to `<home>/.config/codescout/config.toml` classifies `Allowed(<link>)` (the target itself is `OutsideRoot`, not `Denied`), and a
  link to `<project>/.codescout/project.toml` classifies `Allowed(<link>)` (the target itself is `Allowed`); `atomic_write` on either
  link replaced it with a regular file and left the target's bytes (`ORIG`) untouched. So they are replaced, not written through,
  and not protected from a plain write either. Whether they belong on the deny list is a separate question, not decided here.

**Open decisions for the user (not changed by this work; tool output deliberately untouched).** With the policy, `create_file`,
`edit_file` and `edit_code` on an in-root link to a shared outside file return a bare "ok" while the LINK was destroyed (the second
review's probe: 12 of 12 cases ended with the path a regular file and the outside file unmodified). The librarian rejects exactly
this (`src/librarian/util.rs`, `refuse_write_through_symlink`), so the two subsystems disagree about whether replacing a link is a
legitimate write. Options: (1) keep the policy and make the tools say "replaced symlink X (-> Y); Y not modified"; (2) a hard denial
for any in-root link whose end is outside the roots ("'<link>' is a symbolic link to '<end>'; the write would replace the link"),
which also removes the disagreement. Both need the user's decision; the policy is also still pending the user's confirmation (Fix).

Verified by running it on this branch (`scripts/with-slot.sh`): whole `cargo test --lib` exit 0, `7423 passed; 0 failed; 12 ignored`;
`--test result_caps` 71 passed; `--test issue_frontmatter` 12 passed; `--test safety_comments` 3 passed; `--test
env_mutation_isolation` 5 passed; `cargo fmt --all -- --check` exit 0; `cargo clippy --workspace --all-targets --features local-embed
-- -D warnings` exit 0. The issue stays OPEN, at medium, for what that run did not cover:

- The memory writers (`MemoryStore`, `tempfile::NamedTempFile::persist`, no write classifier) were NOT run against a leaf link by this
  session, and neither was `edit_markdown` (the reviewer did).
- The classifier follows a dangling or unreadable (EACCES) DIRECTORY link only as far as `fs::canonicalize` does
  (`canonicalize_through_existing_ancestor`), so a leaf link whose target sits below a dangling directory link is judged at
  the spelling `canonicalize` leaves it. Measured for a dangling directory link with `create_dir_all` on 2026-10-17: no
  escape. Not re-measured for a leaf hop.
- The check and the write are separate syscalls: a link re-pointed between the classifier and the write still wins (the
  second looks narrow the window, they do not close it).

**Status 2026-10-18, on `experiments` `1b9c4dab7`.** Two statements below are out of date.

- The `edit_file` deny-list disclosure is fixed by commit `38a492b4`: the leaf-link target is now run
  through the read and write validators at the shared write resolvers (record:
  `docs/issues/2026-10-17-edit-file-on-a-leaf-symlink-to-a-deny-listed-path-reads-and-copies-the-file.md`,
  status fixed; the in-root twin of that record is closed too). The fixing tests ran green on this tree.
  (OUTDATED since `0b0ab92f4`: `classify_write_path_in` used to answer allowed for `<root>/leaf -> ~/.ssh/id_rsa`, with the
  refusal coming from the resolvers; it now refuses it itself.)
- The first follow-up bullet below (`validate_approve_path` accepts a directory above the home, such as
  `/home`) is closed by `abbd3ec8` and `0d205e1f`: `approve_write` now refuses a filesystem root, a
  home and any ancestor of one, and a path with an unresolved `..`
  (`docs/issues/2026-10-17-write-allowlist-home-and-tmpdir-gaps.md`,
  `docs/issues/2026-10-17-approve-write-accepts-an-unresolved-dotdot-that-collapses-onto-the-home.md`).
  The full gate ran both commits' tests green on the landing tree (cfa24995); this session did not re-run that bullet's `/home` shape by hand.

Shape 1: measured 2026-10-17. `edit_file`, `create_file` (overwrite) and `edit_code` replace/insert/remove do not write
through a leaf link (they reach `atomic_write`); `edit_code` rename does (`std::fs::write`), and is gated by the second look in `fs`. Severity stays medium on the strength of the `edit_file` deny-list case alone,
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


**Update 2026-10-27 — policy decided by delegation, two gaps pinned.** The operator told this session "as you say is best, trust you" and asked it to continue without asking. On that basis the policy in Fix is KEPT: option 1's policy half, with no new refusal. The alternative (a hard denial for any in-root link whose end is outside the roots) stays rejected, because it refuses writes that are harmless for every writer but rename and changes behaviour for projects that keep such links.

Not built: the "replaced symlink X" notice. Checked in the code: it needs about 15 success return points in `create_file`, `edit_file`, `edit_markdown` and `edit_code`, with string and object response shapes mixed. A field on `ToolContext` is not viable (287 construction sites). A hook in `call_tool_inner` would couple the server to tool names and path-param aliases. Revisit when a real occurrence shows in `usage.db`, or when the write tools gain a shared result type. Confidence medium; how often an agent writes through such a link was not measured.

Pinned in `4d70f9594` (patch-id `e34e96c8f03d7632993e22188d975d9c44ebffa5`), both characterization tests that passed on first run:
- `edit_file_heading_grammar_on_a_leaf_link_replaces_the_link_and_spares_the_victim` (`src/tools/edit_file/tests.rs`): the heading grammar replaces the link and leaves the outside file unchanged.
- `a_memory_write_onto_a_leaf_link_never_modifies_the_file_it_points_at` (`src/tools/memory/tests.rs`): shared and private stores; the write may succeed or refuse, but never reaches the pointed-at file.

Still unverified: macOS, case-insensitive filesystems, Windows; the `/etc/passwd` probe; and the check-versus-write race, which the second looks narrow and do not close. The test for the memory store does not say whether it refuses or replaces. The issue stays `open`.

## References

- `docs/issues/2026-10-16-write-allowlist-holds-the-temp-dir-uncanonicalized.md`
