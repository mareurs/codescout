---
id: 4a154d7effff259b
kind: bug
status: fixed
title: 'BUG: the delete dry run tells you an untracked file is git-restorable'
tags:
- cluster/hint-composed-without-the-request
closed: 2026-09-24
opened: 2026-09-21
owner: marius
related:
- docs/issues/archive/2026-09-21-the-delete-preview-omits-the-entry-cite-rows-its-cascade-destroys.md
severity: medium
---

# BUG: the delete dry run tells you an untracked file is git-restorable

## Summary

`doc(action="delete")`'s dry-run response carries a `recoverable` field asserting *"the file
is git-tracked and restorable"*. The string is an unconditional literal — nothing in the
delete path queries git, the index, or even whether the file exists. On an artifact that was
never committed, the preview's one reassurance about the irreversible half of the call is
false, and the force-delete that follows destroys the only copy.

## Symptom (Effect)

Measured 2026-09-21, tree `d155a8f6bb938b769a0a6fd0242ffc7e60a25f77`, on a throwaway artifact
created minutes earlier and never staged:

```
git status --short -- docs/scratch-571eb3d6-ledger.md docs/scratch-571eb3d6-target.md
?? docs/scratch-571eb3d6-ledger.md
?? docs/scratch-571eb3d6-target.md

git ls-files --error-unmatch docs/scratch-571eb3d6-ledger.md
error: pathspec 'docs/scratch-571eb3d6-ledger.md' did not match any file(s) known to git
Did you forget to 'git add'?   (exit 1)
```

The dry run on that same file, verbatim:

```
"recoverable": "the file is git-tracked and restorable; the augmentation, events, links
                and observations are catalog-only and are not"
```

Both clauses are load-bearing and the first is false. The sentence's whole rhetorical shape
is a **contrast** — *this part you can get back, that part you cannot* — so it does not merely
omit a caveat, it actively directs the reader's caution away from the file.

## Reproduction

Tree `d155a8f6bb938b769a0a6fd0242ffc7e60a25f77`, branch `experiments`, 2026-09-21T16:20Z.
Reproduced here from scratch, not relayed:

1. `doc(action="create", kind="tracker", rel_path="docs/scratch-<x>-ledger.md", title=…, body=…)`.
2. Do **not** `git add` it. Confirm with `git ls-files --error-unmatch <path>` → exit 1.
3. `doc(action="delete", id=<id>)` → the `recoverable` line above.

Deterministic: there is no input for which the sentence differs (see Root cause).

## Environment

codescout MCP over stdio, profile `~/.claude-kat`, branch `experiments`, Linux.
Session `571eb3d6-c879-43f6-b3f9-5a51e744e1af`.

## Root cause

**Measured** (above) and **read at the bytes**:

- `src/librarian/tools/delete.rs:103-104` — the field is a string literal inside the `json!`
  macro, with no interpolation and no surrounding conditional:

  ```rust
  "recoverable": "the file is git-tracked and restorable; the augmentation, \
                  events, links and observations are catalog-only and are not",
  ```

- `grep -i git src/librarian/tools/delete.rs` returns **five** hits and **not one is a
  query**: the rationale comment (:82), this literal (:103), the test doc-comment repeating
  the same claim (:373), and two `git_root:` struct-field initialisers in test fixtures
  (:433, :542). There is no `git ls-files`, no index read, no `Repository` handle, nothing.

The sentence is composed from the **gate's own design rationale** rather than from the state
it describes. That rationale is written out at `src/librarian/tools/delete.rs:78-84`: *"the
catalog delete cascades … and those are CATALOG-ONLY. `reindex` rebuilds the row from the
file, but nothing rebuilds an augmentation's params or an event log, and neither is in git. So
the FILE is recoverable and the HISTORY is not, which is the asymmetry a caller cannot see
from the id alone."* The reasoning is sound **for a committed artifact**, which is the
population the author had in mind; the response then states it as a fact about whichever
artifact the caller named.

**The untracked case is not exotic — it is the modal case for the callers `delete` was built
for.** `doc(action="delete")` is the sanctioned way to remove a scratch artifact, and the
briefing convention for probe artifacts in this repo is explicitly *create it, delete it,
never commit it*. So the artifacts most likely to reach this preview are precisely the ones
for which its assurance is false.

## Evidence

### Nothing in the suite can catch it

The claim is a constant, so no input makes it wrong-for-this-call versus right-for-that-one;
there is no predicate to mutate. `src/librarian/tools/delete.rs:373` restates the claim in the
test module's own doc-comment, so the file's test surface *shares* the belief rather than
checking it. Per `CLAUDE.md` § *Testing Discipline* — *a suite tests a guard's PREDICATE and
never its REMEDY TEXT* — this is the untested half by construction, and here the text is not
even a remedy but a **factual claim about state**, which is worse: a wrong remedy wastes a
call, a wrong reassurance authorises a destructive one.

### The answerability test the same section prescribes

Ask what would have to be true for the sentence to be a fact. Two things: the path is tracked
in the index or in some reachable commit, **and** the working-tree copy is not the only version
of its current content. Neither is consulted. A tracked-but-dirty file is a third state the
sentence also mis-describes — `git checkout` restores the committed bytes, not the ones being
deleted — and that state is invisible to a `ls-files` check alone.

### Not a stale-doc case

`src/librarian/catalog/mod.rs` and the delete path have not changed the claim's truth
conditions; the string was never conditional. This is not prose that decayed
(`cluster/doc-contradicted-by-code`), it is a response field that never consulted its subject.

## Hypotheses tried

1. **Hypothesis** — the line is conditional on tracked state somewhere upstream and the
   probe artifact happened to be misclassified. **Test** — `grep -i git` over
   `src/librarian/tools/delete.rs`; read the `json!` block at :91-106.
   **Verdict** rejected: one literal, five git-mentions, zero queries.
2. **Hypothesis** — a second `recoverable` variant exists for the untracked case.
   **Test** — `grep '"recoverable"' src/librarian/**/*.rs`.
   **Verdict** rejected: exactly one occurrence, at `src/librarian/tools/delete.rs:103`.
3. **Hypothesis** — the delete refuses on an untracked file, making the claim vacuously safe.
   **Verdict** rejected: the forced delete of both untracked probe artifacts succeeded
   (`deleted: true`).

## Fix

**FIXED 2026-09-24 at `5a9c51ad`, shape 3.** New `file_git_state`
(`src/librarian/tools/delete.rs`) runs one `git status --porcelain --ignored` and reports
`committed_clean`, `committed_with_uncommitted_edits`, `not_committed` (untracked, ignored, or
staged but never committed) or `unknown` (no repository or no git — stated, never guessed) as
`file_git_state`, and the file half of `recoverable` is built from it. Shape 3 rather than 2
because the middle state is exactly where "restorable" misleads — git restores the last commit and
the uncommitted edit is gone — and it costs the same single call. Synchronous, since the dry run
holds the catalog's `parking_lot` guard. The test doc comment that repeated the old claim as fact
is corrected.

## Tests added

`file_git_state_separates_untracked_committed_and_dirty` (real temp repositories: untracked,
staged, clean, dirty, and outside any repository; red first) and
`delete_preview_does_not_call_a_never_committed_file_restorable` (the reported case, asserted on
the wire field and on the sentence). Mutation via `scripts/mutation-probe.sh`, 5/5 KILLED.

## Fix provenance

- **SHA:** `5a9c51ad` (`experiments`)
- **patch-id:** `dd8b8b67e6354196cb66cefce6f4a94bfa0dcc9f`

## Workarounds

Check tracked state yourself before authorising any `force=true`:

```
git ls-files --error-unmatch <path>   # exit 0 = tracked, 1 = untracked
git status --short -- <path>          # '??' = untracked, ' M'/'M ' = tracked-and-dirty
```

Treat the `recoverable` line as describing the *catalog* half only. If the artifact is
untracked and you might want it back, `git add` it (or copy it to the session scratchpad)
before deleting.

## Resume

N/A — fixed at `5a9c51ad`.

## References

- `src/librarian/tools/delete.rs` — the literal (:103-104), the design rationale it was
  composed from (:78-84), the test doc-comment repeating it (:373), the test (:376), the
  fixture `git_root:` initialisers (:433, :542).
- `docs/issues/archive/2026-09-21-the-delete-preview-omits-the-entry-cite-rows-its-cascade-destroys.md`
  — the other defect in this same `json!` block, filed separately.
- `CLAUDE.md` § *Testing Discipline* — the remedy-text law and the *what can the addressee
  reply* answerability ceiling; § *Parsers Over a Namespace* — documented limitation versus
  silent reinterpretation.

## Cluster

`cluster/hint-composed-without-the-request` (`IC-22`). The class claim covers a
system-authored *"next-step hint or causal explanation … derived from the response's shape, or
from the check's own definition, rather than from the request that produced it or the **state
it describes**"*. That last disjunct is this defect precisely: the sentence is derived from the
delete gate's own rationale comment and published as a statement about whichever artifact was
named. The class's sixth member,
`frontmatter-id-mismatch-asserts-a-move-for-worktree-minted-ids`, contributed exactly the
diagnostic that settles it — *ask not only what input did this text consult but what would
have to be true for it to be a fact* — and here the answer is "the path is tracked", which
nothing asks.

**One mismatch recorded rather than smoothed over.** `IC-22`'s **Blind party** field reads
*"the hint's author, who is writing at the response layer where the request's arguments are no
longer in scope."* That does not hold here: `abs_path` and the catalog row are both in scope
seven lines above the literal. The blindness is narrower than the class's stated one — the
author held everything needed to check and did not, because the claim was inherited from a
correct piece of reasoning about a different population (committed artifacts). The class's
**claim** covers this instance; its **blind-party** field is scoped to the hint-layer subset
and should widen, or note that a response-layer constant is a second seam.

**Two classes checked and rejected.** `cluster/doc-contradicted-by-code` (`IC-11`) requires
the statement to have been *true when written* and the code to have later changed — this one
was never conditional, and the code has not moved.
`cluster/record-asserts-an-unchecked-completion` (`IC-8`) is about an assertion of *completion*
read later as outcome; nothing here asserts that an action completed.
