---
id: 03ead8db9900e24b
kind: bug
status: wontfix
title: 'KNOWN/duplicate: nested hook state is untracked but not ignored — root cause filed in claude-plugins, and the fix proposed here is refuted'
owners:
- marius
tags:
- cluster/blast-radius-exceeds-visibility
- git
- gitignore
- companion-hooks
- shared-checkout
- multi-session
closed: 2026-08-31
duplicate_of: claude-plugins:docs/issues/2026-08-31-buddy-session-dir-treats-cwd-as-project-root.md
---

---

> **Closed 2026-08-31 as a rediscovery, hours after the fact.** Everything below was
> already established by a peer session in this same checkout, and its analysis is better
> than this file's on both ends. Kept as a codescout-side pointer, not as work.
>
> - **Root cause, filed in the right repo:**
>   `claude-plugins:docs/issues/2026-08-31-buddy-session-dir-treats-cwd-as-project-root.md`.
>   `hook_helpers.py:469` builds the session dir as `Path(event["cwd"]) / ".buddy" / sid`
>   and `:605` derives the project root back out of it. A cwd is not a project root, so a
>   session whose cwd is a subdirectory plants its state there. That file also identifies
>   the same defect in the second plugin (`codescout-companion/scripts/detect.py:68-69`,
>   `return cwd / ".codescout"` — a bare join, no walk), which is the `.codescout/`
>   sibling this file observed and did not explain.
> - **The incident and the lesson:** `reconnaissance-patterns:R-150`. The stray directory
>   was `rm -rf`'d on the assumption it was the deleting session's own debris; it was a
>   live peer's. The copy this file found is the *recreated* one, which is how the question
>   the deletion destroyed got answered anyway.
>
> **The fix this file proposed is wrong, and that is the part worth keeping.** It said to
> add `**/.buddy/` and friends — "one line each; no code change". That cannot work.
> `.gitignore:43-44` is `/.buddy/*` followed by `!/.buddy/memory/`, and **git will not
> descend into a directory it has ignored**, so any pattern broad enough to catch a nested
> `.buddy/` makes the root rule's negation unreachable. The remedy is upstream, in the two
> plugins' path resolution, not in this repo's `.gitignore`.
>
> Nothing here was measured wrong — the nine un-ignored files and the `git check-ignore`
> result are reproducible. The error was reaching for a remedy in the surface where the
> symptom appeared, and not looking for prior art on a defect a peer had been narrating in
> the same checkout for an hour.

---

## Summary (as originally observed)

Companion-hook state written into a **subdirectory** of the repo is neither tracked nor
ignored, so `git add -A` stages another session's live scratch state.

Observed 2026-08-31 during the `cluster/` archive backfill. `docs/issues/` held `.buddy/`
and `.codescout/` carrying session `3a6d634e-…` — not this session's id. Nine files, all
reachable by `git add -A`:

```
docs/issues/.buddy/.current_session_id
docs/issues/.buddy/3a6d634e-…/{active_plan.json,cs_tool_log.jsonl,loaded_skills.json,narrative.jsonl,state.json}
docs/issues/.buddy/by-ppid/983773/{session_id,started_at}
docs/issues/.codescout/constitution-seen/3a6d634e-….json
```

## Why the .gitignore looks like it covers this

A gitignore pattern containing a `/` anywhere but at the end is anchored to the directory
of the `.gitignore` itself. So `/.buddy/*` (`:43`) is explicitly root-anchored, and
`.codescout/constitution-seen/` (`:51`) *reads* as unanchored but is not — the mid-string
`/` anchors it to the repo root too. Verified at the bytes:

```
git check-ignore -q docs/issues/.codescout/constitution-seen/<sid>.json  → NOT IGNORED
git add -An docs/issues/.buddy docs/issues/.codescout                     → lists all 9
```

## Impact

Low severity, high nuisance. A session running `git add -A` — the ordinary way to stage a
docs change — commits a peer's in-flight tool log and plan state under an unrelated
message. Same outcome as
`docs/issues/2026-08-31-peer-commit-captures-another-sessions-working-tree.md`, reached by a
second route: there the captured file was a peer's work product, here it is runtime state,
which no reviewer would recognise as out of place.

## Resume

None owed here. Track the fix at
`claude-plugins:docs/issues/2026-08-31-buddy-session-dir-treats-cwd-as-project-root.md`.
Until it lands, stage explicitly (`git add -u <path>`) rather than `git add -A` in any
checkout shared with peer sessions.

