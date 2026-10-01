---
kind: bug
status: wontfix
tags:
- cluster/unclassified
closed: 2026-10-01
opened: 2026-09-24
owner: marius
related:
- docs/issues/2026-09-04-patch-must-be-a-json-object-refusal-is-unreproduced.md
severity: low
---

# BUG: a literal `\uXXXX` escape cannot be written through the edit tools — it arrives decoded, and the layer that decodes it is not identified

## Summary
Reported from another project's SDD run (`codescout-lessons.md` § 6.5, secondhand from a
subagent): writing curly quotes / en-dashes / NBSP through `edit_file` / `edit_code` was
unreliable — "a single `\u` escape auto-decoded to the literal character, while a doubled
backslash was preserved literally instead of collapsing". Worked around with a `python3` heredoc.
Reproduced the first half here; the second half reproduced **inverted**, and codescout's own
decoder does not touch `\u` at all, so this may be the tool-call encoding layer rather than
codescout.

## Symptom (Effect)
Scratch file, 2026-09-24, bytes checked with `od -c`:
```
edit_file(edits=[{new_string: "don’t – x y"}])  → don’t – x y      (342 200 231, 342 200 223)
edit_file(edits=[{new_string: "don\\u2019t"}])            → don’t            ← doubled backslash ALSO decoded
edit_file(insert="append", new_string: C = "don’t")   → don’t            ← single-string mode, same
```
Typed characters arrive correctly; a *literal six-character* backslash-u-2019 sequence cannot be written by any
of these three forms. The report said a doubled backslash stayed literal; here it decoded too.

## Reproduction
Live binary at `506924f2`, Claude Code harness, `edit_file` on any scratch file; inspect with
`od -c`.

**2026-10-01, on the rebuilt binary.** Typing the six characters into `create_file`, `edit_file`
(single-string and `edits[]`) from a Claude Code session: all four wrote the character (bytes 342 200
231 under `od -c`). Handing the same six characters to the server over stdio, correctly escaped:
all five forms wrote them literally. The two probes are the discriminator the `## Resume` section
asked for, taken at the wire instead of by logging inside the server.

## Root cause

**Not in codescout. The decode happens between what a model emits and the JSON the server is
handed.** Identified 2026-10-01 by taking the harness out of the path: a fresh `codescout start`
driven over stdio by a script, sending the six characters (backslash, u, 2, 0, 1, 9) correctly
JSON-escaped, wrote them literally through every path:

| form | result on disk |
|---|---|
| `create_file`, `content` | literal |
| `edit_file`, single `old_string` / `new_string` | literal |
| `edit_file`, `edits[]` as an array | literal |
| `edit_file`, `edits[]` as a JSON string, inner JSON properly escaped | literal |
| the same string form with a single backslash in the inner JSON | decoded, which is JSON's own meaning of that text and not a second decode |

So the three suspects in this file are cleared: the escape repair never touches a unicode escape
(only newline, tab, carriage return and, in one tier, quotes), `optional_array_param` performs
exactly one `serde_json::from_str`, and the plain paths decode nothing.

The same session then reproduced the symptom on its own tool calls, which is the layer's
signature: a single-backslash escape typed into an `edit_code` body was written as the character,
while the doubled-backslash forms typed alongside it survived. So, in a Claude Code session, a
single-backslash unicode escape in a parameter is collapsed before the server sees it and a doubled
one is not. Which party does it, the model's output encoding or the client's argument
serialization, is not visible from either end and is not a codescout question.

## Evidence
See Symptom. The adjacent open bug in `related:` needs the same missing observation — the bytes
the server received — and names the same instrument.

## Hypotheses tried

1. **codescout's escape-repair decodes a unicode escape.** Test: read `edit_repair.rs`, then drive
   the server directly. **Verdict:** rejected, twice.
2. **The stringified-array fallback double-decodes.** Test: stdio driver, inner JSON escaped
   properly and with a single backslash. **Verdict:** rejected: one decode, JSON's own.
3. **The decode happens before the server is called.** Test: the same six characters through the
   harness versus over stdio. **Verdict:** confirmed by exclusion: the server is literal on every
   path it owns, and the harness path decodes the same input. The harness's own half is read, not
   observed, since its wire form is not visible from the caller.

## Fix

**No codescout change, and none is possible here: the layer that decodes is not in this repo.**
What this repo can do is hold its own half of the property, and that is done: the server is pinned
to write the escape literally in the shapes it accepts (see *Tests added*), so a decode added on
any of those paths reds instead of silently becoming this bug's second cause.

Not done, deliberately: a note on the server's prompt surfaces telling callers to double the
backslash. It would state a client's behaviour as codescout's, the claim is measured on one client
(Claude Code) only, and those surfaces carry a character cap. If an operator wants it, the measured
form is: in a Claude Code tool call, a single-backslash unicode escape in a parameter arrives
decoded and a doubled one arrives as written.

## Tests added

In `src/tools/edit_repair.rs` and `src/tools/edit_file/tests.rs`, all passing on unchanged code (a
pin, so there is no red before green):
`decode_literal_escapes_leaves_a_unicode_escape_as_written`,
`edit_file_writes_a_literal_unicode_escape_as_written_in_every_argument_form` and
`edit_file_escape_repair_decodes_a_newline_but_leaves_a_unicode_escape`. They build the six
characters from a doubled backslash because a single-backslash escape typed through these same
tools is what this bug is about.

Mutated one per site on the final bytes: a decoder arm that decodes `u` (as a carriage return) is
killed by the unit pin and the repair case; an un-doubling step in `optional_array_param`'s string
fallback is killed by the third argument form alone, with the first two left literal. The plain
single-string and array forms have no codescout decode to mutate, so they guard against a future
addition and are inert as killers today.

## Disposition

`wontfix`: not a codescout defect. The server is verified literal on every path it owns, and the
layer that decodes is outside this repo. What was done instead is the regression pin below, so
the server's half cannot silently become a second cause.

## Fix provenance

- **SHA:** `5c8e28f8667892c507aa741c77d89c1b6add3a40` (`experiments`)
- **patch-id:** `ffe7522211c9f3750fe7527339e7166b82bc5585` (`git show <sha> | git patch-id --stable`)

This commit adds tests only, no production change. Verified 2026-10-01: gate `FMT=0 CLIPPY=0
LEAN=0 DEFAULT=0`, the three tests read by name out of both lanes, two mutations observed killed on
the final bytes.

## Workarounds
Type the characters themselves (they arrive intact), or write the file with a `python3` heredoc
through `run_command` when a literal escape sequence is the content.

## Resume

Closed. The observation this section asked for, the bytes the server received, is available by
driving `codescout start` over stdio with a script and reading the file it writes; the two scripts
lived in a session scratchpad and are described in *Root cause*, which is enough to rebuild them.

## References
- `codescout-lessons.md` (repo root, untracked) § 6.5.
