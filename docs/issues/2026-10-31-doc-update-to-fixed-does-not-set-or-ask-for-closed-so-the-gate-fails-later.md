---
id: '762915bf35f7a909'
kind: bug
status: open
title: 'BUG: doc(update, patch={status: "fixed"}) on a bug file neither sets nor asks for `closed:`, so the gate fails in the next session that runs it'
tags:
- cluster/declared-not-wired
- librarian
- frontmatter-gate
- doc-update
opened: 2026-10-31
owner: marius
severity: low
---

# BUG: doc(update, patch={status: "fixed"}) on a bug file neither sets nor asks for `closed:`, so the gate fails in the next session that runs it

## Summary
The gate (`tests/issue_frontmatter.rs`) refuses a live bug file with `status: fixed` and no `closed: YYYY-MM-DD`. The tool that flips a bug to `fixed`, `doc(action="update")`, writes the status and does nothing about `closed`, and says nothing. The requirement is written in the bug template and enforced by the gate, but the one call that performs the flip never meets it.

## Class fit
Each piece is correct on its own: the template says to fill `closed` when the status flips, the gate test enforces it, and `doc(update)` writes the status it is given. No call site connects the three, so the rule reads as shipped and the failure turns up later, in another session's gate. That is the trait of this class: a surface declares a requirement that production never reaches. It differs from the class's other members in that the declared thing is an obligation on the caller, not a capability of the tool.

## Symptom (Effect)
On 2026-10-31 a gate on a merged tree failed in both the lean and default lanes with one failing test:

```
every_tracked_issue_file_has_wellformed_frontmatter
docs/issues frontmatter is malformed (1 problem(s) over 146 files). Repair the YAML syntax in place; do not loosen the rule:
docs/issues/2026-10-29-staging-a-split-hunk-with-update-index-costs-two-refusals-before-the-recipe-is-named.md: `status: fixed` needs `closed: YYYY-MM-DD`, found None
```

The file's frontmatter at the time read `status: fixed` and `opened: 2026-10-29`, with no `closed` key. A subagent had set the status with `doc(update)` and reported the call as successful. I did not see the call's response myself.

## Reproduction
Not run as a clean repro. The steps, from the code and the observed state:
1. Take an open bug file.
2. `doc(action="update", id=<id>, patch={status: "fixed"})`.
3. `scripts/with-slot.sh cargo test -p codescout --test issue_frontmatter`. Expected failure: `status: fixed needs closed`.

## Environment
`experiments`, a git worktree of the repo, `doc(update)` pinned to that worktree with `workspace=`.

## Root cause
No production code sets or requires `closed` on a status change. Checked on 2026-10-31:
- `src/librarian/tools/update.rs` does not contain the string `closed`.
- The literal `"closed"` appears in `src` only in `legibility_scan` (a different feature: backlog rows), `catalog/augmentation.rs`, two test fixtures (`src/cli/doc.rs`, `src/tools/markdown/frontmatter.rs`) and the tracker-conventions guide.
- The gate rule is `tests/issue_frontmatter.rs:203-208`: `status: fixed` needs a `closed` date; `mitigated` does not.
- The template (`docs/issues/_TEMPLATE.md`) says to fill `closed` by hand when the status flips.

## Evidence
- The failing output above, and the file's frontmatter as read from disk.
- Setting it afterwards worked: `doc(action="update", id=<id>, patch={extra: {"closed": "2026-10-31"}})` wrote the key, and `issue_frontmatter` then passed (18 of 18).
- Seen in this session on one file. Several other agents set `closed` by hand and so did not trip it.

## Hypotheses tried
None needed for the cause. Not checked: whether `doc(update)` already warns in its response when a bug is set to `fixed` without `closed` (I did not see the agent's response).

## Fix
Not done. Options:
- When a bug file's status becomes `fixed` and the patch carries no `extra.closed` and the file has none, set `closed` to today's UTC date, as `doc(create)` now does for `opened`.
- Or refuse the update with a message naming the fix, as `doc(create)` does for a missing cluster tag.
Stamping fits the earlier decision for `opened` (a value the server always knows).

## Tests added
N/A: no fix yet.

## Workarounds
Pass `extra={"closed": "<YYYY-MM-DD>"}` in the same `doc(update)` that sets `status: fixed`.

## Resume
Choose between the stamp and the refusal, then add a test that flips a bug to `fixed` and reads the file back for a `closed` date. Watch for the same gap with `wontfix`: the gate does not require `closed` there, but the template says to fill it.

## References
- `tests/issue_frontmatter.rs:203-208`
- `docs/issues/_TEMPLATE.md` (status field semantics)
- `docs/issues/2026-10-30-doc-create-for-a-bug-file-does-not-stamp-or-require-opened-so-the-frontmatter-gate-fails-in-another-session.md` (the same shape, for `opened`)
