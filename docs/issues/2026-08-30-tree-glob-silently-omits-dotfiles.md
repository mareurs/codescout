---
status: open
opened: 2026-08-30
closed:
severity: medium
owner: marius
related: []
tags: [tree, glob, negative-results, hidden-files]
kind: bug
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

Not attempted. Two candidate directions, listed so a later session does not have to
re-derive them:

- Add an `include_hidden` parameter to `tree`, mirroring `grep`'s, defaulting to `false`
  for compatibility. Smallest change; makes the intent expressible.
- Additionally, when a glob returns zero and hidden files were excluded, say so in the
  response — that is what turns an untrustworthy zero into a trustworthy one and is what
  the ADR actually asks for. The parameter alone still leaves a silent `0 files` for
  everyone who does not know to pass it.

The second is the one that closes the class; the first only gives an informed caller an
escape hatch.

## Tests added

None yet — bug is filed, not fixed. A regression test should assert the **positive**
direction (a dotfile IS returned when hidden files are requested) rather than only that
the count changed, so that it cannot pass by the file disappearing for some other reason.

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
