---
kind: bug
status: open
tags:
- cluster/hint-composed-without-the-request
- librarian
- body-edits
- error-messages
closed: null
opened: 2026-09-06
owner: marius
related: []
severity: medium
---

# BUG: `body_edits`' invalid-action error names four of the five valid actions, and the one it omits is the only way to edit a guarded ledger without retyping it

## Summary

`doc(action="update", patch={body_edits:[…]})` accepts **five** actions. When a caller passes
an *invalid* one, the error it gets back names **four** — omitting `edit`. The complete list
exists thirty lines above the incomplete one, in the same function, on the *missing*-action
path. So a caller probing for the valid set is shown a list that is authoritative in tone and
short by one, and the omitted member is the only action that can change text inside a
librarian-guarded ledger without replacing whole sections.

## Symptom (Effect)

Probing the action enum through the MCP `doc` tool:

```
doc(action="update", id="d24bf146cf7c789f",
    patch={"body_edits": [{"heading": "## ET-3 — …", "action": "probe_invalid", "content": "x"}]})
```

```
{
  "ok": false,
  "error": "body_edits[0]: invalid action \"probe_invalid\"; expected replace, insert_before, insert_after, or remove",
  "hint": "Check heading name and action."
}
```

`edit` is absent from that list and is nonetheless valid — the same call with
`"action": "edit"`, `old_string`, `new_string` succeeds. It was used successfully **eight
times** in the session that found this, against two guarded ledgers.

The only place `edit` is surfaced to a caller who did not already know it is the *replace*
guard's hint, reached only by attempting a replace that would consume nested headings:

```
"error": "body_edits[0]: replace on '## ET-8 — …' would wipe 4 nested heading(s): … Pass include_subsections: true to opt into consuming children.",
"hint": "Prefer action=\"edit\" with old_string/new_string to target text inside the section without touching its subsections."
```

## Reproduction

Minimal, at `8320d5b09196b723429e1028a95aab3988299e1a` (`experiments`), through the MCP tool:

1. Pick any catalogued artifact id and any heading that exists in it.
2. `doc(action="update", id=<id>, patch={"body_edits":[{"heading":<heading>, "action":"zzz", "content":"x"}]})`
3. Read the `error` string. It enumerates four actions; `edit` is not among them.
4. Re-issue with `"action":"edit"`, `"old_string":<text present in that section>`,
   `"new_string":<replacement>`. It succeeds.

Step 3 is the defect; step 4 is what makes it a defect rather than a wording preference.

## Environment

Linux, `experiments` @ `8320d5b0`, codescout v0.15.0, MCP stdio transport, project
`codescout`. Not transport-specific — the string is composed server-side and the CLI path
reaches the same function.

## Root cause

**Two error paths in one function disagree about the size of the enum, and the caller only
ever reaches the narrower one.** Read at the bytes 2026-09-06, `8320d5b0`.

`apply_body_edits` (`src/librarian/tools/update.rs:242`) validates in this order:

- **`action` missing** → `src/librarian/tools/update.rs:254` emits the hint
  `"Allowed actions: replace, insert_before, insert_after, remove, edit."` — **all five, correct.**
- **`action` present** → no validation against that set. The function branches
  `if action == "edit" { … } else { … }` (`src/librarian/tools/update.rs:265`; the `:297`
  sub-check inside the `else` arm handles `replace` only), so any unrecognised string takes the
  `else` arm.
- The `else` arm calls `perform_section_edit_ext`
  (`src/tools/markdown/edit_markdown.rs:304`), documented as a *"thin wrapper over
  `plan_section_edit` + `apply_planned_edits`"*, which forwards the action unchanged to
  `plan_section_edit` (`src/tools/markdown/edit_markdown.rs:281`). That is where it is rejected,
  with `"invalid action {:?}; expected replace, insert_before, insert_after, or remove"`. The
  `body_edits[{i}]: ` prefix and the `"Check heading name and action."` hint are re-attached on
  the way out by `apply_body_edits`' own `map_err`, which is why the message reads as though
  `apply_body_edits` composed it.

`plan_section_edit`'s message is **correct for `plan_section_edit`**. That function genuinely
does not implement `edit`; `edit` is handled one layer up, by `perform_scoped_edit`, and never
reaches it. The defect is not a stale string — it is that a callee's enumeration of *its own*
options is surfaced verbatim to a caller whose option set is strictly larger, with nothing
marking the difference.

That is also why the obvious repair is wrong: adding `edit` to `edit_markdown.rs:281` would
make the message false for every *other* caller of `plan_section_edit`.

## Evidence

### The complete list and the incomplete one, both in-tree

```
src/librarian/tools/update.rs:254
    "Allowed actions: replace, insert_before, insert_after, remove, edit.",

src/tools/markdown/edit_markdown.rs:281
    "invalid action {:?}; expected replace, insert_before, insert_after, or remove",
```

### `edit` is a first-class branch, not a fallback

`src/librarian/tools/update.rs:265-290` — `edit` is dispatched before the section-edit path,
requires `old_string`, resolves `new_string` via `require_new_string`, and calls
`perform_scoped_edit` with the heading query. It has its own error text
(`"old_string is required for action='edit'"`) naming the full call shape.

### Cost, observed rather than hypothesised

The session that found this needed to change one `**Status:**` line in each of five entries in
`docs/trackers/resume-embedding-transport-stages-1-3.md`, a declared ledger (`entry_prefix: ET`)
whose files `edit_file` refuses. Believing the four-action list, the only available route was
`replace` on whole `##` sections — 134 and 162 lines for two of them, re-emitted through JSON,
which is the anti-pattern `get_guide("librarian")` names as having caused a ~600-line body
loss. `edit` was found only because a `replace` attempt tripped the nested-heading guard and
that guard's hint mentions it.

## Hypotheses tried

1. **Hypothesis:** the `edit` action is newer than the error string, i.e. ordinary prose decay
   (`IC-11`).
   **Test:** read both sites and check whether `plan_section_edit` could serve `edit`.
   **Verdict:** rejected. `plan_section_edit` has no `edit` arm and should not — the action is
   implemented above it. The string is not stale; it is correct for its own scope and wrong for
   the caller's.
   **Evidence:** § *The complete list and the incomplete one*.

2. **Hypothesis:** the tool schema documents `edit`, so the error is a minor redundancy.
   **Test:** probe the enum through the live MCP tool and read what a caller actually receives.
   **Verdict:** rejected — the probe is the only route a caller has, and it returns four.
   **Evidence:** § *Symptom*.

## Fix

Not yet fixed. **Plan:** validate `action` against the caller-level set in `apply_body_edits`,
immediately after it is read (`src/librarian/tools/update.rs:255`), and emit the same five-member
message the missing-action path already emits — so the two paths cannot drift again. Leave
`edit_markdown.rs:281` alone: it is correct for its own callers.

The two messages should be built from **one** source, not two string literals that happen to
agree on the day they are written. A single `const` naming the five, referenced by both the
missing-action hint and the invalid-action error, is what makes the agreement checkable rather
than coincidental.

## Tests added

None yet — no fix applied. The regression test this needs must assert on **what a caller
receives for an invalid action**, not on the presence of the word `edit` somewhere in the file:
a test that greps the source for `"edit"` passes today, because the complete list is already
there on the other path. Assert that the invalid-action error string names every action the
`if/else` chain actually dispatches, so adding a sixth action without updating the message
reds.

## Workarounds

Use `action: "edit"` — it works, and for a librarian-guarded ledger it is the only way to change
text inside a section without re-emitting the whole section:

```
doc(action="update", id=<id>, patch={"body_edits": [{
    "heading": "## SECTION",
    "action": "edit",
    "old_string": "...",
    "new_string": "...",
    "replace_all": false
}]})
```

Omitting `action` entirely is a second, accidental route to the correct list: the
missing-action error names all five.

## Resume

Add the validation at `src/librarian/tools/update.rs:255`, sourcing both messages from one
`const`. Write the test first and confirm it reds against the current tree by probing an invalid
action and asserting `edit` appears in the returned error — it does not today, so the red is
available immediately without a mutation.

## References

- `src/librarian/tools/update.rs:242` (`apply_body_edits`), `:254` (five-member hint), `:265`
  (`edit` branch), `:297` (`replace` branch)
- `src/tools/markdown/edit_markdown.rs:281` (`plan_section_edit`, four-member error)
- `get_guide("librarian")` § *Choosing a mode — anti-patterns* — the whole-section-replace data
  loss this action exists to avoid
- `docs/issues/archive/2026-08-19-guarded-artifact-preamble-cannot-be-edited.md` — the adjacent
  known limit of `body_edits`' section scoping
