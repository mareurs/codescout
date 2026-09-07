---
kind: bug
status: fixed
tags:
- cluster/hint-composed-without-the-request
- librarian
- body-edits
- error-messages
closed: 2026-09-07
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

**CORRECTED 2026-09-07 — the same dispatch exists at THREE sites, and this section named one.**
Everything above is right at the bytes about `apply_body_edits`. It is not the whole population.
Probed live before the fix:

| dispatcher | caller surface | missing-action names | invalid-action names |
|---|---|---|---|
| `apply_body_edits` (`update.rs:242`) | `doc(update, patch={body_edits})` | 5 ✓ | 4 ✗ |
| single-edit mode (`edit_markdown.rs:1386`) | `edit_file(heading=, action=)` | 5 ✓ | 4 ✗ |
| `plan_batch` (`edit_markdown.rs:647`) | `edit_file(edits=[…])` | **0** ✗ | 4 ✗ |

`plan_batch` was worse than the site filed here: its missing-action error was
`edits[0]: missing required 'action' field` with no list at all, so **both** of that surface's
discovery routes omitted `edit`.

**Why this file could not see them, which is the part worth keeping.** The reproduction ran
through `doc(update, patch={body_edits})` while editing a librarian-guarded ledger — the one
surface where `edit_file` is refused outright — so the two `edit_file` dispatchers were
structurally outside it. Nothing here was careless; the scope was narrow because the
reproduction was. Recorded as `bug-fix-session-log:F-117`.

**And the documented surfaces were correct throughout, which is why nobody caught it by
reading.** `edit_file`'s JSON input schema enumerates all five actions at both the single-edit
and batch keys (`src/tools/edit_file/mod.rs:412`, `:449`), and `doc`'s `body_edits` description
points at that shape rather than copying it. A caller who checked the schema concluded the tool
was fine; only a caller who *probed* was misinformed. That is the inverse of the usual
doc-vs-code drift, and it rules out the schema as a fourth site.

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

### Archaeology — the string was never wrong at its own layer, for ~3.5 months

`git log -S` on both literals, re-derived 2026-09-07:

| string | site | introduced |
|---|---|---|
| `expected replace, insert_before, insert_after, or remove` | `plan_section_edit` | `4991cc21`, **2026-03-23** |
| `Allowed actions: replace, insert_before, insert_after, remove, edit.` | `apply_body_edits` | `f351e1a2`, **2026-05-25** |

`f351e1a2` is where `edit` arrived — *"defense-in-depth + surgical `body_edits[]`"*, +446 lines
to `src/librarian/tools/update.rs`, the file holding `apply_body_edits`. So the two messages
have disagreed for roughly **three and a half months**, not days.

**This closes off the reading a future reader reaches for first.** *"Someone added `edit` and
forgot to update the string"* is the natural story, and it leads straight to the repair
§ *Root cause* rules out — editing `edit_markdown.rs:281`. The archaeology shows there was never
a moment when that string was wrong **about `plan_section_edit`**: that function has not
implemented `edit` before or since. Nothing was forgotten. A caller-layer action was added above
a callee whose own enumeration stayed correct, and the callee's message went on being surfaced
to the caller unchanged.

**Corrected from a relayed version, and the correction strengthens it.** `codescout-ae` offered
this archaeology with `09981399` (2026-05-02) for the message and `af974c0a` (2026-09-03) for
`edit`. Re-derived here, both are wrong: `09981399` is a refactor that *moved* the string
(*"split markdown.rs into read_markdown + edit_markdown"*), and `edit` predates `af974c0a` by
three months. The conclusion survives — and a 3-day divergence would read as an oversight where
a 3.5-month one reads as structural, which is the whole point of the entry. Approach credited to
`codescout-ae` (sessionId `cda3afe5-17b8-4863-9f4c-9fe4eadbc17b`); dates re-derived rather than
taken, after that session had already corrected three of its own numbers in one evening.

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

**Fixed 2026-09-07 on `experiments` — `491ed828`, patch-id
`6771fa53b1db5c23e743ee5d35f0a7bf540b6204`.**

`src/tools/markdown/edit_markdown.rs` gains `SECTION_EDIT_ACTIONS` (the five, in one place) and
`require_dispatchable_action(action, prefix)`, called at **all three** dispatchers immediately
after `action` is read and before the `if action == "edit"` branch. The three missing-action
hints now render from the same const, so the two paths at each site cannot drift; `plan_batch`'s
gains a list where it had none. `plan_section_edit`'s own four-member message is untouched, per
the ruling above.

A half-finished sixth action is dead rather than silently partial in **both** orders: listed in
the const but not dispatched, it falls through to `plan_section_edit` and is refused there;
dispatched but not listed, it is refused by the new validator before its branch can run.

<details><summary>Superseded plan, as filed — correct for one of the three sites</summary>

Validate `action` against the caller-level set in `apply_body_edits`,
immediately after it is read (`src/librarian/tools/update.rs:255`), and emit the same five-member
message the missing-action path already emits — so the two paths cannot drift again. Leave
`edit_markdown.rs:281` alone: it is correct for its own callers.

The two messages should be built from **one** source, not two string literals that happen to
agree on the day they are written. A single `const` naming the five, referenced by both the
missing-action hint and the invalid-action error, is what makes the agreement checkable rather
than coincidental.

</details>

## Tests added

Five, and the shape the filing asked for was right — assert on **what a caller receives**, not on
the presence of `edit` somewhere in the file. What it under-specified was the *count*.

**One kill per guarded SITE**, per § *Testing Discipline*: a law implemented at N call sites
needs N kills, and the three here are not substitutable.

| test | site |
|---|---|
| `body_edits_invalid_action_names_the_edit_action_it_dispatches` | `apply_body_edits` |
| `single_edit_invalid_action_names_the_edit_action_it_dispatches` | `edit_file(heading=, action=)` |
| `plan_batch_invalid_and_missing_action_both_name_the_edit_action` | `edit_file(edits=[…])` |
| `every_advertised_body_edit_action_actually_dispatches` | per-member positive control |
| `every_advertised_batch_action_actually_dispatches` | per-member positive control |

**Observed RED, per site, by mutating the PRODUCTION path** — not the tests' inputs, and not all
three at once, which would have been one aggregate kill saying nothing about the other two.
Disabling each site's `require_dispatchable_action` call **alone** reddened exactly that site's
test while the other two stayed green; the site-3 run put 55 action-related tests up and failed
precisely one. Each failure printed the original defective string,
`expected replace, insert_before, insert_after, or remove`.

**Two directions, because the invalid-action assertions are blind to one of them.** They read a
*message*, so they cannot see a const advertising an action no dispatcher implements, nor a
validator refusing a valid one. The `every_advertised_*` tests close that by exercising each of
the five for real — per member, not in aggregate.

**The expected list is spelled out in the assertions rather than read from
`SECTION_EDIT_ACTIONS`.** A test that consults the same const the message is built from asserts
the const against itself: delete `edit` from the const and both sides move together, leaving the
test green while the defect returns.

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

Nothing. Fixed and archived; gate green (fmt, clippy `--workspace --all-targets --features
local-embed`, `LEAN exit=0`, `DEFAULT exit=0`), five regression tests, three observed per-site
reds.

One bound this fix does **not** carry, stated so it is not mistaken for covered: the guard
single-sources the *action set*, not the *list of dispatchers*. A fourth dispatcher added later
would reintroduce the defect and red nothing. `F-117`'s **Rests on:** records the same limit.

## References

- `src/librarian/tools/update.rs:242` (`apply_body_edits`), `:254` (five-member hint), `:265`
  (`edit` branch), `:297` (`replace` branch)
- `src/tools/markdown/edit_markdown.rs:281` (`plan_section_edit`, four-member error)
- `get_guide("librarian")` § *Choosing a mode — anti-patterns* — the whole-section-replace data
  loss this action exists to avoid
- `docs/issues/archive/2026-08-19-guarded-artifact-preamble-cannot-be-edited.md` — the adjacent
  known limit of `body_edits`' section scoping
