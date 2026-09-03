---
id: '84842942105b223b'
kind: bug
status: open
title: librarian guard blames frontmatter for a body-only edit, and its hint prescribes the refused call
owners:
- marius
tags:
- cluster/hint-composed-without-the-request
- librarian-guard
- edit_file
- error-messages
topic: error messages
opened: 2026-09-03
severity: low
unverified: the refusal itself is probably correct (Iron Law 5); only the message is established as wrong. Whether the text grammar SHOULD be refused on markdown is not settled here
---

## Summary

The librarian guard refuses a **text-grammar** `edit_file` call on a stamped artifact with
an error that says the caller tried to edit **frontmatter**, and a hint that prescribes the
exact call form the caller already used. Both statements are false of the request. The
guard is reading the file's status, never the request.

## Symptom (Effect)

A call passing only `path`, `old_string`, `new_string` — no `frontmatter`, no `heading`:

```
edit_file(path="docs/trackers/prompt-surface-compaction-session-log.md",
          old_string="## Wins Index", new_string="## Wins Index")
```

returns:

```
'docs/trackers/prompt-surface-compaction-session-log.md' is a librarian-managed artifact
(stamped — it carries a librarian `id:`, so its frontmatter is catalog-indexed and a direct
frontmatter edit would not reach the catalog) — do not edit its frontmatter directly

hint: Frontmatter on this file is catalog-indexed, so edit it through the catalog:
• doc(action="update", id="<id>", patch={status: "...", tags: [...]})
Reads and BODY edits are allowed directly — read_file, and edit_file without its
`frontmatter` param, both work on this file.
```

The hint's final sentence describes the refused call exactly. Following it reproduces the
refusal.

## Reproduction

Deterministic, and the `old_string` above is **identical to** `new_string` so nothing can
be written either way:

1. Pick any `docs/trackers/*.md` carrying a librarian `id:` in frontmatter.
2. `edit_file(path=<that file>, old_string=<any literal in it>, new_string=<same>)`
3. Observe the frontmatter error.
4. The same edit with `heading=<section>, action="edit"` **succeeds** — confirming the
   discriminator is the grammar, not frontmatter.

Step 4 was run in the same session and wrote successfully.

## Environment

`experiments` @ `636eab37`, codescout MCP over stdio, project `codescout`.

## Root cause

The guard fires on **file status** (stamped / augmented / ledger) combined with the call
grammar, and composes its message from that alone. Nothing in the message-building path
consults whether the request actually carried a `frontmatter` key — so the refusal names a
cause the request does not have, and the hint recommends a form the guard is at that moment
rejecting.

This is `cluster/hint-composed-without-the-request` in its strongest form yet: the class's
fifth member was *"composed from nothing at all: a static string that consults neither
response nor request"*, and this one goes further by **actively contradicting** the request
it is refusing. The plausible-route property that defines the class holds — the prescribed
route is real, correct for the case it names, and returns a clean refusal rather than an
error a caller would question.

*Measured 2026-09-03: both calls above, run back to back, one refused and one written.*

## Evidence

The two calls differ **only** in grammar:

| call | params | result |
|---|---|---|
| A | `path`, `old_string`, `new_string` | refused, "do not edit its frontmatter directly" |
| B | `path`, `heading`, `action="edit"`, `old_string`, `new_string` | `status: "ok"`, written |

Neither passed `frontmatter`. If the message were true of A it would be true of B.

## Hypotheses tried

1. **Hypothesis** — the call implicitly touched frontmatter because the file's first
   section abuts it. **Test** — `old_string` was `## Wins Index`, at line 85, far below the
   frontmatter block, and identical to `new_string`. **Verdict** — rejected.
2. **Hypothesis** — text-grammar edits are refused on markdown by Iron Law 5, and the
   frontmatter text is simply the wrong message on a correct refusal. **Verdict** —
   **confirmed as the likely mechanism**, and it does not excuse the message: IL-5 is a
   different rule with a different remedy, and the hint actively points away from it.

## Fix

Not fixed. The refusal itself may well be correct (IL-5 routes markdown edits through the
heading grammar); what is wrong is the message. It should name the grammar as the cause and
prescribe `heading` + `action`, and it must not claim a `frontmatter`-free call works when
that is the call being refused.

## Tests added

None — not fixed. Reproduction is deterministic and side-effect-free as written.

## Workarounds

On a stamped or augmented markdown artifact, use the heading grammar
(`heading=…, action="edit"`) — it works and is what Iron Law 5 asks for regardless. Read
the *hint* here as naming the file's class, not as describing your call.

## Resume

Find the guard's message-composition site (`src/util/librarian_guard.rs` and its callers in
`src/tools/markdown/`) and branch the text on which grammar was received. Cheapest correct
fix is a distinct message for the text-grammar case.

## References

- `docs/trackers/issue-clusters/IC-22-hint-composed-without-the-request.md`
- `docs/issues/2026-09-03-scoped-edit-silently-takes-the-first-of-several-old-string-matches.md`
  — found in the same minute, on the same file, by the call this one forced

