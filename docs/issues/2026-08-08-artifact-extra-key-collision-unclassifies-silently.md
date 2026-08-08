---
id: '63279f39570cd44a'
kind: bug
status: open
title: 'BUG: artifact `extra` writes a key that collides with a canonical frontmatter key, and the duplicate silently drops the artifact out of its own kind'
tags:
- librarian
- artifact
- frontmatter
- silent-failure
- bookkeeping
closed: null
opened: 2026-08-08
owner: marius
related: []
severity: high
---

# BUG: artifact `extra` writes a key that collides with a canonical frontmatter key, and the duplicate silently drops the artifact out of its own kind

## Summary

`artifact(create|update, extra={…})` writes each `extra` key verbatim into the YAML
frontmatter with no check against the canonical keys the librarian itself emits (`id`,
`kind`, `status`, `title`, `owners`, `tags`, `topic`, `time_scope`). Passing
`extra={"kind": "bug"}` therefore produces **two `kind:` lines in one mapping**. YAML
rejects the duplicate, the whole frontmatter block fails to parse, and the librarian falls
back to path-glob classification — silently. The artifact keeps its file, its id, and its
content, and stops being a bug.

Same consequence as `docs/issues/archive/2026-08-06-artifact-create-bug-defaults-to-invalid-draft-status.md`
(`d61e3b535e2cae4a`, fixed): a correctly-authored, committed, pushed bug file never appears
in the answer to *"what's open?"*, and no gate notices. Different mechanism, same family —
which is why this one is filed rather than folded into that one.

## Symptom (Effect)

`docs/issues/2026-08-08-edit-file-out-of-project-ack-handle-unresolvable.md` was created
2026-08-08 with correct frontmatter. Hours later the canonical triage query returned 11
open bugs and **this file was not among them**. `artifact(action="get")` on its id showed
why:

```
"kind":   "tracker",     <- file says bug
"status": "active",      <- file says open
"title":  null,          <- file has a title
"extra":  <absent>
```

and the body began with the frontmatter itself:

```
"body": "---\nid: a99388a299352d21\nkind: bug\nstatus: open\ntitle: edit_file's out-of-project ack handle…"
```

The frontmatter was being served as body — the parse had failed and nothing said so.
`librarian(action="reindex")` reported `unknown_count: 0` for it: not flagged as
unclassifiable, **confidently misclassified**.

## Reproduction

```
artifact(action="create", kind="bug", rel_path="docs/issues/x.md",
         title="…", extra={"kind": "bug", "opened": "2026-08-08"})
librarian(action="reindex")
artifact(action="find", kind="bug", filter={"status": {"in": ["open","investigating"]}})
# the new file is absent
```

The offending frontmatter, verbatim (note lines 3 and 13):

```yaml
---
id: a99388a299352d21
kind: bug
status: open
title: edit_file's out-of-project ack handle does not resolve …
owners:
- marius
tags:
- tooling
- write-guard
- ack
- misleading-error
kind: bug
opened: 2026-08-08
severity: low
---
```

Canonical keys are emitted first, then `extra` keys are appended — so a colliding `extra`
key always lands *after* the canonical one, in the same mapping.

## Environment

Linux, `experiments` at `0b9e7238`, codescout 0.15.0, live MCP server (release binary
rebuilt 2026-08-08 ~17:45).

## Root cause

`extra` is documented as *"custom frontmatter keys … Written verbatim to YAML and
round-trip-safe across updates"*. There is no collision check against the canonical key
set, and no validation that the emitted frontmatter re-parses.

**measured 2026-08-08:** removing the single duplicate `kind: bug` line and re-running
`librarian(action="reindex")` restored every field at once — `kind: bug`, `status: open`,
the full `title`, `owners: [marius]`, all four `tags`, and `extra: {opened, severity}` —
and the body moved back to starting at `## Summary`. One line, whole classification. That
is the experiment, not an inference from reading the parser.

**Do not infer the mechanism from a grep count.** `grep -c '^kind:'` over `docs/issues/`
returns 2 for this file *and* 2 for
`docs/issues/archive/2026-08-06-artifact-create-bug-defaults-to-invalid-draft-status.md`,
which is classified correctly — its second match is inside a fenced example in the body.
The count does not distinguish "duplicate key in frontmatter" from "the word appears in
prose". Read the block.

## Evidence

### The scan that found the blast radius

`grep(pattern="^kind:", glob=["docs/issues/**/*.md","docs/trackers/**/*.md"], mode="files")`
→ 333 matches in 331 files. Exactly two files have 2; one is this defect, the other is the
false positive described above. So the corruption is **one file**, not a class — this time.

### Why it survived a full working day

The file reads correctly to a human, `git` shows nothing unusual, `audit_doc_refs` does not
look at frontmatter validity, and the canonical open-bug query returns a plausible-looking
list that is simply one shorter. There is no surface on which this looks wrong.

## Hypotheses tried

1. **Hypothesis:** duplicate `kind:` keys break the parse.
   **Test:** removed the second line, reindexed, re-fetched.
   **Verdict:** confirmed — every field returned in one step.
2. **Hypothesis:** any file with two `^kind:` matches is corrupted.
   **Test:** the archived sibling has two and classifies correctly.
   **Verdict:** rejected — the second match is body prose. The grep is a *finder*, not a
   test.

## Fix

Not implemented. Two independent guards, and the second matters more than the first:

1. **Reject the collision at the input boundary.** `artifact(create|update)` should return
   a `RecoverableError` when an `extra` key names a canonical frontmatter field, listing
   the reserved set and pointing at the dedicated parameter (`kind=` / `status=` / …). This
   is the *repair-and-continue* class only if the value matches what the canonical field
   already holds; otherwise it is genuinely ambiguous and must error.
2. **Never write frontmatter that does not re-parse.** After serializing, parse the block
   back before the write lands. This catches the whole family — including collisions no
   allowlist anticipates — and turns a silent misclassification into a failed call.

A `doctor` check is the third layer: report artifacts whose on-disk frontmatter fails to
parse, rather than letting them fall through to glob classification. Today a parse failure
and a genuinely unclassified file are indistinguishable in the reindex report — this one
counted as `unknown_count: 0`.

## Tests added

None yet — not fixed. When fixed: a create with `extra={"kind": …}` must error rather than
write; and a round-trip test asserting emitted frontmatter re-parses to the same field set
for every canonical key.

## Workarounds

Do not put canonical keys in `extra`. For bug files the canonical set is `kind`, `status`,
`title`, `owners`, `tags` — pass those as their own parameters. `extra` is for `opened`,
`closed`, `severity`, `owner`, `related`.

To detect an existing casualty: `artifact(action="get", id=…)` and check whether `body`
begins with `---`. If it does, the frontmatter is being served as body and the row is
misclassified.

## Resume

Implement guard 2 first (re-parse before write) — it subsumes guard 1 and needs no
maintained list of reserved names. Site it next to the frontmatter serializer in the
librarian write path; find it with `references()` on the emit function rather than grepping
for `kind:`, per *Root cause*.

Before closing, re-run the scan in *Evidence* — it is cheap and it is the only thing that
would catch a second casualty created in the meantime.

## References

- `docs/issues/archive/2026-08-06-artifact-create-bug-defaults-to-invalid-draft-status.md`
  — same family (bug file invisible to the canonical open-bug query), different mechanism,
  already fixed
- `docs/issues/2026-08-08-edit-file-out-of-project-ack-handle-unresolvable.md`
  (`a99388a299352d21`) — the casualty, repaired in this same commit
- `get_guide("librarian")` § *artifact(action="create")* — where `extra` is specified

