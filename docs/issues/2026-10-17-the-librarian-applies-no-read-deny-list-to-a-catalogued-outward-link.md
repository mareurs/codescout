---
status: open
opened: 2026-10-17
closed:
severity: high
owner: marius
related: [docs/issues/2026-10-17-edit-file-on-a-leaf-symlink-to-a-deny-listed-path-reads-and-copies-the-file.md, docs/issues/2026-10-17-doc-update-writes-through-a-catalogued-symlink-and-rewrites-a-file-outside-the-root.md]
tags:
- cluster/unclassified
kind: bug
unverified: 'Phase 1 (2026-10-18, experiments 44cb3bfe4): every reader listed in Design (Phase 1) was run against a decoy key and leaks, including doc(find) semantic, librarian(context), doc(gather), gather_from sources, params_path and both backfill arms. The earlier claim that the chunk table holds no text was FALSE: artifact_chunk.content stores it. Rows 9 and 17-25 of the path table were read, not run. Not fixed. macOS and Windows not run. Whether the configured embedder is remote on a given install was not checked.'
---

# BUG: the librarian applies NO read deny-list, so a repository can ship `notes.md -> ~/.ssh/id_rsa` and have the key catalogued, embedded and returned by `doc(get)`

## Summary

`grep -rn "validate_read_path\|denied_read\|is_denied" src/librarian` returns nothing: no librarian read path consults the
deny-list that `read_file` applies. The indexer keeps an in-root `.md` symlink whose target is OUTSIDE the root on purpose
(`src/librarian/indexer.rs`, the `is_symlink` block). So a repository that commits `notes.md -> <home>/.ssh/id_rsa` gets that
file catalogued on the next reindex. It is the READ twin of the `edit_file` leaf-link issue, in a different subsystem and with a
longer reach: the key's text is queued for the embedder and `doc(get)` returns it.

## Class fit

A containment check missing from a read path whose input a repository controls: the same absence as the `edit_file` leaf-link
read, reached through the catalog instead of a tool argument. No named class covers it; filed unclassified.

## Symptom (Effect)

After `librarian(reindex)` on a repository containing the link, `doc(get, id=<notes.md id>)` returns the key's contents, with and
without `full=true`, and the same text sits in the embedding queue.

## Reproduction

Measured 2026-10-17 on `worktree-agent-a66600865e063ffd2` with a throwaway test (removed), a fake home and a decoy key only:
`fake/home/.ssh/id_rsa` holding `-----BEGIN FAKE KEY-----\nFAKE-KEY-MARKER\nsecond line\n-----END FAKE KEY-----`, a project
root holding `notes.md` symlinked to it, `mk_ctx(root)`, then the real `index_repo_sync(.., want_embeddings = true, ..)` and
`get::call`:

```
PROBE added=1 queue_len=1
PROBE queue_has_marker=true          # the key's text is in the embedding queue
PROBE get_full_has_marker=true       # doc(get, full=true)
PROBE get_default_has_marker=true    # doc(get) with no body selector
```

No real key was involved; the deny-listed location is irrelevant to the librarian, which has no list. Any file the user can
read and that a link can name is reachable the same way.

## Environment

Linux, `cargo test --lib`, `TMPDIR=/home/marius/.cache/cc-tmp`.

## Root cause

`indexer.rs` keeps outward FILE links deliberately and reads them with `std::fs::read(path)` (following the link) to hash and
chunk them; `EmbedQueueItem.text` carries the chunk text onward to whatever embedder is configured, and the artifact readers
(`doc get` and the others) read the file by `abs_path`. None of these asks whether the resolved target is somewhere `read_file`
would refuse. The librarian's `ToolContext` carries no `PathSecurityConfig`, so there is nothing to ask with.

## Evidence

Measured, above. Read, not run: the `artifact_chunk` table stores `chunk_id, artifact_id, chunk_ix, start_line, end_line, ...`
and no text, so a semantic hit for the artifact is hydrated from the FILE at query time, through the same link; the embedder
receives the text at index time. **(False, corrected in Design (Phase 1): the table stores the chunk text in `content`, and a
semantic hit returns it from the catalog.)**

## Hypotheses tried

1. **Hypothesis:** only `.md` targets matter, so a key (no extension) is safe. **Test:** the probe. **Verdict:** rejected. The
   LINK is named `notes.md`; the target can be anything.
2. **Hypothesis:** the embedder is local, so the text goes nowhere. **Verdict:** not established; `embedding` backends include a
   remote HTTP one, so this is an exfiltration path on such installs, not only a display one.

## Fix

Not applied (filed as a follow-up of the symlink-escape work). Options, none free:

- Apply the read deny-list where an outward link is KEPT: pass the project's `PathSecurityConfig` into the librarian's
  `ToolContext` and refuse to catalog (or to read) a row whose resolved target is deny-listed. Needs the config plumbed through
  `index_repo_sync`, the artifact readers and the embedding drain.
- Stop cataloguing outward file links. The indexer comment chose keeping them over a "silent omission"; with the write side now
  refusing every link, an outward-linked row is read-only anyway, so the trade is worth revisiting.
- A narrower floor: refuse an outward link whose target is under `$HOME`'s dot-directories. Cheaper, and wrong in the ways
  deny-lists are wrong.

## Design (Phase 1)

Written 2026-10-18 on `worktree-agent-a8918c1498fdd30e8`, reset to `experiments` `44cb3bfe4` (the worktree had been created from
`master`, which lacks the write door). Nothing below is implemented. Every "measured" line comes from a throwaway test
(`src/librarian/tools/read_probe_tests.rs`, run with `scripts/with-slot.sh cargo test --lib`, then deleted). It used a fake home
holding the decoy key `-----BEGIN FAKE KEY-----\nFAKE-KEY-MARKER\n…`, a fake `.docker/config.json` and outside `.md` files, all
under `TempDir`s. The deny-list needed no fake `HOME` because nothing in the librarian consults it. "Read" means read in the
code, not run.

### Correction to this issue's Evidence and `unverified:`

The chunk table holds the TEXT. `artifact_chunk` has `content TEXT NOT NULL` (`src/librarian/catalog/mod.rs:298-310`;
`ChunkRow.content`, `catalog/chunk.rs:41`). A semantic hit is therefore NOT hydrated from the file: `doc(find)` returns up to
480 characters of `artifact_chunk.content` as `matched.snippet` (`tools/find.rs:1103-1116`). So the key is copied into the
machine-wide `catalog.db` at index time and outlives the link. Qdrant payloads carry ids only (`src/retrieval/artifact.rs:121-124`).

### Path table: every librarian read of catalogued or repository-named content

"Follows" means a plain `std::fs` open, which follows a leaf link and every parent link.

| # | site | path from | follows | content goes to | at `44cb3bfe4` |
|---|---|---|---|---|---|
| 1 | `indexer.rs:484` `index_repo_sync` walk (`std::fs::read(path)?`) | main walk + `force_include` walk | leaf: yes, on purpose. The `is_symlink` block (`:458-468`) skips only links that resolve inside the walk root. Parent: refused by `parent_escapes` (`:476`) | `file_sha256`, title and frontmatter fields into the row, `artifact_chunk.content`, `EmbedQueueItem.text` and then the embedder | **leaks** (measured: the embedder received the key, 1 chunk row holds it) |
| 2 | `indexer.rs:721-760` reclamation | `Path::exists()`, follows | n/a | decides deletion | a row with history below a directory swapped for an outward link is KEPT and stays readable (measured, see #7) |
| 3 | `indexer.rs:1447` backfill, artifact with no stored chunks | `row.abs_path` | yes | chunks, embedder | **leaks** (measured, `BackfillTarget::Store`) |
| 4 | `indexer.rs:1465-1472` backfill, stored chunks | `artifact_chunk.content`, no file read | n/a | embedder | **re-sends the stored key** (measured, `BackfillTarget::SqliteVec`) |
| 5 | embed drain in `reindex` | `EmbedQueueItem.text` from #1 | n/a | embedder | through #1 only |
| 6 | `src/cli/backfill_chunks.rs` | calls #3/#4 | n/a | n/a | through #3/#4 |
| 7 | `tools/get.rs:645`: one read feeds `preview`, the body, `full`, `heading`/`headings`, `start_line`/`end_line` and `extra` | `row.abs_path` (`resolve_file_path`, `:21-26`) | yes | returned | **leaks** (measured: default, `full=true`, lines 1-3; also a kept escaping row and a stale row) |
| 8 | `tools/context.rs:916` | `row.abs_path` | yes | the first 30 body lines, returned | **leaks** (measured: `anchor_id`, and `topic`) |
| 9 | `tools/context.rs:128` `load_slug` | `row.abs_path` | yes | statement text (needs a `## X-N —` heading) | read |
| 10 | `tools/refresh.rs:21` `read_body` | `row.abs_path` | yes | `current_body` of `doc(gather)` | **leaks** (measured) |
| 11 | `tools/gather.rs:455` `gather_file` | params `gather_from` path joined to the project root; lexical guard only (`:429-449`) | yes | returned | **leaks** (measured) |
| 12 | `tools/gather.rs:484` `gather_grep` | same | the root is followed; links below it are not | matching lines, returned | **leaks** (measured, `path` = a directory link) |
| 13 | `tools/gather.rs:526` `gather_config_value` | same | yes | a value, returned | **leaks** (measured: a link to a fake `.docker/config.json`) |
| 14 | `tools/find.rs:1103-1116` semantic snippet | `artifact_chunk.content` | n/a | returned | **leaks** (measured) |
| 15 | `doc(find)` without `semantic` | catalog row | n/a | `title` only | no leak for the key (it has no H1); a `# ` line in a target becomes the title (read) |
| 16 | `tools/augment.rs:319` `params_path` | ANY path the agent names | yes | stored as params, returned by `doc(get)` | **leaks** (measured: a fake `.docker/config.json` read and returned) |
| 17 | read-modify-write verbs that read BEFORE the write door: `update.rs:587` (door `:780`), `update.rs:966` (`:1008`), `augmentation.rs:1016` (`:1029`), `:1837` (`:1980`), `:2179` (`:2257`), `rekey.rs:377` (`:476`), `doctor.rs:2040` (refusals `:2076`/`:2084`) | `row.abs_path` | yes | written back (refused by the door); errors raised before the door can echo content: frontmatter parse errors, the nested-heading list, the splice heading hint | read |
| 18 | `doctor.rs` scans, about 30 sites, machine-wide, `scope.admit` after the read | `row.abs_path` | yes | echoed fields: frontmatter `id`, `status`, `unverified` (up to 240 characters), `claimed_by`, `expects_augmentation`, `**Valid:**` text, SHA and patch-id text; elsewhere counts, hashes and tokens | read |
| 19 | `tools/link_scan/mod.rs:385` | `row.abs_path` | yes | `raw` citation tokens | read |
| 20 | `requirements.rs:99` `declared_values` | `.codescout/requirements.toml` `tag_values_from`: ANY path (`root.join(rel)`, absolute and `..` accepted) | yes | tokens matching `[a-z-]+` on lines with a repository-chosen prefix | read |
| 21 | `doctor.rs:5683` `external_authority_problem` | frontmatter `external_prefix` authority: ANY path | yes | yes/no: "holds a `PREFIX-<n>` id" | read |
| 22 | `tools/mv.rs:328` pre-rename read | `row.abs_path` | leaf yes (a move moves the link); parent refused (`:234-238`) | a hash into the moved row | read |
| 23 | `tools/reindex.rs:92`, `doctor.rs:7062` artifact reads (their sidecar reads are confined) | `row.abs_path` | yes | the declared sidecar `rel` | read |
| 24 | `audit_doc_refs/resolver.rs:415`, `:374` | paths cited in a doc; lexical guard | yes | a line count, a yes/no | read |
| 25 | repository config: `classify.rs:211` (`librarian.toml`), `indexer.rs:827` (`project.toml`), `current_project.rs:109`, `requirements.rs:66` | fixed names under the root | yes, if the config file is itself a link | parsed; a TOML error can quote a line of the file (`load_project_rules` propagates it from `librarian/mod.rs:302`; where it surfaces was not traced) | read |

Already confined: declared sidecars (`augmentation_sidecar::read_in`/`confined_path`, round 2), `doctor.rs:2126` (its caller
checks first), and the walkers that skip links (`doctor.rs:8204`, `audit_doc_refs/mod.rs:674`). Out of scope: `audit/shard.rs`,
`audit/host.rs`, `session_registry.rs`, the `taxonomy_recipes` test module. The site list for rows 9 and 17-25 came from a
read-only subagent sweep. I re-read rows 17 (`update.rs:575-640`), 20 and 21 myself.

### Measured baseline

```
reindex                       added=3 (notes.md -> fakehome/.ssh/id_rsa catalogued), embedder saw the key: true
artifact_chunk.content        rows containing the marker: 1; title of the row: None
force_include "vault/**", vault -> outside dir      catalogued: false, embedded: false   (round-2 fix holds)
force_include "../outside/**" and "/abs/**"         catalogued: false
doc(get) default / full / start_line..end_line      true / true / true
doc(find) plain / semantic                          false / true (matched.snippet from the chunk table)
librarian(context) anchor_id / topic                true / true
doc(augment) on the link row                        accepted
doc(gather) on the link row                         current_body: true
gather_from file "notes.md" (from a regular row)    true
gather_from grep path "vault" (a directory link)    true
gather_from config_value "cfg.json" -> fake .docker/config.json   true
doc(augment, params_path=<fake .docker/config.json>) then doc(get)   true (augmentation.params)
stale leaf: docs/real.md replaced by a link after indexing, no reindex   get / context / gather: true / true / true
  ... then reindex                                  updated=1, embedder saw the key again: true
directory kept/ swapped for an outward link, row has an event   reindex: escaping_kept_ids=[the row]; get / context return the OUTSIDE file: true / true
backfill SqliteVec (stored chunks)                  re-embedded the stored key: true (no file read)
backfill Store (stored chunks deleted)              read the key through the link and embedded it: true
existing door, refuse_catalogued_path_outside_repo(roots, link, "read"):
  outward leaf, roots=[proj]: refuse; roots=[] (spelled .git): refuse; in-repo leaf link: admit; plain file: admit
```

Live catalog, opened `mode=ro` (`~/.local/share/librarian/catalog.db`, 2026-10-18): 6223 rows. 0 are leaf links, 0 have any link in
their parent chain, 23 name a missing file. The table holds 122,923 chunk rows. No live data is at stake.

### Who controls each input

| input | where it lives | controlled by | can it name an outward read? |
|---|---|---|---|
| file and directory links | repository content | the repository's author, or anyone whose commit you pull (a `git pull` can turn a catalogued file into a link: the stale row above) | **yes** (measured) |
| `[ignored_paths] force_include` | `<root>/.codescout/project.toml` (read by `indexer.rs:825-843`). This repository gitignores that file (`.gitignore:14`), but any repository can commit one, and the agent can write it | repository or agent | no, at this tree. An absolute anchor replaces the root in `join`, then every result fails `strip_prefix` and is dropped. A `..` anchor is dropped by `parent_escapes`. `~` is not expanded. All measured |
| `[security] profile = "root"` | same `project.toml` | repository or agent (open issue `2026-10-17-project-toml-under-the-writable-project-root-can-set-security-profile-root.md`) | it turns off the deny-list of `read_file`. **The librarian must not honour it** |
| `.codescout/librarian.toml` | committed here | repository | no: `[[rule]]` classification only (`classify.rs:199-214`). The walk still takes `.md` names only. It carries no `force_include`, contrary to the brief |
| `.codescout/requirements.toml` `tag_values_from` | committed here | repository | **yes**, any path (row 20) |
| frontmatter `external_prefix` authority | committed docs | repository | **yes**, any path, as a yes/no answer (row 21) |
| frontmatter `expects_augmentation` | committed docs | repository | no, confined since round 2 |
| `<root>/.codescout/workspace.toml` `[[umbrella]]` | repository (gitignored here) | repository | no: only the umbrella NAME is taken from it. Members come from the global file (`reindex.rs:257-276`; read, not run) |
| `~/.config/librarian/workspace.toml` roots and umbrella members | operator config | operator (trusted) | walk roots. The live file has 9 `[[roots]]`; none is `$HOME` or contains a deny-listed directory |
| augmentation params `gather_from` | catalog, written by `doc(augment)`. Sidecars restore params EMPTY (`augmentation_sidecar.rs:141`), so a repository cannot ship them | the agent, which a repository's text can prompt-inject (a restored sidecar `prompt` is repository text) | **yes** (measured) |
| `doc(augment)` `params_path` | tool argument | the agent | **yes**, any path (measured) |
| `codescout doc augment --params @file` | CLI | operator | trusted; no change |
| rows and chunk text written by an older indexer | `catalog.db` | whatever the old walk read | **yes** (rows and stored text) |

### Options

| option | what it closes | what it leaves open | what it breaks | blast radius | check-then-read race |
|---|---|---|---|---|---|
| 1. Plumb `PathSecurityConfig` into the librarian, refuse deny-listed targets | the 36 entries of the Unix deny list (`platform/unix.rs:192-244`), at the sites that get the config | every other file the user can read: other repositories' private docs, `~/Documents`, unlisted `~/.config/*`, browser profiles. Also stored chunk text, and a repository-settable `profile = "root"` turns it off | nothing | a new `ToolContext` field set in every constructor (`build_tool_context`, the CLI's `open_ctx`, `TestToolContextBuilder`), plus every site | open unless combined with an open-then-verify read |
| 2. Stop cataloguing outward file links (indexer only) | new outward leaf rows | stale rows (a pulled commit turns a file into a link), kept rows with history, rows and chunk text from older catalogs, and every input that is not the catalog (rows 11-13, 16, 20, 21) | an outward link to shared docs leaves the catalog | `indexer.rs` | indexer only |
| 3. Refuse targets in `$HOME` dot-directories | `~/.x` targets | everything else, including non-dot secrets and every non-catalog input | — | small | — |
| **4. A read door: containment by the write door's `door_root`, the deny-list as a second layer, open then verify the opened file, at every site** (recommended) | every repository-named read, for every reader, including stale and older rows and the stored chunk text. The agent-named `params_path` gets `read_file`'s deny-list | hard links, Windows' race window, titles of kept rows (D4), vectors already sent to a remote embedder (they cannot be recalled) | an outward link to shared docs leaves the catalog, and the reindex reports it (D1). F2-shaped rows (a nested repository linking into its superproject) stop being catalogued, matching the write door | `util.rs` plus about 12 files in tier 1; `doctor.rs`, `link_scan`, `requirements.rs` in tier 2 | closed on Linux (below) |
| 5. `O_NOFOLLOW`, or `openat2(RESOLVE_BENEATH)` | the leaf (`O_NOFOLLOW`); every escape (`RESOLVE_BENEATH`) | parent links (`O_NOFOLLOW`). `RESOLVE_BENEATH` is Linux ≥ 5.6 only and needs a raw syscall | in-repo leaf links, including the monorepo `P/notes.md -> ../shared/notes.md`. `RESOLVE_BENEATH` also refuses every absolute link, even one into the repository | small | strongest on Linux; no macOS or Windows equivalent in `std` |

### Recommendation (option 4) and the invariant

**Invariant, to be stated once in `util.rs` and in each call site's doc:** *the librarian hands the bytes of a file that a
repository or the catalog names (to a caller, to the chunk table or to an embedder) only if the file, AS OPENED, lies inside
the root `door_root` names for its spelling, and is not on the read deny-list. With no root to name, it reads only a path whose
leaf is not a symbolic link.* The root is the write door's root, so the librarian reads only where it would write. The read
door adds no second notion of "inside". The only difference from the write door is that a leaf link resolving INSIDE is read,
because its bytes are the repository's own. The measured probe above shows `refuse_catalogued_path_outside_repo` already
answers the leaf-link rows correctly.

The read door, `util::read_catalogued_file(roots, path) -> Result<Vec<u8>, ReadRefusal>`, plus `_to_string` and a predicate
`catalogued_read_admitted(roots, path)` for the stored-text sites. Order, and why it is closed on Linux:

1. `File::open(path)`, following every link once. The descriptor names one inode, X.
2. `fstat` on the descriptor: it must be a regular file, else refuse (directory, fifo, device).
3. `real = fs::canonicalize(path)`. Judge `real` against `door_root(roots, path)`, resolved as the write door resolves it, and
   against the deny-list (`validate_read_path` with `PathSecurityConfig::default()`, so the repository-settable `root` profile
   does not apply; no plumbing needed).
4. `stat(real)` must give the same `(st_dev, st_ino)` as step 2, else refuse ("the file changed while it was being read").
5. Read from the descriptor of step 1. Never reopen by path.

Guarantee (Linux, and any Unix where `(st_dev, st_ino)` names a file): the bytes returned are inode X's, and after the open X
was reachable at `real`, which passed the judgement. The open descriptor keeps X allocated, so its inode number cannot be
reused while we compare. A link swapped between any two steps gives a different inode or a refused `real`, so the read fails
closed. Not covered: a HARD link to the outside file placed inside the repository. It has the same inode at an in-root path.
`git checkout` cannot create one, and a process that can create one can already read the target. The same goes for a bind mount
made by the operator. `std::os::unix::fs::MetadataExt` gives `dev()`/`ino()` on macOS as well, but **macOS was not run**.
Windows has no stable file index in `std`, so there the door judges and then reads with a race window. **Windows is unverified,
and out of scope by the brief.**

Where each site gets its roots: tools use `tools::managed_roots(ctx)`. `index_repo_sync` uses `[escape_root]`, the git root
of the walk root it already computes (`indexer.rs:382-385`), so its signature and its ~70 test callers do not change. The CLI
backfill passes `managed_roots` of the context it already opens. When the indexer and a reader disagree, both outcomes fail
safe: either a row is omitted, or a row is catalogued and its read is refused. Neither returns bytes.

Smallest sufficient change, tier 1 (the raw-content leak): the door and a table test. Then:

- the walk read (#1): a refused file is skipped and recorded with the escaping ids;
- reclamation (#2): judged by the same door predicate;
- backfill (#3, #4): the door on the file, the predicate before re-embedding stored chunks;
- `get`, `context` (#8, #9), `refresh::read_body` and the three `gather` sources (one directory-door check of the grep root;
  its walk already skips links);
- the find snippet (#14): withheld, with the reason, when the predicate refuses;
- `params_path` (#16): the deny-list only, to match `read_file`, because its documented use is a JSON file in a scratch or
  temp directory;
- the read-modify-write verbs (#17): run the existing write-door preflight (`refuse_write_through_symlink` +
  `refuse_catalogued_path_outside_repo`) BEFORE the read, so no error built from the content can be raised.

Tier 2 (derived values and narrow oracles): one `read_row_text` helper for the `doctor` scans, `link_scan`, `mv`'s hash,
`reindex`'s and `doctor`'s artifact reads, `requirements.rs`, `external_authority_problem` (refused, reported as a finding),
the doc-cited paths of `audit_doc_refs`, and repository config files (#25). Then a source gate,
`no_librarian_production_read_bypasses_the_read_door`, in the style of `like_escape_idiom_is_not_inlined_outside_helper`. It
scans `src/librarian` production code for `fs::read`/`read_to_string`/`File::open` outside `util.rs`, against an allowlist of
fixed-name config reads, and gives every allowlist entry a liveness floor. The gate is what makes the invariant hold for the
next reader someone adds. Without tier 2 the invariant is true only for raw content, and the issue must say so.

**Existing rows after the fix.** On the next reindex of their project, rows the door refuses are reclaimed the way round 3 set up.
`DELETE FROM artifact` cascades to `artifact_chunk` (the stored text) and its vectors on sqlite, and `removed_ids` drops the
Qdrant points. Rows that carry an augmentation or events are KEPT, per round 3's keep-forever decision (reclaiming deletes
history). For a kept row the fix ALSO deletes its chunk rows and vectors (derived data that can be rebuilt) and lists it in the
existing `escaping_kept_ids`, plus a new `escaping_chunks_dropped` count. A later backfill then finds no chunks, tries the
file, is refused and counts `refused_outside_repo` instead of embedding. Rows never reindexed are protected at read time: the
door covers file reads, and the predicate covers the stored text (snippet, backfill). New in the reindex response:
`skipped_outside_repo` and `skipped_outside_repo_paths`, the LINK paths, not their targets, uncapped like
`reclaimed_escaping_ids`. The silent omission the indexer comment feared becomes a reported one. The text a remote embedder
already received cannot be recalled; the note says so.

### Red tests to write first (each run and seen failing before any fix)

1. `read_door_layout_table`: every row of `door_layout_table`, `_round6` and `_round7` gets a read column. It must equal the
   write door's containment verdict, because their leaves are not links. New leaf rows: L1 outward absolute leaf REFUSE; L2
   outward relative leaf REFUSE; L3 in-repo leaf ADMIT; L4 monorepo leaf into the git root but outside the project ADMIT; L5
   dangling leaf yields no bytes; L6 a leaf link to a directory REFUSE (not regular); L7 a deny-listed target inside a managed
   root REFUSE, by the deny layer alone (fake `HOME` child process, as in the `edit_file` tests); L8 no root, no repository,
   leaf link REFUSE; L9 no root, no repository, plain ADMIT.
2. `a_catalogued_outward_link_reaches_no_reader_and_no_embedder`: the probe made permanent, with a capturing embedder. It
   asserts the marker appears in none of: embedder inputs, `artifact_chunk.content`, `doc(get)` (default, `full`, lines,
   heading), semantic `doc(find)`, `librarian(context)` (`anchor_id`, `topic`), `doc(gather)` `current_body`. It also asserts
   the reindex reports the link path. Control: a regular file's marker appears in each of them.
3. `a_file_replaced_by_an_outward_link_after_indexing_is_not_read_before_the_next_reindex`.
4. `the_next_reindex_reclaims_an_outward_row_and_its_stored_chunk_text`: the row is seeded through today's walk, and after the
   reindex no `artifact_chunk` row remains for it.
5. `an_outward_row_with_history_is_kept_but_its_chunks_and_vectors_are_dropped`: the leaf form and the swapped-directory form.
6. `backfill_never_embeds_an_outward_row_from_disk_or_from_stored_chunks`: both targets.
7. `gather_sources_refuse_a_link_out_of_the_repository`: file, grep root, `config_value`, plus an in-repo control.
8. `params_path_refuses_a_deny_listed_file` (fake `HOME` child), plus the control `params_path_still_reads_a_json_file_in_temp`.
9. `a_read_modify_write_verb_refuses_before_it_reads`: the target is markdown with headings and frontmatter, and the error
   text contains none of it.
10. `the_read_door_refuses_a_file_that_is_not_the_one_it_opened`: a unit test of the verify step through a seam that takes
    the opened file's metadata and `real`.
11. Tier 2: `doctor_never_echoes_a_field_of_a_file_behind_an_outward_link`,
    `requirements_tag_values_from_refuses_a_path_outside_the_repo`, `external_authority_outside_the_repo_is_a_finding_not_a_read`,
    and the source gate with its liveness floor.

### Mutation targets (`./scripts/mutation-probe.sh`, own slot)

- In the door: drop the containment check; drop the deny layer; make the inode comparison always pass; compare `ino` without
  `dev`; judge the spelled path instead of `real`; drop the regular-file check; reopen by path for the read.
- At the call sites (the load-bearing mutations, per the test-design memory): put back a raw read in the walk, both backfill
  arms, `get`, `context` (both), `read_body`, the three `gather` sources, the snippet predicate and `params_path`; drop the
  preflight in each read-modify-write verb; return reclamation to `parent_escapes`; drop the kept-row chunk purge; drop the
  new report fields.
- Roots: `managed_roots(ctx)` replaced by `[]` at each tool site. Some rows are expected to stay equivalent through the
  spelled-repository fallback; each such survivor gets a probe or a stated argument.

### Decisions needed from the operator

- **D1.** Outward file links: stop cataloguing them and report the link paths in the reindex response (recommended), or keep
  them catalogued but unreadable?
- **D2.** Is the read door's root the write door's `door_root` (recommended: one notion of inside, and the seven-round table
  carries over)? It also stops cataloguing F2-shaped rows that the walk admits today.
- **D3.** Deny-list second layer: always on, ignoring `[security] profile` (recommended: that profile is repository-settable),
  or plumb `PathSecurityConfig` and honour `root`?
- **D4.** Kept rows: drop their chunks and vectors (recommended). Should their title, and other frontmatter-derived fields
  taken from an outward target by an older walk, be cleared too, or kept and stated as a residual?
- **D5.** `params_path`: deny-list only (recommended, `read_file` parity), or containment (breaks the documented
  temp-file workflow)?
- **D6.** Scope: tier 1 only (closes the raw-content leak; the issue then stays partly open), or tiers 1 and 2 with the
  source gate (recommended, so the invariant is true as stated)?
- **D7.** A read refusal is recoverable (`LibrarianRecoverableError`, like the write door; `doc(get)` puts it in
  `body_error`) rather than fatal (`path_security`'s convention). Recommended: recoverable.
- **D8.** In-repo leaf links (including the monorepo case) stay readable (recommended), rather than the write door's
  refuse-any-leaf-link rule?
- **D9.** On Windows the door judges and then reads, with a stated race window, and stays unverified. Accept?

## Tests added

N/A: filed, not fixed. The probe above becomes the regression test when a fix lands (assert the marker is in neither the queue nor
the `doc(get)` output).

## Workarounds

Do not reindex a repository you do not trust with the librarian enabled; check `find <repo> -type l` first.

## Resume

Decide between the three options, then add the probe as a red test. Also run `doc(find)`, `librarian(context)`, `gather` and a
semantic search over a catalogued outward link, which this issue did not. **Done in Design (Phase 1): all of them leak. Next: the
operator's answers to D1-D9, then Phase 2 (red tests first).**

## References

- `docs/issues/2026-10-17-edit-file-on-a-leaf-symlink-to-a-deny-listed-path-reads-and-copies-the-file.md`
- `docs/issues/2026-10-17-doc-update-writes-through-a-catalogued-symlink-and-rewrites-a-file-outside-the-root.md`
