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

# BUG: the `/.codescout/projects/` ignore comment's "no untracked content" clause holds only in a fresh clone or CI — and the dirs behind it come from an unvalidated project id

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

`experiments` at `88316ac9`, Windows VDI, codescout MCP with eight entries present under
`.codescout/projects/`: `Mercury BOM`, `Mercury MRP Automation`, `codescout`,
`m365-data-agent`, `m365-mcp`, `researcher`, `servicenow-mcp`, `web`. None of them is a
subdirectory of the codescout checkout, and only `codescout` and `researcher` correspond to
anything the operator works in regularly — `researcher` is not a `codescout-ecosystem`
umbrella member, and no umbrella member other than codescout itself appears. See the review
addendum below for what actually creates these.

Contrast host: Linux, `9cbe4002`, same operator, same registry, nine repos with a
`.codescout/projects/` tree — zero untracked entries in any of them.
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

## Review addendum — cross-host sweep and re-diagnosis (2026-08-08)

Added by the review pass that owns `6f261da9`. The observation above is real and the
third-clause correction is accepted. The **diagnosis** is wrong, and it aims the fix at
the wrong layer.

### The sweep: nine repos, zero `??`

Every root registered in `~/.config/librarian/workspace.toml` — 10 `[[roots]]` plus both
umbrellas, 15 paths — checked on the Linux host at `9cbe4002`:

| repo | `projects/` entries | tracked | `??` | why it is clean |
|---|---|---|---|---|
| codescout | 9 fixtures | 53 | 0 | no ignore rule; nothing generated lands there |
| `~/personal` | `personal` | 10 | 0 | structural allow-list (see Fix) |
| mirela/backend-kotlin | 5 | 27 | 0 | all content tracked |
| eudis | 13 | 0 | 0 | blanket `.codescout/` |
| wingman | 8 | 0 | 0 | blanket `.codescout/` |
| claude-plugins | `buddy`, `mcp-server` | 0 | 0 | blanket `.codescout/` |
| mirela/eduplanner-site | `optaplanner` | 0 | 0 | the directory is **empty** |

No `projects/` tree at all: `prompt-engineering`, `llm-proxy`, `RAG-reranker`,
`smart-promoter`, `claude-code`, `mirela/deployment`, `eduplanner-mobile`,
`eduplanner-ui`. Not git repos (container directories): `mirela`, `southpole`,
`invest-europe`. Dead registry entry: `code-explorer` points at a path that no longer
exists.

**Total untracked entries under `.codescout/projects/` across every registered root: 0.**

So "the normal state for the cross-project flows CLAUDE.md documents" does not hold —
nine repos run exactly those flows and produce no `??` at all. The VDI is an outlier, not
the baseline.

### The phantom directories are here too, and they are empty

`.codescout/projects/<id>/` exists for ids that are **not** subdirectories of their repo:

- `claude-plugins/.codescout/projects/mcp-server/` — no `mcp-server` directory exists in
  that repo. Empty.
- `mirela/eduplanner-site/.codescout/projects/optaplanner/` — `optaplanner` is a
  **sibling** at `mirela/optaplanner`, not a child. Empty.

Git cannot see an empty directory. That, not the absence of the mechanism, is why this
host looks clean.

### The actual root cause

`Workspace::memory_dir_for_project` resolves any id that is not the root project into
`self.root/.codescout/projects/<id>/memories`, and its own docstring states the behavior
is deliberate:

> An unknown `project_id` is treated as a sub-project (returns a per-project subdirectory
> under `projects/<id>/`). Previously defaulted to the root memory dir on unknown ID,
> which silently co-mingled memories from typos or stale IDs …

The id is validated against nothing. Any string — a stale id, a typo, a foreign workspace
name — materializes a directory under the *active* workspace's root. The two empty
directories above are that mechanism caught in the act. On the VDI something additionally
*wrote* stubs into them, which is the only reason they became visible to `git status`.

Two independent tells in the original evidence agree. `Root: work\Mercury BOM` occupies
the same field as this repo's `Root: tests/fixtures/rust-library` — a *relative
sub-project root*. And `.codescout/projects/codescout/` can only exist when the project
whose id is `codescout` has `relative_root != "."`, which contradicts the checked-in
`.codescout/workspace.toml` (`[[project]] id = "codescout", root = "."`).

So the two populations are not "fixtures vs. foreign workspaces". They are "real
sub-projects" and "directories conjured by an unvalidated id". The second should not exist
at all, which is why no glob can cleanly describe it.

### Measured 2026-08-08 — both populations reproduce on one host

The mechanism above was inferred when this addendum was first written. It has since been
reproduced directly, on `experiments` at `9cbe4002`, with no misresolution involved:

- `memory(action="write", topic="…", project_id="zz-definitely-not-a-project")` → `"ok"`,
  directory count 9 → 10, one new `?? .codescout/projects/zz-definitely-not-a-project/`.
  **This is the VDI's eight.**
- `memory(action="read", topic="…", project_id="zz-read-path-probe")` → creates the directory
  and leaves it **empty**, so the count goes 10 → 11 while `git status` still shows one `??`.
  **This is this host's two.**

So the difference between the hosts is read-vs-write traffic against a bad `project_id`, not
configuration and not workspace-root misresolution — the review pass's own first guess, which
was wrong in the same way the original report was. Tree restored afterwards: count back to 9,
`git status --porcelain .codescout/projects` empty. Full transcript in the sibling bug file.

### Two corrections to the report above

- `## Environment` claims the eight are "four repos in the `codescout-ecosystem` umbrella
  plus four unrelated". The umbrella is codescout, prompt-engineering, claude-plugins,
  llm-proxy. The listed eight include `codescout` and `researcher` but none of
  prompt-engineering, claude-plugins, or llm-proxy — and `researcher` is not a member.
- The cited tip `88316ac9` is two commits stale, and the untracked set is exactly the
  thing that moves between commits.

## Fix

Two layers, in this order.

**1 — Upstream, the real fix.** Validate `project_id` in
`Workspace::memory_dir_for_project`. An id that is neither the root project nor a declared
or discovered sub-project of this workspace should be an error, not a freshly created
directory. Filed separately as
`docs/issues/2026-08-08-memory-dir-for-project-materializes-any-id.md`.

**2 — Belt and braces in `.gitignore`.** Use the structural pattern already running in the
operator's `~/personal` repo. It is strictly better than Options A/B/C below:

```gitignore
/.codescout/*
!/.codescout/memories/
!/.codescout/projects/
/.codescout/projects/*/*
!/.codescout/projects/*/memories/
```

It enumerates no ids, so it never goes stale; it keeps **every** project's `memories/`
visible, including projects that do not exist yet; and it ignores anything generated that
lands elsewhere under the tree. Verified non-destructive on 2026-08-08 —
`find .codescout/projects -type f -not -path '*/memories/*'` returns nothing, so all 53
tracked files sit inside a `memories/` directory and survive the pattern intact.

The three original options are superseded:

- **A** (ignore the eight by name) — stale on the ninth workspace, by its own admission.
- **B** (`projects/*/` plus `!` re-includes for the nine fixture ids) — **rejected.**
  Enumerating ids reintroduces exactly what `6f261da9` removed: a rule that silently hides
  the tenth fixture project. The structural pattern above is the version of B that works,
  because it re-includes by *shape* rather than by name.
- **C** (relocate generated foreign-workspace state) — right instinct, wrong premise. There
  is no foreign-workspace-state feature to relocate; there is an unvalidated id to reject.
  That is fix 1.

The `.gitignore` comment's third clause still needs the correction this report asks for:
the tree holds no untracked content **in a fresh clone or in CI** — not "at all".
## Tests added

None. A `.gitignore` policy has no unit-test surface, and the failure is environmental —
invisible in a fresh clone and in CI, which is precisely how the premise got in. Closest
mechanical guard would be a hygiene check asserting `git status --porcelain
.codescout/projects` is empty, which only holds on the machines where nothing is wrong.

## Workarounds

Filter it at read time — `git status --porcelain | grep -v '.codescout/projects'` — or
add the eight to `.git/info/exclude`, which is per-clone and commits nothing.

## Resume

Fix 1 is the load-bearing one and is tracked in its own bug file — do not close this one on
the `.gitignore` change alone.

1. On the **VDI**, before anything else: run `project_status` there and compare each
   project's `relative_root` against the workspace `root`. That says whether the eight are
   unvalidated ids (expected) or genuine mis-discovery, and it is not reproducible from the
   Linux host.
2. Apply fix 2 (the structural `.gitignore` pattern) plus the third-clause comment
   correction in the `Deliberately NOT` block, in one commit.
3. Re-check `git status --porcelain .codescout/projects` on both hosts afterwards. On Linux
   it was already empty, so the pattern must not *change* anything there — that is the
   regression check.
4. Do not re-propose Option B in its id-enumerating form; it is rejected above with reasons.
## References

- `6f261da9` — the review-pass commit that set the current rule (finding 5)
- `49a1e1ac` — the blanket rule it replaced
- `.gitignore` lines ~72-80
- `src/workspace.rs` — `Workspace::memory_dir_for_project`, the unvalidated-id site
- `docs/issues/2026-08-08-memory-dir-for-project-materializes-any-id.md` — the upstream bug
- `~/.config/librarian/workspace.toml` — the registry the cross-host sweep enumerated
- PR https://github.com/mareurs/codescout/pull/10
- PR https://github.com/mareurs/codescout/pull/11 — this file, and the review addendum
