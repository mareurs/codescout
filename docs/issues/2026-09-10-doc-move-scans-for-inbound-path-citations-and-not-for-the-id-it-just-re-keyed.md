---
id: fbcd7b25cb5f45cf
kind: bug
status: open
title: 'BUG: doc(move) scans the repo for inbound PATH citations and not for the ID it just re-keyed, while instructing the caller to re-point both'
owners:
- marius
tags:
- cluster/unclassified
- librarian
- catalog
---

## Summary

`doc(action="move")` returns `inbound_path_citations` + `inbound_path_citation_count`, computed by
an actual repo scan (`files_mentioning`, `src/librarian/tools/mv.rs:262-265`) rather than from
`cites` edges — deliberately, and the comment at `:314-320` explains why the edge count would be
wrong by 4x.

It returns **no equivalent for the id**, even though the same call sets `id_changed: true` and
`previous_id`, and `artifact.rs:183` instructs the caller in so many words to *"re-point prose
citing the old one"*.

The two halves cost the same to compute: `files_mentioning(&root_path, stem, …)` takes a needle,
and `previous_id` is a 16-hex token that greps cleanly. One half is served; the other is stated as
prose and left to the caller's memory, with **no signal at all** when it is skipped.

## Symptom (Effect)

A prose citation of the old 16-hex id survives the move, resolves to nothing, and nothing reports
it. It is strictly quieter than the path half: `audit_doc_refs` scores a dead path-shaped token at
`high` and reds CI, and there is no id-shaped equivalent — so the unserved half is also the half
with no downstream backstop.

The reader who meets it later gets `unknown id`, which is a *correct* error about a *stale premise*
— indistinguishable from a typo, and it costs a re-derivation to tell apart.

## Reproduction

Both instances are from 2026-09-10, roughly forty minutes apart, by two sessions who had each read
the guide text that names the obligation:

1. sessionId `59112612-5fc8-4b31-8c8c-e19220d99eac` archived a bug file and had to hand-repoint
   `04aa6207d31a861f`'s citation of a re-keyed id (`2b1c3aaa9b09534d` → `d81efeef5252bfcc`).
2. sessionId `c86ebb51-7ae3-477d-b755-f25db6180782` (this author) archived
   `2026-09-09-the-pre-push-remedy-…`, acted on the returned `inbound_path_citations` list of four,
   re-pointed all four — and left `750c60a5135d52f9` live in prose, because no field named it. The
   move response had reported `id_changed: true` on the same screen.

Instance 2 is the informative one: the caller **did** read the response and act on it. What they
acted on was the field that existed.

## Environment

`experiments`, 2026-09-10. `src/librarian/tools/mv.rs`, current HEAD.

## Root cause

`citing_files` is derived from `old_full.file_stem()` only. Nothing in `call()` scans for
`previous_id`. The field family around it is unusually carefully built — `vectors_refile_error`
carried beside `vectors_refiled` because the pair disambiguates, `inbound_path_citation_count`
carried beside a capped list because a sample presented as a population is its own defect class —
which is what makes the omission worth filing rather than shrugging at: this is not a response
nobody thought about.

## Evidence

- `src/librarian/tools/mv.rs:262-265` — the scan, keyed on the file stem.
- `src/librarian/tools/mv.rs:324-330` — the two path fields and their rationale comments.
- `src/librarian/tools/mv.rs:274` — `"id_changed": grafted.is_some()`, in the same object.
- `src/librarian/tools/artifact.rs:183` — the schema text that names the id obligation and hands it
  to the caller.
- `grep 750c60a5` after instance 2's move returned a live prose hit; the equivalent path grep
  returned zero, because the path half had been served.

**Not measured:** how many stale id citations already sit in the corpus. A repo-wide scan for
16-hex tokens that no longer resolve would answer it and has not been run — stating a number here
would be inventing one.

## Hypotheses tried

- **`IC-18` (`selector-narrower-than-its-population`)?** Rejected, though it is the nearest. That
  class is about a selector under-covering *the population it names* — here the field is honestly
  named `inbound_path_citations` and covers path citations completely. The gap is a missing second
  field, not an under-reaching first one.
- **`IC-14` (`guard-narrower-than-its-name`)?** Rejected for the same reason, more sharply: the
  name is accurate. Tagging it there would corrupt the count a promotion threshold reads, which is
  the failure `cluster/unclassified`'s own `**Members:**` line exists to prevent.
- **`IC-22` (`hint-composed-without-the-request`)?** Rejected. The response is composed from what
  the move actually did; nothing about it presumes a caller state that does not hold.

Filed under the escape hatch: looked, nothing fits without forcing. The shape, offered for whoever
meets a second instance rather than claimed as a class — *a response serves the cheap half of a
two-half obligation and states the other half as prose, where both halves cost the same* —
provisional slug `half-an-obligation-served-and-half-narrated`.

## Fix

Not applied. Add `inbound_id_citations` + `inbound_id_citation_count` beside the path pair, from a
second `files_mentioning(&root_path, &previous_id_hex, &a.new_rel_path)`, and keep the same
`null`-means-scan-could-not-run convention the neighbouring fields already document.

**Do not fold the two into one list.** They are re-pointed differently — a path citation becomes
the new path, an id citation becomes the new id — and a caller sizing a commit needs them apart.

**Do not derive the id half from `cites` edges** for exactly the reason the path half does not: the
catalog indexes markdown only, so an id quoted in a `.rs` comment or a shell script can never
become an edge, and this repo's guard scripts do quote ids.

## Tests added

None — nothing is fixed. `mv.rs` already carries
`a_citation_scan_that_cannot_run_reports_null_not_an_empty_list` (`:627`) and
`the_citation_scan_sees_an_untracked_citing_file` (`:693`); the id half wants both shapes again,
plus one asserting the two lists stay **separate** on an artifact cited by path in one file and by
id in another. Note that a test asserting only *"the id list is non-empty"* would be monotone under
the two scans being merged, which is the fix this file rejects.

## Workarounds

After any `doc(action="move")` reporting `id_changed: true`, grep the repo for `previous_id` by
hand. It is one command and it is not optional; the response will not remind you.

## Resume

Implement the second scan in `mv.rs:262-265` and thread two fields into the `json!` at `:324`.

## References

- `src/librarian/tools/mv.rs` — the scan and the response object
- `src/librarian/tools/artifact.rs:183` — the schema text stating the obligation
- `docs/issues/archive/2026-09-02-tracked-only-staging-commits-half-an-archive-move.md` — the
  earlier half-a-move defect, whose remedy (`stage_together`) is the precedent for serving an
  obligation in the response rather than narrating it
