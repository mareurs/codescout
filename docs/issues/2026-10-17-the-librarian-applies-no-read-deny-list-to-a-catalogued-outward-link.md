---
status: fixed
opened: 2026-10-17
closed: 2026-10-18
severity: high
owner: marius
related: [docs/issues/2026-10-17-edit-file-on-a-leaf-symlink-to-a-deny-listed-path-reads-and-copies-the-file.md, docs/issues/2026-10-17-doc-update-writes-through-a-catalogued-symlink-and-rewrites-a-file-outside-the-root.md]
tags:
- cluster/unclassified
kind: bug
unverified: 'macOS not run (it takes the fallback path: F_GETPATH not wired) and Windows unverified: no inode check there, so the door judges then reads with a race window (D9). On the fallback path (no /proc, macOS, other Unix) a PARENT directory swapped inside for the judgement and back afterwards is still not refused (review round 1, F2). Not covered by design: a hard link or bind mount placed inside the repository. Kept rows keep their slug and citation edges, and a cascaded entry_cite delete keeps its row_id in the audit trail (Design deviation 8, operator decision open). Audit shard lines exported before a removal, text an embedder received, and freed SQLite pages are not recalled. Rows a pre-fix walk catalogued through an outward link still show a stored title in doc(list_stale), the worktree reports and tracker_design until the next reindex of their project clears or reclaims them. Config-file TOML errors that quote a line were not exercised. The doctor cost on the live catalog is estimated, not measured on it. The external-store vector delete is tested with the in-memory store only.'
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

**Applied: see Fix (Phase 2) below.** The three options the issue was filed with (Phase 1 weighs them and a fourth, which
was chosen):

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

## Fix (Phase 2)

Branch `worktree-agent-a8918c1498fdd30e8`, on `experiments` `7a09623bd` (rebased onto `0a8af94d7` for review round 1). The operator answered D1-D9 with the
recommendations, and D4 with "clear everything derived from the outside file".

| commit | patch-id | what |
|---|---|---|
| `eae21e694` | `7bfbbd10c5a0a4be4d7dfeb2edb1eaa31f8ae125` | the read door (`src/librarian/read_door.rs`), its unit tests, a read column in the write door's layout tables |
| `acba669d6` | `b4e72fc7b87485a9d8fe8c39968ee3ad5d85d751` | the walk, reclamation (keep and clear), backfill, and the reindex report |
| `bbdfca690` | `23d2abc7aef8f0dcb9c72f8ec90ab24b94b0c961` | every other read site, tiers 1 and 2, and the site table `tools/read_door_tests.rs` |
| `ebb3d5715` | `394c11fb30be5535bed536a111fb811c7a46729e` | the source gate |
| `e1f5d3ad4` | `fea062f37642cd1fed4661de84e9c2b5966d10ca` | two tests strengthened after their mutants survived (below) |
| `2903c3747` | `b24957d48be5ee8443eb1fc14136095779b4f44f` | a clippy fix to the test hook (the gate's clippy lane caught it) |

### The invariant (stated in `read_door.rs` and here)

*The librarian hands the bytes of a file that a repository or the catalog names (to a caller, to the chunk table or
to an embedder) only if the file, AS OPENED, lies inside the root `util::door_root` names for its spelling and is not
on the read deny-list. When no root can be named, it reads only a spelling that crosses no symbolic link at all.*

The root is the write door's root, so the librarian reads only where it would write. The read door adds no second
notion of inside: the write door's three layout tables (`door_layout_table`, `_round6`, `_round7`, 71 rows) now
carry a read column that must equal the file door on every row, and it does. The one deliberate difference is the
leaf rule: a leaf link that resolves inside is read, because its bytes are the repository's own; a write never goes
through a leaf link.

How a read works (`ReadDoor::bytes`): `metadata` must say regular file (a FIFO would block an open); open once with
`O_NONBLOCK`; `fstat` the handle (regular file again); canonicalize the path; judge THAT path against
`door_root(roots, spelling)` and the deny-list (`validate_read_path` with `PathSecurityConfig::default()`, so the
repository-settable `[security] profile` cannot switch it off, decision D3); `stat` the canonical path and require
the opened file's `(st_dev, st_ino)`; read from the descriptor, never reopening. A refusal is a
`LibrarianRecoverableError` naming the path and the reason, never content (D7). Stored bytes (chunk text, a title)
are guarded by `ReadDoor::refusal`, the same judgement without an open; a dangling leaf link is refused there.
Agent-named `params_path` gets the deny-list only (`read_agent_named_text`, D5).

### What every site does now

| site (Phase 1 path table) | now |
|---|---|
| #1 walk | refused file skipped, path reported in `escaping_skipped_paths`; an outward file link is no longer catalogued (D1) |
| #2 reclamation | judged by the same door; a refused row without history is reclaimed (its chunk text and vectors go with it), one with history is KEPT and cleared (`clear_escaping_row`): title, kind and status (`unknown`), owners, tags, topic, time scope, content hash, mtime, embedded stamp, every chunk row, and the store's vectors are dropped (D4). It still shows its id, path, slug, augmentation and events |
| #3, #4 backfill | the disk read goes through the door; stored chunk text of a refused row is not re-embedded; both counted in `refused_by_read_door` |
| #7 `doc(get)` | refusal in `body_error`; stored title, owners, tags, topic and time scope withheld |
| #8, #9 `librarian(context)` | a refused row is left out of the pack |
| #10-#13 `doc(gather)` | `current_body`, `file`, `config_value` through the door; `grep` judges its root and reads every file through the door |
| #14, #15 `doc(find)` | snippet and stored title withheld for a refused row, with the reason in `withheld` (also `gather` `artifacts`) |
| #16 `params_path` | deny-list only, open-then-verify read |
| #17 read-modify-write verbs | `ReadDoor::text_for_write`: the write door's leaf rule, then the read door, BEFORE the read (`doc(update)`, the frontmatter field writer, `append_entry`, `allocate_entry_id`, the snapshot resync, rekey, the export stamp, the id repair). `doc(move)` reads through the door and refuses a row whose file resolves outside its repository before anything moves |
| #18 doctor | every scan reads through a door on `DoctorScope` (managed roots) or one passed in |
| #19-#25 | `link_scan`, the sidecar readers (`declared_target` now refuses instead of answering "nothing declared"), `requirements.toml` and its `tag_values_from` files, the `external_prefix` authority (a refused authority is a finding, not a read), the `audit_doc_refs` resolver, and the config loaders (`librarian.toml`, `project.toml` for `force_include`, chunk grain and backend, `workspace.toml` umbrellas) through `read_door::for_project` |

Every remaining raw read in `src/librarian` production code is in the source gate's allow-list
(`no_librarian_production_read_bypasses_the_read_door`) with its reason: the global `workspace.toml`, the session
registry, `/etc/hostname`, `/proc/<pid>/stat`, git's worktree pointer files, and the audit shard reads (out of scope;
filed as `2026-10-18-the-audit-shard-readers-follow-a-link-a-repository-commits-under-codescout-audit.md`).

### The layout table (`read_door_site_table`), every row and its verdict

Each row is run through 16 read sites (walk, get, context, context entry anchor, gather body, gather file, gather
grep, config_value, find snippet, find title, backfill disk, backfill stored, doctor, link_scan, requirements,
external authority) plus `doc(update)`. The table asserts that every site gives the row's verdict, that the verdict
equals the write door's containment answer on every comparable row, and that `doc(update)` reads only what both
doors admit.

| row | layout | verdict | write door: containment / full | cells that cannot observe the row (and why) |
|---|---|---|---|---|
| R1 | plain file in the project | ADMIT: every site and `doc(update)` return the marker | admit / admit | none |
| R2 | leaf link, absolute target outside the repository | REFUSE: no site, no `doc(update)` | refuse / refuse | none |
| R3 | leaf link, relative target outside | REFUSE | refuse / refuse | none |
| R4 | directory link out of the repository, reached by `force_include` | REFUSE | refuse / refuse | none |
| R5 | leaf link to a file inside the repository | ADMIT; `doc(update)` refuses (leaf rule) | admit / refuse | walk (a second name for a walked file), grep (never follows a leaf link) |
| R6 | directory link that stays inside | ADMIT | admit / admit | none |
| R7 | monorepo: leaf link out of the project, inside its git root | ADMIT; `doc(update)` refuses (leaf rule) | admit / refuse | grep (leaf link) |
| R8 | nested repository whose directory link points into its superproject (F2) | REFUSE (D2) | refuse / refuse | none |
| R9 | dangling leaf link | NO BYTES, and stored text is withheld | admit / refuse (not comparable: no target to contain) | none |
| R12 | no managed root, no repository: leaf link out | REFUSE | admit / refuse (not comparable: the write door has only its leaf rule here) | gather file, grep, config_value (paths join the server's cwd), link_scan (reports no cross-repo token without a project) |
| R13 | no managed root, no repository: plain file | ADMIT | admit / admit | same four |
| R14 | no managed root, no repository: directory link out | REFUSE | admit / admit (not comparable: the write door cannot judge a parent link here) | same four |

Deny-list rows (child process with `HOME` set): a target in `<home>/.ssh/` inside a managed root that IS the home,
so containment admits; all 16 sites, `doc(update)` and `params_path` refuse. Config rows
(`repository_config_files_are_read_only_through_the_door`): six loaders, each a plain file and an in-repo link (take
effect) and an outward link (refused).

### Tests seen red first, and pins

Measured before any fix (logs kept in the session): the site table had 97 failing cells, with at least one red cell in
every site; the deny-list child had all 16 sites, `doc(update)` and `params_path` returning the marker; the config
table had all 6 outward-link loaders taking effect; `a_file_replaced_by_an_outward_link_is_reclaimed_with_its_stored_text`,
`an_outward_row_with_history_is_kept_and_cleared_of_everything_its_target_supplied` and
`a_row_below_a_directory_swapped_for_an_outward_link_is_kept_and_cleared` failed. Green from the start (pins):
`params_path_still_reads_a_json_file_outside_the_repository` (D5), and the walk cells of R4 and R14 (a directory link
reached by `force_include` was already refused by round 2). The door's unit tests, the read column and the gate
tests were compile-red only (new API), so each was mutation-checked instead (below).

Existing tests changed by the behaviour change, each saying why in its doc: two indexer tests (outward file links no
longer indexed, now asserted as reported), `link_write_tests` (rows built by indexing a regular file and then
swapping it for a link, the only way such a row exists now; the move is refused; the frontmatter-id sweep cannot
read the target and reports nothing), six `link_dir_tests` move tests (leaf-link moves now use in-repo targets; a
directory or a link to one is refused as "not a regular file").

### Mutation probe (`scripts/mutation-probe.sh`, this worktree's own pool `…agent-a8918c1498fdd30e8.worktrees/mutation-slot-0`)

42 mutants, all KILLED in the end. Door: containment dropped; deny layer dropped; the inode check's call removed;
the check reduced to `st_dev`; the spelling judged instead of the canonical path; both regular-file checks removed;
the dangling-link refusal dropped; the no-root rule dropped; `for_project` without the git root; the verdict memo
keyed on the resolved path alone. Walk and catalogue: the walk's door replaced by a raw read; the reclaim predicate;
the kept-row clear; the store delete for kept rows; both backfill arms. Sites: get, context (both), gather (body,
file, grep root and files, config_value), the find predicate, `params_path`, `doc(update)`, the doctor scope read and
the doctor frontmatter-id read, `link_scan`, the `requirements` values, the external authority, six config loaders,
`doc(move)`, the sidecar refusal, the resync refusal. Gate: a raw read added to a production file; a second read
appended to an allowed line.

Two survived their first run and were killed after the test was strengthened (commit 5): the doctor scope read made
raw (no scan using `DoctorScope::read_text` echoed content for the probe row, so the row is now a terminal bug with
an `unverified:` caveat), and a second read on an allowed line (the gate counted lines; it now counts reads). Not
probed, with the reason: comparing only `st_ino` (equivalent on a single filesystem, which every fixture is); the
`O_NONBLOCK` flag and each regular-file check alone (the `metadata` pre-check and the handle check each cover the
other except under a concurrent swap, which no deterministic test makes; removing both is killed).

### Guide and prompt surfaces

No guide or prompt text changed (`git diff 7a09623bd -- src/prompts docs/manual` is empty), so the three margins the
brief named are where they were: `a_p50_session_stays_under_the_committed_emission_byte_ceiling` 12,228 of 12,244 B,
the bug-archive move 13,776 of 13,886 B, the librarian reference section 2,486 of 2,500 B (both emission tests pass in
the gate). The new behaviour explains itself where it happens: the refusal's hint, the reindex `escaping_note`
(rewritten), and `withheld` in `doc(find)`.

### Cost, measured

Release build, 500 files: the first read of a file through a fresh door costs 161 µs (the judgement and the deny-list's
~40 canonicalizations), every later read of the same file through the same door 19.6 µs, a raw read 4.5 µs. A
`doctor` run builds one door for the run. On the live catalog (6,223 rows, about 20 scans reading each) that is an
estimated +1 s for the first reads and +2 s for the rest; estimated, not run on the live catalog.

### Deviations from the Phase 1 design

1. **No-root rule is stricter**: the design said "reads only a path whose leaf is not a link"; the door reads only a
   spelling that crosses NO link (leaf or parent). Without it a row with no managed root and no repository above it,
   below a directory link out, was read (layout R14), while the walk refused it.
2. **`doc(move)` refuses a row whose file resolves outside its repository.** Round 3 allowed moving an outward link (a
   move moves the link); a move reads the content to re-key the row, which the read door refuses. An in-repo leaf link
   still moves.
3. **Stored titles are withheld** by `doc(get)`, `doc(find)` and `gather`'s `artifacts` for a refused row, not only
   the snippet. Other surfaces that print a stored title (`doc(list_stale)`, the worktree and `merge_worktree` reports,
   `tracker_design`) still print one for such a row until the next reindex of its project clears or reclaims it.
4. **Read-modify-write verbs** use the write door's leaf rule plus the READ door (whose containment equals the write
   door's, as the table asserts), not the write door's own containment call.
5. **A verdict memo** per `(spelling, resolved path)` inside a door, for `doctor`'s cost.
6. **Project-root readers** (`for_project`) judge against the project's git root and the project, the active-project
   part of `managed_roots`; judging against the project alone disagreed with every other site on the monorepo row R7.
7. **The reindex deletes store vectors for reclaimed escaping rows too**, not only kept ones. The MCP reindex never
   deletes them for ordinary vanished rows; that is filed separately
   (`2026-10-18-librarian-reindex-drops-a-vanished-rows-catalog-entry-but-never-its-external-store-vectors.md`).
8. **Kept rows keep their slug** (immutable once minted; `entry_cite.src_slug` references it) **and their citation
   edges**. D4 asked to clear everything derived from the outside file: a slug minted from a title that a pre-fix walk
   read through an outward link, and `cites` edges a pre-fix `link_scan` derived from its text, survive. They carry at
   most a 50-character slug and entry tokens. **Decision needed: clear them too (re-keying citations), or accept.**
9. **`doctor fix=repair_frontmatter_id` no longer lists an outward row** (it cannot read the target's `id:`); nothing is
   written either way.
10. **`gather` with no active project** joins paths to the server's absolute working directory (it used `.`), so the
    door can judge them; what is read is unchanged.

## Review round 1 (Opus review of `c6d235ffa`): the audit trail, a swap back, a huge file, wording, the gate

The reviewer read code and ran nothing; every item below was reproduced RED by a test before its fix.

- **F1 (high), the clearing moved the text into the audit trail.** `artifact` is audited: the UPDATE trigger
  stores `[old, new]` of every changed column, the DELETE trigger the whole old row, and the reindex exports
  `catalog_audit` to the committed shards. Measured red on the real pre-fix state (below): `catalog_audit.payload:
  {"slug":[null,"fakekeymarker-title"]}` (the slug minted from the target's title when it was first indexed) and
  `{"kind":["doc","unknown"],...,"title":["fakekeymarker title",null],...}` (the clear), and for a reclaimed row the
  whole delete image with the title. **Invariant: removing what the read door refuses never copies it.** When a row
  is reclaimed or cleared because its file resolves outside its repository, every audit payload of that row loses the
  values of the columns its file supplied (`audit::ARTIFACT_FILE_DERIVED`: title, kind, status, owners, tags, topic,
  time scope, file hash, mtime, embedded hash, slug), replaced by `{"elided": "read-door"}`, in the same savepoint as
  the removal and before the export. Keys and routing keys (`id`, `abs_path`) stay, so the export still attributes the
  row and a reader sees which columns changed. Historic rows of that id are elided too (the slug-mint row above
  predates the removal; the trail cannot tell an earlier legitimate value from a target-derived one). **This rewrites
  rows of an append-only trail**, narrowly: no schema, trigger or shard-format change, no row deleted, no `seq` gap,
  and the stand-in says why the value is absent. **Not recallable:** shard lines exported before the removal (committed,
  possibly pushed), text an embedder already received, rows other catalogs hold, and SQLite pages freed but not reused
  (`catalog.db` and its WAL are not scrubbed at the byte level). Cascaded `entry_cite` delete rows keep their `row_id`
  (`slug:local→dst_ref`), which can carry the slug; that is Deviation 8's territory and is left open.
- **F6, the tests could not see F1.** `indexed_then_swapped` indexed `# plain`. The keep, reclaim and directory-swap
  tests now build the real pre-fix state (`prefix_state`: the target's text indexed, the precondition asserts the
  title, the chunks and the catalog hold the marker, then the file or its directory is swapped for an outward link)
  and assert that no table (every text value of every table, `catalog_audit` included, the kept row's own `slug`
  excepted for Deviation 8) and no exported shard holds it, and that the export ran. Red before the fix (the
  payloads above), green after; the kept-row test also reindexes twice.
- **F2 (medium), a swap there and back.** `same_file` used `metadata`, which follows links. Seam test with a second
  hook (after the judgement): the leaf opened as an outward link, swapped to an inside file for the judgement, swapped
  back before the inode check; red: `the door returned "FAKE-KEY-MARKER"`. A `symlink_metadata` check alone does not
  close the same race through a PARENT directory (`root/d` an outward link at the open, a real directory for the
  judgement, the link again at the check; red the same way). Fix: on Linux the door judges the descriptor's own path
  (`/proc/self/fd/N`), the opened inode's real location; the inode check compares the canonical path's own inode
  (`symlink_metadata`). Both flip-flops and the leaf flip-flop on the fallback path (no `/proc`, forced by a test
  knob) are refused. **The parent flip-flop on the fallback path (macOS, other Unix, Linux without `/proc`) is still
  open**, stated in the module doc; macOS `F_GETPATH` is not wired and was not run.
- **F3 (low), a huge file aborted the process.** Child-process test with a sparse 4 EiB file: red `ExitStatus(134)`
  (SIGABRT from `Vec::with_capacity`). Now `try_reserve_exact`, an `OutOfMemory` I/O error, as `std::fs::read` does.
  A file that IS allocatable is still read whole (no size cap, as before the door).
- **F4 (low), the refusal named the target.** Red: `it resolves to <base>/out/secret-place-id_rsa, outside the
  repository root ...`. Now the read door names the spelling and the repository root only, for the outside, the
  no-root and the deny-list refusals. **Not changed:** `util::refuse_path_outside_root` (the write door) still names the
  resolved path; left for the coordinator, as instructed.
- **F5 (medium), the gate missed equivalent spellings.** Red: `the gate does not count std::fs::File::options()
  .read(true).open(p)`. The gate now also counts `File::options(`, `io::read_to_string(` and every `use` item that
  imports a reading function or type from `std::fs`, `tokio::fs` or `std::io` (grouped root imports included) or
  aliases the module (`as`, `*`), on the line it starts. One probe per shape plus controls (`use std::fs;`,
  `use std::io::Read;`, `read_dir`, `metadata`). No production read was found by the wider net.

**Mutation probe, round 1** (this worktree's own pool, `…agent-a8918c1498fdd30e8.worktrees/mutation-slot-0`), 19
mutants. Killed: both elision calls (kept, reclaimed); `elide_values` never rewriting; it rewriting any row of the table;
`slug` and `title` dropped from the derived columns; no savepoint; the descriptor path disabled; the inode check
following links again; the infallible allocation; the outside refusal naming the target; the five gate holes
(`File::options(`, `io::read_to_string(`, imports, grouped imports, `as`). Two survived their first run and were killed
after a test was added: the savepoint releasing instead of rolling back (`a_failed_elision_rolls_the_removal_back`, a
unit test of `in_savepoint`) and the no-root refusal naming the target (the wording test now reaches the no-root
branch through a spelling no repository contains). **Survived, equivalent:** dropping the `is_absolute` check on the
descriptor path (`readlink` of `/proc/self/fd/N` for an open regular file is always absolute; the check guards a
pseudo-file name such as `anon_inode:`, which a regular-file descriptor never has).

**Unverified after round 1:** the fallback-path parent flip-flop (open, by construction); macOS and Windows not run;
the byte-level residue in `catalog.db` and its WAL; the `entry_cite` `row_id` of a cascaded delete.

## Fix provenance

Branch `worktree-agent-a8918c1498fdd30e8` (not yet on `experiments`).

- **SHA:** `eae21e694` (the read door)
- **patch-id:** `7bfbbd10c5a0a4be4d7dfeb2edb1eaa31f8ae125`
- **SHA:** `acba669d6` (walk, reclamation, backfill)
- **patch-id:** `b4e72fc7b87485a9d8fe8c39968ee3ad5d85d751`
- **SHA:** `bbdfca690` (every other read site)
- **patch-id:** `23d2abc7aef8f0dcb9c72f8ec90ab24b94b0c961`
- **SHA:** `ebb3d5715` (source gate)
- **patch-id:** `394c11fb30be5535bed536a111fb811c7a46729e`
- **SHA:** `e1f5d3ad4` (tests strengthened after two surviving mutants)
- **patch-id:** `fea062f37642cd1fed4661de84e9c2b5966d10ca`
- **SHA:** `2903c3747` (clippy fix to the test hook)
- **patch-id:** `b24957d48be5ee8443eb1fc14136095779b4f44f`
- **SHA:** `6bf7b1d78` (a Unix-only test helper)
- **patch-id:** `80ef542f1f8aaef45a7656a447702174eb2c4941`
- **SHA:** `dbc44f403` (review round 1, F1 and F6: the audit trail)
- **patch-id:** `b5d252c89da38ed0a0748c27e79e9f169aa7bd0d`
- **SHA:** `c34a3ed7c` (review round 1, F2-F4: descriptor path, fallible read, refusal wording)
- **patch-id:** `62059f8abf07ac3131048e1442a6e533ac9a2aec`
- **SHA:** `d6a2b2124` (review round 1, F5: the gate counts every spelling of a read)
- **patch-id:** `f7d47519e2b07950845bd6a421cbb803ec1b7b2e`
- **SHA:** `f13905660` (review round 1: tests for the two surviving mutants)
- **patch-id:** `8796cd0f5bea62b2e9a6fcd09f459225bb4c7245`

(Commits rebased onto `experiments` `0a8af94d7` on 2026-10-18; the Phase 2 SHAs above are the
rebased ones, their patch-ids unchanged.)

## Tests added

See Fix (Phase 2): `src/librarian/tools/read_door_tests.rs` (`read_door_site_table`, the deny-list child
`deny_listed_target_inside_a_managed_root_reaches_no_read_site`, `repository_config_files_are_read_only_through_the_door`,
the three reclaim/keep tests, the `params_path` pin), `src/librarian/read_door.rs` tests (inode check through its
seam, the swap between open and check, non-regular targets, the no-root rule, the stored-bytes predicate, the
source gate and its lexer test), and the read column of the three write-door layout tables.

## Workarounds

None needed after the fix. Before it: do not reindex a repository you do not trust with the librarian enabled; check
`find <repo> -type l` first. Text a remote embedder received before the fix cannot be recalled.

## Resume

Phase 2 is done (Fix (Phase 2)). Open: the operator's decision on Design deviation 8 (kept rows keep their slug and
citation edges). Follow-ups filed: `2026-10-18-librarian-reindex-drops-a-vanished-rows-catalog-entry-but-never-its-external-store-vectors.md`,
`2026-10-18-the-audit-shard-readers-follow-a-link-a-repository-commits-under-codescout-audit.md`.

## References

- `docs/issues/2026-10-17-edit-file-on-a-leaf-symlink-to-a-deny-listed-path-reads-and-copies-the-file.md`
- `docs/issues/2026-10-17-doc-update-writes-through-a-catalogued-symlink-and-rewrites-a-file-outside-the-root.md`
