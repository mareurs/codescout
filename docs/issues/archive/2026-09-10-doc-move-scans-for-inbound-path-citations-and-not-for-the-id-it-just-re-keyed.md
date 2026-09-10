---
id: e23a819ab15aa2ee
kind: bug
status: fixed
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
it.

**The unserved population is PROSE ids specifically, and the narrower claim is the stronger one.**
An earlier draft of this file said a dead id has "no equivalent backstop", which is false of
*frontmatter* ids: `librarian(action="doctor")` reports `frontmatter_id_mismatch` and ships
`fix=repair_frontmatter_id` to repair it. Narrowed 2026-09-10 on sessionId
`59112612-5fc8-4b31-8c8c-e19220d99eac`'s correction — saying exactly which half nothing covers is
worth more than the wider claim.

**One re-key has three consequences, three observers, and only one fires without someone deciding
to look:**

| what goes stale | caught by | fires when |
|---|---|---|
| the old **path**, cited anywhere | `audit_doc_refs` — `policy_default` is `high` | CI, unprompted |
| the file's own **frontmatter id** | `doc(move)` rewrites it in the same call (`mv.rs:160`, test `move_rewrites_the_frontmatter_id_it_just_invalidated`); `doctor`'s `frontmatter_id_mismatch` catches rows the tool did not write | never needed after a `doc(move)`; a manual scan otherwise |
| the old **id**, cited in prose | nothing | at some later reader's call site, as `unknown id` |

That last one is a *correct* error about a *stale premise* — indistinguishable from a typo, and it
costs a re-derivation to tell apart.
## Reproduction

Both instances are from 2026-09-10, roughly forty minutes apart, by two sessions who had each read
the guide text naming the obligation:

1. sessionId `59112612-5fc8-4b31-8c8c-e19220d99eac` hand-repointed `04aa6207d31a861f`'s citation of
   a re-keyed id (`2b1c3aaa9b09534d` → `d81efeef5252bfcc`). **A near-miss with a documentation
   backstop, not a catch by the tool** — their own sharpening, and it matters: they swept for the
   old path *and* the old id because `get_guide("tracker-conventions")` says to sweep both. A
   session that had not read that sentence would have missed it exactly as instance 2 did. A guide
   sentence is the weakest possible mechanism, which strengthens this filing rather than softening
   it.
2. sessionId `c86ebb51-7ae3-477d-b755-f25db6180782` (this author) archived
   `2026-09-09-the-pre-push-remedy-…`, acted on the returned `inbound_path_citations` list of four,
   re-pointed all four — and left `750c60a5135d52f9` live in prose, because no field named it. The
   move response had reported `id_changed: true` on the same screen.

Instance 2 is the honest one: the caller **did** read the response and act on it. What they acted
on was the field that existed.
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

Applied. `citing_ids` is a second `files_mentioning(&root_path, &a.id, "")` beside the existing
path scan in `src/librarian/tools/mv.rs`, surfaced as `inbound_id_citations` +
`inbound_id_citation_count` next to their path twins, with the same
`null`-means-the-scan-could-not-run convention and the same `CITATION_SAMPLE` cap.

**The three design constraints this file recorded before the fix existed, all held:**

- **The lists stay separate.** They are re-pointed differently — a path citation becomes the new
  path, an id citation becomes the new `id` from the same response — and a caller sizing a commit
  needs them apart.
- **Not derived from `cites` edges.** The catalog indexes markdown only, so an id quoted in a
  `.rs` comment or a shell script can never become an edge, and this repo's guard scripts quote
  ids.
- **The open question is answered, and the answer is NO.** The id scan does **not** inherit the
  path scan's self-exclusion. The path scan excludes the new path because a file carrying its
  former *slug* in a superseded note cites itself, which is not work. For the id the reverse
  holds: this same call has already rewritten the moved file's frontmatter `id:`
  (`repair_frontmatter_id`), so the only way the new path can still match `previous_id` is that
  its **body** names it — a dead 16-hex token in prose, which is precisely the population nothing
  else catches. Excluding it would hide the one hit that is always genuine.

**Three read surfaces changed in the same commit, because leaving them narrating the obligation
would reproduce the defect one layer up.** `artifact.rs`'s `new_rel_path` schema text said *"re-point
prose citing the old one"*; it now names both fields and their `null` convention.
`src/prompts/guides/librarian.md` § *Archiving / Moving Trackers* and
`src/prompts/guides/tracker-conventions.md` both said `id_changed: true` **is the signal** — true
before, and now the weaker of two available statements. Both name the list instead.

Fix SHA: `38f7490e` (experiments)
Patch-id: `6f187107b6d04b736366539ed340fca1cf15ccf4`

**Dogfooded at archive time, and the result is worth keeping:** this file's own archive move
reported `inbound_path_citations: ["src/librarian/tools/mv.rs"]` and **no id field at all** — the
live MCP server is the pre-fix binary (`cargo rb` + `/mcp` not yet run), so the id sweep for this
very move was done by hand, which is the manual step the fix removes. One citation, found by
`grep`, re-pointed in the same commit.
## Tests added

Three, in `src/librarian/tools/mv.rs`, each with an observed RED against the **production** path.

- `the_id_scan_reports_null_when_it_cannot_run` — mirrors the path twin deliberately: the two
  fields are read as a pair, so a caller told `null` means *unasked* for one and handed `[]` for
  the other has been given two different contracts by one response.
- `the_two_citation_lists_stay_separate` — the fixture is built so **neither citer can satisfy the
  other's assertion**: one file mentions only the stem, the other only the 16-hex id, and neither
  string occurs in the other file. `git grep -F` is a plain substring match, so the id-citer says
  *"the tracker"* rather than naming the file — any mention of the stem there would put it in both
  lists for an honest reason and destroy the discrimination. Annotated on the line.
- `the_id_scan_reports_the_moved_files_own_body_citing_its_old_id` — pins the exclusion decision,
  and carries **a control**: it also asserts the moved file's frontmatter no longer contains the
  old id. Without that, a regressed `repair_frontmatter_id` would make the test pass for the wrong
  reason and read as coverage of a decision it was no longer testing.

**Observed RED, 2026-09-10, one mutation per claim, restored in the same command so no window
stayed open:**

| mutation of the production line | red |
|---|---|
| `.or(Some(Vec::new()))` — collapse `null` to an empty list | `the_id_scan_reports_null_when_it_cannot_run` |
| `let citing_ids = citing_files.clone()` — merge the two lists | `the_two_citation_lists_stay_separate` **and** the self-body test |
| `files_mentioning(…, &a.id, &a.new_rel_path)` — inherit the exclusion | the self-body test |

Control re-run after restore: 21 passed, 0 failed, file byte-identical to the original. The merge
mutation killing **two** tests is honest rather than redundant — merging both mixes the lists and
re-imports the path scan's exclusion, so each test names a different thing that broke.

**Mutated in place rather than on a relocated copy, and the trade is stated because the opposite
call was made earlier the same day.** A shell guard can be *live wrong* for the seconds it is
patched; a `cargo test --lib` mutation changes no committed bytes, does not rebuild
`target/debug/codescout` (which `tests/cli_doc.rs` resolves at run time), and each window was one
second of test runtime. A separate worktree would have cost a full cold build to remove a hazard
that is not the same hazard.
## Workarounds

After any `doc(action="move")` reporting `id_changed: true`, grep the repo for `previous_id` by
hand. It is one command and it is not optional; the response will not remind you.

## Resume

Nothing owed. The open question in `## Fix` is answered and its answer is pinned by a test with a
control.
## References

- `src/librarian/tools/mv.rs` — the scan and the response object
- `src/librarian/tools/artifact.rs:183` — the schema text stating the obligation
- `docs/issues/archive/2026-09-02-tracked-only-staging-commits-half-an-archive-move.md` — the
  earlier half-a-move defect, whose remedy (`stage_together`) is the precedent for serving an
  obligation in the response rather than narrating it
