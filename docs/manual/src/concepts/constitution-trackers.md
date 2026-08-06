# Constitution Trackers

> ⚠ **Unreleased — on the `experiments` branch only.** Not in v0.15.0 and not on
> crates.io; the API may change without notice. The full cohort is listed under
> `[Unreleased]` in
> [CHANGELOG.md](https://github.com/mareurs/codescout/blob/experiments/CHANGELOG.md).

A tracker archetype for rules the agent must follow no matter what — enforced
mechanically by hooks rather than by trusting prose.

## What it is not

Not a place for advisory guidance. "Prefer X over Y", "consider Z" belong in a
regular tracker or a memory. A constitution rule is one whose violation should
*stop* a tool call, which means it has to be phrased tightly enough that a hook
can act on it without judgement.

## Enabling enforcement

Two things are required, and the archetype shape is only one of them:

1. The tracker uses the `constitution` archetype — call
   `librarian(action="tracker_design")` for the full shape.
2. The artifact's `tags` **include `"constitution"`**.

Without the tag, codescout-companion's hooks cannot find the tracker and the
rules are inert prose. This is the single most common way a constitution ends up
doing nothing.

## Rule shape

Rules live in the `rules` entry collection:

```json
{
  "id": "C-1",
  "paths": ["**/solver/**", "**/*Constraint*.kt"],
  "title": "Never disable a constraint via weight 0",
  "rule": "A constraint_profiles weight of 0 or 1 is a sentinel, not disabled — read the lambda before touching it.",
  "status": "active"
}
```

`paths` is what splits the two enforcement modes:

| Rule | Enforcement |
|---|---|
| `paths` present | `PreToolUse` deny — the call against a matching path is blocked |
| `paths` absent | `UserPromptSubmit` injection — the rule is put in front of the agent every turn |

`status` is `active` or `superseded`.

## The CLI the hooks call

```bash
codescout constitution-check --path src/solver/Foo.kt   # path-scoped rules
codescout constitution-check                            # global, path-less rules
```

Read-only and fast, because it runs on every tool call. It **always exits 0** and
prints `[]` on any internal error — a hook that could fail closed would make a
catalog hiccup look like a policy violation and block legitimate work.

`--project` overrides the project root, which otherwise defaults to the working
directory.

## Maintaining a constitution

- Add rules with [`append_entry`](entry-citations.md), never by hand-picking the
  next integer.
- **Never delete a rule.** Supersede it: set `status: "superseded"` and point at
  its replacement. A denied tool call cites a `C-N`, and that citation has to
  keep resolving to something after the rule changes.
- When a call is denied citing `C-N`, read that rule's body section before
  retrying — the `## C-N` sections carry the why, the how-to-apply, and the
  evidence that the one-line `rule` field cannot.

## Where this lives

`src/librarian/tools/tracker_design.rs` (`archetype_constitution`),
`src/librarian/tools/constitution_check.rs` (`find_matching_rules`,
`find_global_rules`), and `src/cli/constitution_check.rs` for the subcommand.

## Related

- [tracker_design](tracker-design.md)
- [Entry Citations](entry-citations.md)
- [Routing Plugin](routing-plugin.md) — where the hooks live
