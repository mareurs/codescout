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
unverified: 'Measured: the indexer catalogs an outward link to a deny-listed file, queues its TEXT for embedding, and doc(get) returns it. NOT run: doc(find), librarian(context), gather, and the semantic-search hydration path (the chunk table holds line ranges, not text, so what a semantic hit returns for a catalogued outward link was read, not run). Whether the configured embedder is remote on a given install was not checked.'
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
receives the text at index time.

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

## Tests added

N/A: filed, not fixed. The probe above becomes the regression test when a fix lands (assert the marker is in neither the queue nor
the `doc(get)` output).

## Workarounds

Do not reindex a repository you do not trust with the librarian enabled; check `find <repo> -type l` first.

## Resume

Decide between the three options, then add the probe as a red test. Also run `doc(find)`, `librarian(context)`, `gather` and a
semantic search over a catalogued outward link, which this issue did not.

## References

- `docs/issues/2026-10-17-edit-file-on-a-leaf-symlink-to-a-deny-listed-path-reads-and-copies-the-file.md`
- `docs/issues/2026-10-17-doc-update-writes-through-a-catalogued-symlink-and-rewrites-a-file-outside-the-root.md`
