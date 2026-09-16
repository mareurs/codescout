---
id: '7f5fe0e00732b494'
kind: bug
status: open
title: 'BUG: doc(update) section replace drops the blank line before the next heading, silently'
tags:
- cluster/accepted-parameter-silently-dropped
- librarian
- doc-tool
- markdown
topic: librarian document editing
closed: null
opened: 2026-09-15
severity: low
---

# BUG: doc(update) section `replace` drops the blank line before the next heading

## Summary

`doc(action="update", patch={body_edits: [{action: "replace", …}]})` writes the new section
body with no blank line between its last line and the **next heading**, joining them. The
`content` may be passed *with* a trailing newline and it is still discarded, the call returns
`updated: true`, and nothing in the response mentions it. The same tool's `action: "edit"`
preserves the byte.

## Symptom (Effect)

After four `replace` edits to one bug file, four headings had lost their preceding blank line:

```
is not simply broken.
## Root cause
```

No error, no warning. The response was `{"id": …, "updated": true, "wrote_to": …}` each time.

## Reproduction

1. Take any catalog-managed markdown artifact whose sections are separated by blank lines.
2. `doc(action="update", id=…, patch={body_edits: [{heading: "## X", action: "replace",
   content: "…text…\n"}]})` — note the trailing newline in `content`.
3. Read the file: the line following the section's last line is `## <next heading>`, with no
   blank line between them.

Detector, which is also how this was found:

```bash
awk 'NR>1 && /^## / && prev != "" {print NR": "$0} {prev=$0}' <file>
```

## Environment

codescout MCP, `experiments` at `b59a035d`, this checkout. Observed 4/4 on `replace`; `edit`
did not reproduce it in 4 attempts.

## Root cause

Unknown — not read in the source. What is established is the **boundary**: the defect is
specific to the `replace` action and not to section writing in general, because `action: "edit"`
on the same artifact in the same call shape preserved and restored the separator.

Inferred, not measured: `replace` appears to trim trailing whitespace from `content` and then
join sections without re-inserting a separator, while `edit` performs a substring replacement
inside an already-assembled body and never touches the boundary.

## Evidence

Measured on `docs/issues/2026-09-15-file-provenance-reads-mv-inside-a-filename-and-attributes-a-write.md`,
2026-09-15, across two `doc(update)` calls carrying four `replace` edits between them.

**The control is what makes this a measurement rather than a complaint about a file that may
always have been malformed** — the committed version was checked and was clean:

```bash
git show "HEAD:$F" | awk 'NR>1 && /^## / && prev != "" {print NR": "$0} {prev=$0}'
# (no output — every heading in the committed file had its blank line)
```

So the joins were introduced by the writes, not inherited. Repairing them with `action: "edit"`
and a trailing `\n` in `new_string` worked on all four, which is the second half of the same
discrimination: the byte is representable, and `replace` is what drops it.

### Independently reproduced 2026-09-16 — published because a confirmation is a DENOMINATOR

Not a second catch, and recorded as such. Session `a3bf229c` hit this while fixing the
mutation-probe non-cargo-runner bug: one `doc(action="update")` call carrying two
`body_edits` entries with `action: "replace"` (`## Fix`, `## Tests`), after which **both**
following headings — `## Tests` and `## References` — had lost their preceding blank line.
The replaced sections' own content was intact and the heading map read back correctly, so
neither the response nor a re-read named it; `cat -A` on the boundary did.

Two details this adds rather than restates. It fires **per replaced section**, not once per
call — two replaces produced two damaged boundaries in a single write. And `action: "edit"`
with `old_string`/`new_string` does **not** exhibit it: the repair was two `edit` calls
appending a newline, which makes the workaround this file already names a confirmed one
rather than an assumed one.

**Extended the same evening, and one half of the above was too narrow.** `action:
"insert_after"` with `at: "end-of-section"` exhibits it as well — two such inserts, into two
different bug files, each cost the blank line before the NEXT `##` heading. So the subject is
not `replace`: it is any body write that ends a section, and a reader who took "replace" as the
trigger would not check after an insert.

**And it accumulates unnoticed, which is the part worth acting on.** Scanning one of those files
before editing it found **two** boundaries already damaged — `## Fix` and `## Resume` — present
in the committed bytes at `HEAD`, from earlier writes by other sessions. Nothing had reported
them, and a later reader would have attributed them to whoever touched the file last. Both were
repaired in the same commit as the instance that found them.

The discriminator is one line, needs no tooling, and is worth running after any `doc` body write.
**It must be fence-aware, and the first version published here was not** — see below for why that
is this corpus's own parser law rather than a typo:

```
awk '/^```/{f=!f} !f && prev !~ /^$/ && /^#/ {print "MISSING BLANK BEFORE "NR": "$0} {prev=$0}' <file>
```

**The fence-blind version reported two findings on THIS FILE, and both were false.** Line 33 is
the `## Root cause` heading inside the fenced block above — this file quoting its own damage —
and line 77 is a `#` shell comment inside a fenced `bash` example. A document about a heading
defect cannot exhibit a damaged heading without a heading scanner reading the exhibit as real.
That is `CLAUDE.md` § *Parsers Over a Namespace* holding about the detector for the very bug
this file records, and the fence is the escape the corpus already has: honour it, or the
instrument is loudest exactly on the files that document the problem.

**Prevalence, measured with the corrected form at `HEAD` on 2026-09-16.** `CLAUDE.md` alone
carries **11** damaged `##` boundaries, none of them in a fence and none written by the session
that counted them. Reported as an observation, not a diagnosis: this detector reads a boundary
and cannot attribute a cause, so nothing here claims all 11 came from `doc`. They were left
unrepaired deliberately — an 11-hunk sweep of the hottest shared file on a checkout with live
peers costs more than the defect does.

The limit of the check, stated because it is an ABSENCE assertion: it is monotone under any
change that removes headings entirely, and it is a boundary check rather than a content one.


## Hypotheses tried

1. **The file was already malformed.** **Refuted** by the `git show HEAD` control above.
2. **Trailing newline in `content` was simply omitted by the caller.** **Refuted** — the
   `## Root cause` replace passed `content` ending in `\n` explicitly and still lost it.

## Fix

Not attempted. Either preserve a caller-supplied trailing newline, or unconditionally emit the
separator when a section is followed by a heading. The second is probably right: the separator
is a property of the *document*, not of the section's content, so making it depend on caller
bytes is what produced the bug.

## Tests added

None. Shape: a `replace` against a section followed by a heading, asserting the rendered file
still matches `\n\n## ` at that boundary — plus the `edit` case as a control, so a fix that
makes both paths equally wrong cannot pass.

## Workarounds

Use `action: "edit"` with `old_string`/`new_string` when the section's tail matters, and run
the `awk` detector above after any batch of `replace` edits. Repairing after the fact works:
an `edit` whose `new_string` appends `\n` to the section's final line restores the separator.

## Severity note

**Low, and the reason is worth stating so nobody re-raises it as urgent.** CommonMark lets an
ATX heading interrupt a paragraph, so the rendered output is unchanged — this is a silent
mutation of stored bytes, not a rendering defect. It matters because it is silent and because
it accumulates: every `replace` on a document removes one more separator, and no reader of the
response has any signal that it happened.

## Classification

`cluster/accepted-parameter-silently-dropped` (`IC-15`). The caller's `content` — specifically
its trailing newline — is accepted at the boundary, the call succeeds, and the byte is
discarded downstream. `updated: true` is the only feedback, and it is identical whether the
newline was applied or dropped, which is that class's claim exactly.

## Resume

Read `replace`'s section-assembly path in the librarian body-edit code and confirm or refute
the inferred mechanism above before changing anything — the Root cause here is explicitly
marked inferred, and this repo has a standing rule that an unmeasured mechanism is a
hypothesis wearing a conclusion's clothes.

## References

- `docs/issues/archive/2026-09-15-file-provenance-reads-mv-inside-a-filename-and-attributes-a-write.md` (`76c83a43c2d1752f`) — the artifact this was found on, while editing it.
