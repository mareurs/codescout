---
id: c202d8febd80ca8a
kind: bug
status: fixed
title: a body that already begins with a frontmatter block silently becomes a second, inert block
tags:
- cluster/addressing-without-an-escape-hatch
- librarian
- frontmatter
- silent-corruption
- artifact-create
- artifact-update
closed: null
opened: 2026-08-31
owner: marius
related:
- docs/issues/2026-08-31-peer-commit-captures-another-sessions-working-tree.md
- docs/issues/archive/2026-08-31-doctor-test-substring-matches-a-random-tempdir-name.md
severity: medium
---

# BUG: a body that already begins with a frontmatter block silently becomes a second, inert block

## Summary

`artifact(action="create")` and `artifact(action="update", patch={body})` interpolate the
caller's body verbatim beneath the frontmatter they generate. A body that itself begins with
`---\n…\n---` therefore lands as a **second frontmatter block inside the body**, where it is
inert: every key in it is invisible to the catalog and to every `artifact(find)` query, while
reading exactly like authoritative frontmatter to a human or an agent opening the file.

Both calls return success. Nothing warns.

## Symptom (Effect)

The file acquires four `---` lines instead of two. Reproduced 2026-08-31 on `experiments`:

```
     1	---
     2	id: '0fa067bda1e1df6a'
     3	kind: note
     4	status: draft          <- the catalog's value
     5	title: Repro — body that already carries a frontmatter block
     6	---
     7	
     8	---
     9	status: open           <- the caller's value, inert
    10	opened: 2026-08-31
    11	severity: medium
    12	owner: marius
    13	---
    14	
    15	# Repro body
```

The two `status` values disagree and the catalog silently wins. In the wild this produced a
bug file whose body read `status: fixed`, `closed: 2026-08-31` while
`artifact(find, kind="bug")` reported it `open`.

## Reproduction

Minimal, on `experiments` at `c6d7d83b`:

```
artifact(action="create", kind="note",
         rel_path="docs/zz-repro-double-frontmatter.md",
         title="…",
         body="---\nstatus: open\nowner: marius\n---\n\n# Repro body\n")
```

Then `grep -c '^---$' docs/zz-repro-double-frontmatter.md` → **4**.

The second seam, on the same artifact:

```
artifact(action="update", id="…", patch={body: "---\nstatus: mitigated\n---\n\n# Second seam\n"})
```

→ **4** again. Both seams reproduce; neither warns.

## Environment

Linux; codescout `experiments` @ `c6d7d83b`; MCP stdio transport; project `codescout`.
Observed and reproduced 2026-08-31.

## Root cause

`src/librarian/frontmatter.rs:149` ends `write` with:

```rust
format!("---\n{yaml}---\n{body}")
```

`body` is interpolated with no check that it begins with a frontmatter delimiter. **Measured
2026-08-31**: the reproduction above, run twice against a live server, once per seam — not
inferred from the source alone.

The near-miss is what makes this worth recording. The same function already reasons carefully
about duplicate keys, ten lines above, and its comment states the stakes exactly:

> *"A reserved key here would emit a SECOND line for a key `serde_yml` already wrote above, and
> a duplicate key makes the entire block unparseable — which costs every field, not just this
> one."*

That guard covers the `extra` map and stops at the map boundary. A body carrying a whole
duplicate *block* is the same failure one level up, and no guard looks there. The author was
reasoning about precisely this hazard at the adjacent seam.

## Evidence

### Two instances in the wild, both on 2026-08-31

- `docs/issues/2026-08-31-peer-commit-captures-another-sessions-working-tree.md` — orphan block
  carried `opened`, `severity`, `owner`, `related`; all four invisible to every query. Repaired
  in `351836a8` by rewriting the body and merging the keys into real frontmatter via `extra`.
- `docs/issues/archive/2026-08-31-doctor-test-substring-matches-a-random-tempdir-name.md` — orphan block
  carried `status: fixed`, `closed`, `severity`, `owner`, `unverified`. The catalog reported
  `open`. Repaired by a concurrent session.

### Why it survives review

The malformed file is *more* plausible than a correct one, not less: it opens with a
well-formed stamped block, and the orphan below it looks like an ordinary hand-written header.
Nothing in `git diff` marks the second block as inert, and `artifact(get)` returns it as body
content without comment.


### Recurrence 2026-09-01 — still live at `30b6fc41`, and the author knew the bug

Session `codescout-b7` filed `docs/issues/2026-09-01-two-correct-pre-commit-guards-have-an-empty-intersection.md`
via `artifact(action="create")` with a body copied from `docs/issues/_TEMPLATE.md` — which
begins with a frontmatter block, because the template is a bug file. Result, verified in the
bytes immediately after the call:

```
$ grep -c '^---$' docs/issues/2026-09-01-two-correct-pre-commit-guards-...md
4          # two complete blocks
```

The catalog's block came first with `id`/`kind`/`status`/`title`/`owners`/`tags`/`topic`; the
template's followed with `opened`/`closed`/`severity`/`owner`/`related` — the five fields the
catalog block does **not** carry, so the inert block held the only copy of every date and the
severity. Repaired by hand-merging into one block and re-running `librarian(action="reindex")`.

**Two things this datapoint adds beyond "it still reproduces."**

**The trigger is the project's own documented workflow, not a user error.**
`get_guide("tracker-conventions")` and `docs/issues/_TEMPLATE.md` both instruct *copy this
file*; the template's first line is `---`. So the prescribed way to open a bug file is also the
reproduction, and every bug filed through `artifact(create)` rather than a literal `cp` hits it.

**The author had read this bug file in the same session and hit it anyway** — which is the
`OB-N` shape rather than carelessness. Knowing the class does not help, because the defect is
silent at the moment of the write: `create` returned `{"id", "abs_path", "wrote_to"}` with no
warning, and the file *looks* correct until you count `---` lines. The observer who could see it
is the one who reads the file back, which nothing in the create path makes anyone do.

**Cheap remedy this suggests, stated as a candidate not a fix:** `create` already parses the
body enough to know whether it starts with `---`. Refusing with a `RecoverableError` naming the
duplicated keys would be loud at the only moment anyone can act on it, and the refusal text
could carry the merge. Not designed here; see § *Fix*.
## Hypotheses tried

1. **Hypothesis:** the two observed instances came from hand-editing rather than a tool path.
   **Test:** ran `artifact(create)` with a frontmatter-leading body against a live server.
   **Verdict:** rejected — the tool reproduces it exactly, byte-for-byte in structure.
2. **Hypothesis:** only `create` has the seam, so a fix there is sufficient.
   **Test:** ran `artifact(update, patch={body})` with the same shape.
   **Verdict:** rejected — both seams reproduce. Two guarded sites, not one.

## Fix

Not yet implemented. Refuse at the input boundary rather than repair at the writer:
`RecoverableError` from `create` and from `update`'s body path when `body` starts with `---`
followed by a delimiter line, naming the two legitimate intents — *pass the fields as
`status`/`tags`/`extra` parameters*, or *fence the block if it is documentation*.

Refusal is preferred over silently stripping the block: stripping would discard keys the
caller believed they were setting, which is the same silent-loss failure in the other
direction.

**The writer must stay infallible.** `write` returns `String`, and its own comment explains
that the `extra` backstop exists so it can. The check belongs where the `RESERVED_KEYS`
refusal already lives — at `artifact(create|update)`'s input boundary — not at
`frontmatter.rs:149`.

## Tests added

`create_refuses_a_body_that_opens_its_own_frontmatter_block` and
`create_accepts_a_body_whose_dashes_are_not_a_leading_block` in `src/librarian/tools/create.rs`;
`update_refuses_a_full_body_replacement_that_opens_a_frontmatter_block` and
`body_edits_may_splice_content_that_begins_with_dashes` in `src/librarian/tools/update.rs`.

**Three mutations, because there are two guarded SITES and two directions** — CLAUDE.md
§ *Testing Discipline*: a mutation answers a question about one *line*, so a kill at `create`
says nothing about `update`.

| # | mutation | result |
|---|---|---|
| A | delete the guard call in `create::call()` | 25 passed / 1 failed — kills the `create` refusal test **only** |
| B | widen the predicate to `body.contains("---")` | 25 passed / 1 failed — kills the `create` acceptance twin **only**; the refusal test stays **green** |
| C | delete the guard call in `update::call()` | 983 passed / 1 failed — kills the `update` refusal test **only**; every `create` test stays **green** |

**B is why the acceptance twin exists.** A refusal test is an *existence* assertion and is monotone
under widening — a guard refusing every body whatsoever satisfies it completely — so it cannot see
over-refusal at all. Its three fixtures fail for three different reasons: the leading blank line
(the escape the hint promises, under test), a horizontal rule further down, and a fenced YAML
example, which is how any document *about* frontmatter is written, including this repo's guides.

**C is why the second site got its own test.** It is also the measurement that would have caught a
premature close: the first fix covered `create` alone, and this file's own *Tests added* section had
already named both seams. Sources restored byte-exactly after every run (`diff -q` → identical),
verified rather than assumed.

**Deliberately unguarded, and pinned as such:** `body_edits`. Those splice content at a heading
inside an existing document, so a fragment opening with `---` is a horizontal rule mid-body, never a
second block at position 0. `body_edits_may_splice_content_that_begins_with_dashes` exists so that
exemption is distinguishable from having forgotten the site.
## Workarounds

Pass metadata as parameters, never as body text:

```
artifact(action="create", kind="bug", rel_path=…, title=…, status="open",
         tags=["cluster/…", …], extra={"opened": "…", "severity": "…"},
         body="# BUG: …\n\n## Summary\n…")
```

This file was written that way deliberately.

To detect existing instances:

```
for f in docs/issues/*.md; do n=$(awk 'NR>1 && /^---$/{c++} END{print c+0}' "$f");
  [ "$n" -ge 2 ] && echo "$n $f"; done
```

To repair one, merge the orphan's unique keys into real frontmatter and rewrite the body
without it, in a single `artifact(action="update", patch={body: …, extra: {…}})` — the orphan
sits above the first heading, so `body_edits` cannot target it.

## Resume

Add the input-boundary refusal to `artifact(create)` and `artifact(update)`. The `RESERVED_KEYS`
clash refusal is the pattern and the place to put it next to — read
`src/librarian/frontmatter.rs:70-78` (`reserved_keys_in_extra`) and follow its caller into the
tool boundary. Write the two-seam regression test first; a fix applied to `create` alone passes
a single-seam test and leaves `update` open, which is how this class survives.

Then sweep `docs/issues/` and `docs/issues/archive/` with the detection loop above — only the
open corpus has been checked, and only on 2026-08-31.

## References

- `src/librarian/frontmatter.rs:105-150` — `write`, and the `RESERVED_KEYS` guard at `:70-78`
- `docs/issues/2026-08-31-peer-commit-captures-another-sessions-working-tree.md` — instance 1
- `docs/issues/archive/2026-08-31-doctor-test-substring-matches-a-random-tempdir-name.md` — instance 2
- `docs/trackers/issue-clusters.md` — `IC-6`, the class this instantiates
