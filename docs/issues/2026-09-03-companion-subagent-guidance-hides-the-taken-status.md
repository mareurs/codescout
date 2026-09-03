---
status: open
opened: 2026-09-03
closed:
severity: medium
owner: marius
related: []
tags:
- cluster/doc-contradicted-by-code
kind: bug
---

# BUG: the companion plugin tells every subagent to query `status="open"` alone, hiding the `taken` state the code just gained

## Summary

`claude-plugins/codescout-companion/hooks/subagent-guidance.mjs:61` injects this into every
subagent dispatch:

```
• Bug/regression hunts: doc(action="find", kind="bug", status="open") — the known-bug ledger.
```

`status="open"` alone hides `taken`, `investigating` and `zombie`. The `taken` state landed on
`experiments` today (`80f97731`, merged at `ed8843cf`), and that branch taught **every**
triage surface in the codescout repo about it — `CLAUDE.md`, `docs/issues/_TEMPLATE.md`,
`docs/TEAM-ONBOARDING.md`, `.codescout/system-prompt.md`, `src/prompts/guides/tracker-conventions.md`,
`src/prompts/guides/project-activation-bootstrap.md`, `src/prompts/guides/librarian.md`. It
could not reach this one, because this one lives in a different repository.

The surface it missed is the one that reaches **subagents**, which do not read `CLAUDE.md`.

## Symptom (Effect)

A subagent dispatched today receives guidance whose query cannot return a bug another session
is actively holding. The stated purpose of the line — *"Don't re-report a filed bug as new;
mark rediscoveries KNOWN with the ledger path"* — is exactly what the omission defeats: a
`taken` bug is the one most likely to be under someone's hands right now, and it is invisible
to the query the subagent was told to run.

## Reproduction

```
grep -n 'status="open"' claude-plugins/codescout-companion/hooks/subagent-guidance.mjs
# 61:• Bug/regression hunts: doc(action="find", kind="bug", status="open")
```

Control — the same advice in codescout, post-merge, at `ed8843cf`:

```
src/prompts/guides/project-activation-bootstrap.md:
  doc(action="find", kind="bug",
      filter={"status": {"in": ["open", "taken", "investigating", "zombie"]}})
```

## Environment

Linux. `claude-plugins/codescout-companion`, hook `subagent-guidance.mjs`, active on every
profile in this checkout. codescout at `experiments` `ed8843cf`.

## Root cause

Straight `IC-11`: the statement was true when written — before `taken` existed, `status="open"`
was very nearly the right query, missing only `investigating` and `zombie` — and the code then
gained a capability the prose does not know about.

What makes it survive is the **repo boundary**. `prompt_surfaces_reference_only_real_tools`,
`claude_md_contains_no_deprecated_tool_names` and `reader_docs_contain_no_retired_call_forms`
all scan paths under the codescout repo root. The companion plugin is a sibling checkout, so no
codescout gate can see it, and a feature branch sweeping "every triage surface" enumerates the
surfaces its own `git grep` returns. The sweep was complete over the population it could
address.

`companion_surfaces_reference_only_real_tools` (`src/server.rs`) is the one gate that *does*
reach into the plugin — so the crossing is already built. It checks tool **names**, not status
vocabularies, so this line passes it: every tool named is real.

## Evidence

### The omission is not the older three-status one

This is not simply "the pre-`taken` query". Even before today, `status="open"` alone hid
`investigating` and `zombie`, and `CLAUDE.md` has warned about that for weeks. `taken` makes it
worse in kind rather than degree: `investigating` and `zombie` are states a reader might
reasonably deprioritise, whereas `taken` means *another live session is working this right
now*, which is the single most important thing a subagent about to file a duplicate needs to
know.

### Measured cost, same day, in this session

Filing a bug earlier today, a worktree-scoped `doc(action="find", kind="bug")` returned neither
an open severity-high bug covering the same class nor two 2026-09-02 precedents. A near-
duplicate was written and discarded only because `librarian(action="doctor")` was run for an
unrelated reason. That was a different mechanism (worktree row scoping —
`docs/issues/2026-09-03-reindex-walks-zero-files-in-a-worktree-and-reports-success.md`), and it
is cited here because it is the same *consequence*: a ledger query that returns a quietly short
list, and a duplicate filing as the cost.

## Hypotheses tried

1. **Hypothesis:** the codescout branch simply missed this file.
   **Test:** `git grep` for the guidance string inside the codescout repo.
   **Verdict:** rejected — the string exists only in the sibling `claude-plugins` checkout.
   The branch's sweep was complete over every surface reachable from its own repo root.

2. **Hypothesis:** an existing gate should have caught it.
   **Test:** read `companion_surfaces_reference_only_real_tools`.
   **Verdict:** rejected, and this is the useful half. That gate does cross the repo boundary,
   so the crossing exists and is maintained — it just asserts about tool *names*. Every tool
   this line names is real; the defect is in an argument value. The gate is not wrong, its
   question is.

## Fix

1. `claude-plugins/codescout-companion/hooks/subagent-guidance.mjs:61` →
   `doc(action="find", kind="bug", filter={"status": {"in": ["open", "taken", "investigating", "zombie"]}})`,
   with the one-line gloss the codescout surfaces carry (`taken` = a live session holds it).
2. **The general fix is the interesting one.** `companion_surfaces_reference_only_real_tools`
   already reaches into the plugin. Extending it — or adding a sibling — to assert that any
   `kind="bug"` triage query in a companion surface names the full non-terminal status set
   would close the class rather than the instance. Without that, the next status added repeats
   this exactly, and the branch that adds it will again sweep its own repo completely.

SHA: *pending.* patch-id: *pending.*

## Tests added

None yet. Item 2 above is the test.

## Workarounds

Subagents dispatched into this checkout should treat the injected bug query as a lower bound
and run the four-status form from `get_guide("project-activation-bootstrap")` instead.

## Resume

Apply fix 1 in `claude-plugins` (its own repo, its own gate; cite the SHA with the
`<repo>:<sha>` prefix). Then decide fix 2 on its own merits — it is a gate-design question,
not a follow-up to the one-line edit, and it is the only part that prevents recurrence.

## References

- `claude-plugins/codescout-companion/hooks/subagent-guidance.mjs:61`.
- `src/server.rs` — `companion_surfaces_reference_only_real_tools`, the existing cross-repo
  gate whose scope this would extend.
- `80f97731` (`feat(librarian): accept taken as a bug status and make it reachable`) and
  `1f3f00a6` (`docs: teach every triage surface about taken`), merged at `ed8843cf` — the
  capability, and the sweep that could not reach across the repo boundary.
- `docs/architecture/companion-plugin.md` — hook inventory and cross-repo flow.
