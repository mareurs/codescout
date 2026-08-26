---
status: mitigated
opened: 2026-08-08
closed: 2026-08-08
severity: medium
owner: marius
related: [docs/issues/archive/2026-08-08-gitignore-projects-rule-premise-false-on-a-real-host.md, docs/issues/archive/2026-08-08-memory-dir-for-project-materializes-any-id.md]
tags: [workspace, memory, config, windows]
kind: bug
unverified: no regression test for the config state itself; recurrence is now DETECTABLE but not prevented (doctor's declared_root_missing reports; c0bdeec7 still validates ids against this same file); all three fix actions were on a gitignored file, so the repair half is not verifiable on experiments
---

# BUG: a mis-rooted `.codescout/workspace.toml` declared eight sibling repos as projects of this workspace — the second cause behind the eight VDI directories, and one `c0bdeec7` does not cover

## Summary

This answers Resume item 2 of the archived gitignore bug: the eight untracked
`.codescout/projects/<id>/` directories on the Windows VDI were **not** produced by an
unvalidated `project_id`. They were produced by a `.codescout/workspace.toml` that
declared eight *sibling repos* as projects of the codescout workspace, with roots
relative to `$HOME` while the file sat at the repo root. Under that config
`memory_dir_for_project` was answering correctly. `c0bdeec7` does not prevent recurrence,
because it validates ids **against this same file**.

## Symptom (Effect)

The eight directories are a 1:1 match with the eight non-root project ids the config
declared — including the two nobody would type by accident, `Mercury BOM` (with the
space) and `web`:

```
id = "root"                       (no dir — relative_root == ".")
id = "m365-data-agent"            ?? .codescout/projects/m365-data-agent/
id = "Mercury BOM"                ?? ".codescout/projects/Mercury BOM/"
id = "Mercury MRP Automation"     ?? ".codescout/projects/Mercury MRP Automation/"
id = "web"                        ?? .codescout/projects/web/
id = "codescout"                  ?? .codescout/projects/codescout/
id = "researcher"                 ?? .codescout/projects/researcher/
id = "m365-mcp"                   ?? .codescout/projects/m365-mcp/
id = "servicenow-mcp"             ?? .codescout/projects/servicenow-mcp/
```

## Reproduction

Not a code defect — a config state. On the VDI, before the fix:

```
workspace(action="list_projects")   → the eight ids above, live
workspace(action="status")          → same set
git status --porcelain .codescout/projects   → eight ?? entries
```

## Environment

`experiments` at `5dfea4ba`, Windows VDI. `.codescout/workspace.toml` mtime 2026-07-07;
the eight directories 2026-07-13, six days later.

## Root cause

The config declared project roots relative to the **home directory** while living at the
**repo** root:

```toml
[[project]]
id = "codescout"
root = 'work\claude\codescout'

[[project]]
id = "Mercury BOM"
root = 'work\Mercury BOM'
```

Measured 2026-08-08 — none of those paths exist in the repo, all exist under `$HOME`:

```
ABSENT in repo:  work/Mercury BOM                          exists under HOME
ABSENT in repo:  work/claude/codescout                     exists under HOME
ABSENT in repo:  AgentsToolkitProjects/m365-data-agent     exists under HOME
ABSENT in repo:  work/system/m365-mcp                      exists under HOME
```

The file had been authored for a session whose workspace root was
`C:\Users\MAILINCA.BRN.002`, then persisted into `<repo>/.codescout/`. Every declared
project therefore had `relative_root != "."`, so
`<root>/.codescout/projects/<id>/memories` was the correct answer to the wrong question.

Two independent confirmations that this, not the unvalidated id, produced the eight:

1. **Exact set match.** Eight declared, eight on disk, names matching character for
   character including a space. An unvalidated-id mechanism has no reason to produce
   precisely the declared set and nothing else.
2. **Content split by date.** The eight (2026-07-13) hold only `language-patterns.md` +
   `onboarding.md` — the two auto-generated onboarding topics. The nine tracked fixture
   projects (2026-06-04/05) hold `architecture`, `conventions`, `project-overview` and
   `.anchors.toml`. Two generations of one directory, from two different configs.

### Why `c0bdeec7` does not cover this

`c0bdeec7` rejects a `project_id` that no project owns, validating against the workspace
config. Here the config *owned* all eight ids. `memory(project_id="Mercury BOM")` would
still be accepted post-fix, and would still write inside the codescout repo. The
validation is only as trustworthy as the file it validates against, and that file is
gitignored (`.gitignore:28`) — per-machine, unreviewable.

### The premise correction

The addendum's second tell reads:

> `.codescout/projects/codescout/` can only exist when the project whose id is
> `codescout` has `relative_root != "."`, which contradicts the checked-in
> `.codescout/workspace.toml` (`[[project]] id = "codescout", root = "."`)

The inference is sound; the premise is not. That file is not checked in:

```
$ git ls-files .codescout/workspace.toml
(blank)
$ git cat-file -e HEAD:.codescout/workspace.toml
fatal: path '.codescout/workspace.toml' exists on disk, but not in 'HEAD'
$ git check-ignore -v .codescout/workspace.toml
.gitignore:28:/.codescout/workspace.toml
```

Two hosts, two different local copies of one gitignored path. The contradiction was real
and pointed at a mis-rooted config rather than at an unvalidated id.

## Evidence

### The two views of "what projects exist" disagree

`workspace(action="activate")` reports discovery — correctly repo-rooted:

```
codescout ".", codescout-embed "crates/codescout-embed",
edit-eval-rust/java-library/kotlin-library/nav-eval-rust/python-library/
rust-library/typescript-library under "tests/fixtures/"
```

`workspace(action="status")` and `workspace(action="list_projects")` report the file — the
home-rooted eight. Memory routing followed the file. So activation looked correct while
writes went astray, which is why this survived from 2026-07-07 to 2026-08-08.

## Hypotheses tried

1. **Hypothesis (the addendum's):** the eight are directories conjured by an unvalidated
   `project_id`.
   **Test:** compared the eight against the ids declared in the live config.
   **Verdict:** rejected for this host — exact 1:1 match with the declared set, so the ids
   were valid, not invented. The mechanism is real and reproduced; it produced the two
   *empty* dirs on the Linux host, not these eight.
2. **Hypothesis (the original report's):** the eight are caches for registered foreign
   workspaces, a normal cross-project state.
   **Verdict:** rejected. They are sibling repos mis-declared as sub-projects — an
   abnormal state on one machine, not a class of host.

## Fix

Applied 2026-08-08, all local (the config is gitignored, so only the doc half is
committable):

1. **`.codescout/workspace.toml` rewritten** to declare only this repo's real
   sub-projects, roots relative to the repo, matching what `activate` discovers and what
   the 53 tracked memory files belong to: `codescout` at `.`, `codescout-embed` at
   `crates/codescout-embed`, and the eight `tests/fixtures/*` projects. The header comment
   records what was wrong and that cross-repo grouping belongs in an `[[umbrella]]`.
2. **The eight directories deleted.** Backed up first; contents were the two generated
   topics only. `git status --porcelain .codescout/projects` now empty, `git ls-files` still
   53 — no tracked file touched.
3. **`CLAUDE.md` umbrella section rewritten** — it named an umbrella (`codescout-ecosystem`)
   that exists in no registry, with two of four members wrong. Now describes the mechanism
   and says the name/membership are per-machine, plus an explicit **umbrella ≠ workspace**
   note pointing at this failure mode.

## Tests added

None, and this is the gap worth naming rather than excusing. The defect is a config state,
and the one guard that would catch it belongs in `doctor`: assert every declared
`relative_root` resolves to an existing directory under the workspace root. All eight
entries here would have failed it on 2026-07-07. Filed as the Resume action rather than
implemented, because it is a new doctor check with its own output-shaping decisions.

## Workarounds

Check the config against reality before trusting per-project memory routing:

```
workspace(action="list_projects")      # what the file says
workspace(action="activate", path=…)   # what discovery finds
```

If those disagree, the file is mis-rooted and writes are going somewhere unexpected.

## Resume

**DONE 2026-08-19** — `f632e7ef`, patch-id `757f9d606e2ad65c1e38b685b3f18e2ee3a227e2`.
`scan_declared_project_roots` in `src/librarian/tools/doctor.rs` emits
`declared_root_missing` for every `[[project]]` whose declared root is not a directory
under the root owning the config, sited next to `abs_path_outside_managed_roots` as this
section asked. All eight entries from 2026-07-07 would have fired it. Six tests, six
mutations applied and run, zero surviving. Verified live against this host's real config
(`declared: 1, missing: 0`).

Two things the implementation had to decide that this section did not specify — recorded
because they change what a green result means:

- **The base is read from the same variable as the config**, never derived independently.
  Otherwise a mis-rooted config gets validated against the root it was mis-rooted *from*,
  and passes — which would have made this very defect green.
- **A linked worktree is skipped, and says so.** The config is gitignored, so it does not
  travel into `git worktree add` and discovery there inherits main's copy. The skip is
  stated in `catalog_health.declared_roots.note`; so are unreadable and unparseable
  configs. A silent zero would be indistinguishable from a clean bill of health.

**Still open:** the check REPORTS, it does not prevent — deliberately, per CAP-7 open
decision 1 (the measured cause of codescout's unarchived-bug pile is an unsatisfiable
gate). And this file's own second Resume item stands: **the other machine has not been
re-checked.** The config is per-machine and gitignored, so the Windows VDI's copy is
still unreviewed by anyone; run `codescout doctor` there.
## References

- `docs/issues/archive/2026-08-08-gitignore-projects-rule-premise-false-on-a-real-host.md` — Resume item 2, which this answers
- `docs/issues/archive/2026-08-08-memory-dir-for-project-materializes-any-id.md` — the other cause, fixed in `c0bdeec7`
- `.gitignore:28` — why the config is unreviewable
- PR https://github.com/mareurs/codescout/pull/11

## Fix provenance

- **SHA:** `f632e7ef`
- **patch-id:** `757f9d606e2ad65c1e38b685b3f18e2ee3a227e2`
