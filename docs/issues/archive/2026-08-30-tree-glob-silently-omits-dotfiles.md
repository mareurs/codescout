---
kind: bug
status: fixed
tags:
- tree
- glob
- negative-results
- hidden-files
closed: 2026-08-31
opened: 2026-08-30
owner: marius
related: []
severity: medium
---

# BUG: `tree(glob=...)` returns `0 files` for dot-prefixed files that exist and are tracked

## Summary

`tree` silently prunes dot-prefixed entries — **files and directories both** — from glob
results, and offers no `include_hidden` parameter to opt in, unlike its sibling `grep`. The
result is a bare `0 files`, indistinguishable from genuine absence. Because hidden
*directories* are pruned with their whole subtree, `tree` cannot see any part of `.github/`,
so for this tool the repo has no CI configuration. It misled this session into stating that
a tracked, committed config file was absent.
## Symptom (Effect)

```
tree(glob=".pre-commit-config.*")        → 0 files
tree(glob="**/.pre-commit-config.yaml")  → 0 files
```

Both for a file that is present, tracked, and not ignored:

```
$ ls -la .pre-commit-config.yaml
-rw-rw-r-- 1 marius marius 793 May 16 08:21 .pre-commit-config.yaml

$ git ls-files .pre-commit-config.yaml
.pre-commit-config.yaml

$ git check-ignore -v .pre-commit-config.yaml
check-ignore exit: 1          # 1 = NOT ignored
```

The same file is found by the sibling tool when hidden files are requested:

```
grep(pattern="pre-commit", glob=[...], include_hidden=true)
  → .pre-commit-config.yaml (2)
       1- # Pre-commit hooks for codescout.
       2: # Install with: pre-commit install
```

No error, no warning, no scope note. Just `0 files`.

## Reproduction

```
git rev-parse HEAD          # bdfd7a62 at time of filing
tree(glob=".pre-commit-config.*")
tree(glob="**/.pre-commit-config.yaml")
```

Both return `0 files`. Any dot-prefixed path reproduces it — `.gitignore`,
`.env.example`, `.github/workflows/ci.yml`.

## Environment

Linux, `experiments` @ `bdfd7a62`. codescout MCP server, release binary built
2026-08-30T23:39:53+03:00, symlinked from `~/.cargo/bin/codescout`. Server PID 1012762,
started 23:40:22 — i.e. running the freshly built binary, so this is not a stale-binary
artefact.

## Root cause

**Measured 2026-08-30** — the walker prunes dot-prefixed entries before globbing. It is
not a glob-matching problem.

The distinguishing test was a fully-literal glob with no wildcard anywhere, which a
glob-matcher explanation predicts would succeed:

```
tree(glob=".pre-commit-config.yaml")     → 0 files      # literal, no wildcard
tree(glob=".github/workflows/ci.yml")    → 0 files      # literal, dot on the DIRECTORY
```

Both fail. So:

- **Confirmed:** the walk excludes hidden entries, and the exclusion happens before any
  pattern is applied — no pattern can reach them.
- **Rejected:** "a leading `.` does not match `*`/`**`", the standard shell-glob rule. It
  would have predicted the literal forms succeed. They do not.

The second probe widens the scope beyond what the title says: in `.github/workflows/ci.yml`
the dot is on the **directory**, so hidden *directories* are pruned as well and their whole
subtree is unreachable. `tree` therefore cannot see any part of `.github/`, `.githooks/`,
`.codescout/`, or `.superpowers/` — for `tree`, this repo has no CI configuration.

Still unread at the source: which layer applies the exclusion, and whether it is a
`WalkBuilder::hidden(true)` default (the `ignore` crate's) or an explicit filter. That
distinction decides whether the fix is one builder line or a filter change, but it does not
change the observable contract above, which is measured.
## Evidence

### The false conclusion it produced, in this session

This is the part worth preserving. The sequence was:

1. `tree(glob=".pre-commit-config.*")` → `0 files`
2. I wrote, in user-facing text: *"No `.pre-commit-config.yaml`"*, and began reasoning
   about what the CONTRIBUTING.md hook reference could otherwise mean.
3. A later `grep` with `include_hidden=true`, run for an unrelated reason, returned the
   file with two matching lines.

The zero was believed because nothing about it looked uncertain. It took an unrelated
query to contradict it.

## Hypotheses tried

1. **Hypothesis:** the file is gitignored, and `tree` correctly respects `.gitignore`.
   **Test:** `git check-ignore -v .pre-commit-config.yaml`.
   **Verdict:** rejected — exit 1, not ignored. `tree`'s documented gitignore behaviour
   does not explain this.
2. **Hypothesis:** the file is untracked/local-only.
   **Test:** `git ls-files .pre-commit-config.yaml`.
   **Verdict:** rejected — tracked.
3. **Hypothesis:** the glob form was wrong.
   **Test:** tried `.pre-commit-config.*` and `**/.pre-commit-config.yaml`.
   **Verdict:** rejected for the tested forms — both `0 files`.
4. **Hypothesis:** it is the standard shell-glob rule that a leading `.` does not match a
   `*` or `**` segment — i.e. a matcher problem, fixable by writing a better pattern.
   **Test:** a fully-literal glob with no wildcard at all: `tree(glob=".pre-commit-config.yaml")`.
   **Verdict:** **rejected, measured 2026-08-30** — still `0 files`. No pattern reaches
   these entries, so no pattern is the fix.
5. **Hypothesis:** it affects only hidden *files*, so hidden directories still traverse.
   **Test:** `tree(glob=".github/workflows/ci.yml")` — a non-hidden file inside a hidden
   directory, known to exist (its matrix was read earlier this session).
   **Verdict:** **rejected, measured 2026-08-30** — `0 files`. Hidden directories are
   pruned too, taking their whole subtree with them.
## Why this is worth more than its size

It is a **negative result that does not name its scope**, which this project has an ADR
about: `docs/adrs/2026-08-27-negative-results-name-their-scope.md`. The rule there is to
name the scope examined when a zero is suspicious and stay silent when it is trustworthy.
A `0 files` from a tool that silently excluded an entire file class is the exact case the
ADR exists for — the caller cannot tell exclusion from absence, so the zero is untrustworthy
and says nothing about it.

It also breaks the principle the sibling tool already follows: `grep` has `include_hidden`
and defaults it off, which is fine *because the parameter exists* — a caller who cares can
ask. `tree` gives the caller no way to express the same intent.

## Fix

**Fixed 2026-08-31 on `experiments`, in three commits**, each gate-green and
live-verified after `cargo rb` + `/mcp`:

| commit | patch-id | what |
|---|---|---|
| `799e5dc6` | `67f9094a40e4a2872fac52d316b22d641fbd7ba1` | `include_hidden` + the withheld-count warning, glob **and** list mode |
| `2f434fba` | `c409c4ca1e3844a3bdfa02b678731d4aa7613e0b` | a pruned hidden directory counts as ONE entry, not as its subtree |
| `390bf4f0` | `227d98a5400e3eca0fa8997fe46c12f2ab67a2da` | a cap-truncated tally refuses cross-run comparison |

**This section's own analysis was right about the important thing** and is kept above
rather than replaced: it called the warning, not the parameter, the change that closes
the class, and that is exactly how it shipped. The parameter alone would have left a
silent `0 files` for everyone who did not already know to pass it.

Live verification on the rebuilt binary:

```
tree(glob=".github/workflows/*.yml")        → 0 files + "2 matching files … were withheld"
tree(glob="**/*.yml")                       → 1 file  + "2 … withheld"
tree(glob="…", include_hidden=true)         → both files returned
tree(path=".")                              → 22 entries + "16 hidden entries not listed"
tree(path=".", include_hidden=true)         → 38 entries = 22 + 16
tree(path="src/tools")                      → 33 entries, no note   (silence control)
```

### Three things this file did not know

1. **The zero is the milder half.** `**/*.yml` returned **1 of 3** — a plausible
   *non-zero*, so no suspicious-zero heuristic fires and the reader has no prompt to
   doubt it. `grep`'s `completeness_warning`, the obvious model, gates on an empty result
   and could not have reached this case; `tree`'s walk opens no files, so it can afford
   the exact count instead of the proxy.
2. **List mode had the same defect plus a worse one.** `list_dir_impl` had its own
   `.hidden(true)` and never read `include_hidden` — so after the first commit the
   parameter was *accepted by the schema and silently ignored*, a worse contract than an
   absent one. It is also the sharper case: list mode sets `git_ignore(false)` /
   `ignore(false)`, so hidden is its **only** exclusion, and the listing shows gitignored
   `target/` while omitting `.github/` — reading as complete precisely because it shows
   what other tools hide. Reported by `codescout-fe`.
3. **Placement decides whether any of it is read.** `format_compact` output passes
   through `truncate_compact` (2 KB soft cap) and a 100-file listing runs several times
   that, so a note appended after the list is cut on exactly the results big enough to
   need it. The glob note leads; the list note joins the cap signals below the header via
   `insert_below_header`. The first revision appended both.
## Tests added

Thirteen in `src/tools/tree.rs`, every behavioural claim mutation-checked — they were
written after the implementation and went straight to green, so their ability to fail was
unestablished until each mutation ran.

| mutation | dies |
|---|---|
| warning never rendered | both rendered-output tests |
| warn unconditionally | both silence tests |
| restore `.hidden(true)` | both list-mode tests |
| note appended at the tail | the placement assertion, on its own line |
| count by rel-path component instead of pruning | the subtree-exclusion test, 1 against 4 |
| collapse the truncated note into the plain one | the lower-bound assertion |

This section's original instruction — *assert the **positive** direction, so it cannot
pass by the file disappearing for some other reason* — is honoured: `include_hidden=true`
returns the named files, rather than merely changing a count.

**Three defects in my own tests, each found only by mutation:**

- `rendered.contains('2')` scanned the whole output **including tempdir paths**, so a
  temp path containing a `2` would satisfy it whatever the warning said. Now matches the
  exact phrase.
- `contains("violations[")` matched the appended generic shape line rather than the
  truncation line. Scoped to the line under test.
- One mutation **survived**, and the fault was the mutation: it stopped the pruning but
  kept counting by entry *name*, and the fixture's only dot-named entry is `.github`
  itself, so the tally stayed 1 at every depth. Reading a surviving mutation as "the test
  is weak" would have weakened a guard that works.

The hidden-**directory** case is the primary fixture rather than an add-on:
`.github/workflows/*.yml` has visible filenames and is hidden by its directory, so a
dotfile-only fixture would have gone green while `.github/` stayed invisible — the
`## Resume` section called this out and it was right.

The capped-tally wording is tested on a **pure function**, deliberately: reproducing it
through a walk needs a run that both hits the entry cap and prunes a hidden entry before
breaking, and `ignore::Walk` guarantees no such ordering — that test would pass or fail by
luck, which is worse than none.
## Workarounds

Use `grep(pattern=..., include_hidden=true)` to establish existence of a dot-prefixed
path, or `ls -la` / `git ls-files` via `run_command`. Do not treat `tree`'s `0 files` as
evidence of absence for any path whose first character is `.`.

## Resume

Root cause is measured; what remains is the source read. Find where the walk excludes
hidden entries (likely a `WalkBuilder::hidden(...)` default in `tree`'s implementation) and
decide between the two fix directions under *Fix*. The behavioural facts are already
pinned, so a regression test can be written before the source is read:

- `tree(glob=".pre-commit-config.yaml")` must return the file once hidden entries are
  reachable — a **positive** assertion, so it cannot pass by the file vanishing.
- `tree(glob=".github/workflows/ci.yml")` must return it too — this is the one that guards
  the *directory*-pruning half, which a file-only fix would leave broken while the first
  test went green.
## References

- `docs/adrs/2026-08-27-negative-results-name-their-scope.md` — the rule this violates
- `CONTRIBUTING.md` § *Local Embedding (ONNX) Tests* — cites the pre-commit hook whose
  config this bug hid
- `docs/PROBES.md` — instrument index; a tool whose zero cannot be trusted belongs in the
  same conversation as its "Know before you run it" column
