---
status: open
opened: 2026-08-08
closed:
severity: low
owner: marius
related: []
tags: [gitignore, repo-hygiene, workspace]
kind: bug
---

# BUG: the `/.codescout/projects/` ignore decision rests on a premise that is false on any host with registered foreign workspaces

## Summary

`6f261da9` dropped the blanket `/.codescout/projects/` ignore rule, correctly, because
53 tracked memory files live under that tree. Its justification also asserts the path
"has no untracked content at all". On this host it has eight untracked directories of
auto-generated content, so `git status` now carries eight permanent `??` entries — the
same noise-blindness the change was avoiding, arriving from the other side.

## Symptom (Effect)

`git status --porcelain .codescout/projects` on `experiments` at `88316ac9`:

```
?? ".codescout/projects/Mercury BOM/"
?? ".codescout/projects/Mercury MRP Automation/"
?? .codescout/projects/codescout/
?? .codescout/projects/m365-data-agent/
?? .codescout/projects/m365-mcp/
?? .codescout/projects/researcher/
?? .codescout/projects/servicenow-mcp/
?? .codescout/projects/web/
```

Permanent, and they crowd the working-tree noise floor that the ignore file exists to
keep clear.

## Reproduction

On a checkout where codescout has activated any workspace outside this repo:

```
git checkout experiments   # 88316ac9 or later
git status --porcelain .codescout/projects
```

Fresh clones and CI show nothing, which is why the premise reads true from there.

## Environment

`experiments` at `88316ac9`, Windows VDI, codescout MCP with eight foreign workspaces
registered (four repos in the `codescout-ecosystem` umbrella plus four unrelated).

## Root cause

Two different populations share one directory, and the rule was written against only one
of them.

- **Tracked (53 files, 9 projects):** `codescout-embed`, `edit-eval-rust`,
  `java-library`, `kotlin-library`, `librarian-mcp`, `nav-eval-rust`, `python-library`,
  `rust-library`, `typescript-library`. All are **in-repo fixtures** (`tests/fixtures/*`,
  `crates/*`) with hand-authored memories maintained by `chore(memories):` commits. These
  are what `6f261da9` protected, and protecting them is right.
- **Untracked (8 dirs):** the operator's **foreign workspaces**, registered on this
  machine. Their contents are machine-generated stubs, not authored — e.g.
  `.codescout/projects/Mercury BOM/memories/onboarding.md`, 100 bytes, whole file:

  ```
  Languages: python, markdown, bash, css, javascript
  Root: work\Mercury BOM
  Manifest: requirements.txt
  ```

  Written 2026-07-13 by activation, never edited by hand, and regenerable.

So "that path has no untracked content at all" is true of a fresh clone and of CI, and
false of any developer machine that has activated a workspace elsewhere — which is the
normal state for the cross-project flows CLAUDE.md documents. Measured 2026-08-08:
`git ls-files .codescout/projects` → 53, all fixtures; `git status --porcelain
.codescout/projects` → 8, all foreign.

The deeper shape: the tree has no naming or structural boundary between authored and
generated content, so no single glob can express the intent. The comment's confident
absolute is a symptom of that, not sloppiness — it was checked against the only
environment where the question looks settled.

## Evidence

### The rule and its justification, `6f261da9`

```
-# Local codescout search index + per-project caches (regenerated on activate)
+# Local codescout search index (regenerated on activate).
+#
+# Deliberately NOT `/.codescout/projects/`: that tree holds 53 tracked,
+# hand-authored per-project memory files (`*/memories/*.md`, `*.anchors.toml`)
+# maintained by explicit `chore(memories):` commits. Ignoring it would not
+# untrack those, but it would hide every future one from `git status` with no
+# error — and it shadows no cache, since that path has no untracked content at
+# all. The index DB below is the thing that is actually regenerated.
 /.codescout/code-index.db
-/.codescout/projects/
```

The first two clauses are correct and load-bearing. Only the third clause is wrong.

### Prior state

`49a1e1ac` (this branch, pre-review) had added the blanket `/.codescout/projects/`. That
rule would have suppressed the 8 — and silently hidden any future fixture memory. Both
rules are wrong in opposite directions; neither population is served by a single glob.

## Hypotheses tried

1. **Hypothesis:** the 8 dirs are stale leftovers to delete, not something to ignore.
   **Test:** each corresponds to a live registered workspace, and activation rewrites
   them; `.codescout/projects/codescout/memories/` is regenerated for this very repo.
   **Verdict:** rejected — deleting them is a no-op until the next activation.
2. **Hypothesis:** they should be tracked like the fixture memories.
   **Test:** contents are three-line generated stubs naming machine-local absolute roots
   (`Root: work\Mercury BOM`). They are per-operator, not per-repo.
   **Verdict:** rejected.

## Fix

Not yet applied — the choice belongs to whoever owns the `6f261da9` stance.

- **Option A (narrow, no policy change):** ignore the eight by name. Honest, zero risk to
  the fixture files, and stale the moment a ninth workspace is registered.
- **Option B (structural):** `/.codescout/projects/*/` plus `!` re-includes for the nine
  fixture projects. Self-maintaining for new workspaces and it keeps future fixture
  memories visible — but it re-adopts a blanket rule over the tree, which is the thing
  `6f261da9` argued against, so it needs that author's sign-off.
- **Option C (root fix, largest):** stop co-locating generated foreign-workspace state
  with authored fixture memories — write registered-workspace stubs somewhere already
  ignored. Removes the need for any glob gymnastics.

Whichever lands, the `.gitignore` comment needs its third clause corrected: the tree
**does** hold untracked generated content on a developer host, and that is exactly why
the rule is hard to write.

## Tests added

None. A `.gitignore` policy has no unit-test surface, and the failure is environmental —
invisible in a fresh clone and in CI, which is precisely how the premise got in. Closest
mechanical guard would be a hygiene check asserting `git status --porcelain
.codescout/projects` is empty, which only holds on the machines where nothing is wrong.

## Workarounds

Filter it at read time — `git status --porcelain | grep -v '.codescout/projects'` — or
add the eight to `.git/info/exclude`, which is per-clone and commits nothing.

## Resume

Pick between Options A/B/C above with the author of `6f261da9`, then apply it together
with the comment correction in `.gitignore` (the `Deliberately NOT` block, lines ~72-79).
Re-check the untracked set first — `git status --porcelain .codescout/projects` — since
it grows with every workspace activated since 2026-08-08.

## References

- `6f261da9` — the review-pass commit that set the current rule (finding 5)
- `49a1e1ac` — the blanket rule it replaced
- `.gitignore` lines ~72-80
- PR https://github.com/mareurs/codescout/pull/10
