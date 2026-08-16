---
status: open
opened: 2026-08-16
closed:
severity: low
owner: marius
related: []
tags: [cli, arg-parsing, junk-artifact, unreproduced]
kind: bug
---

# BUG: a directory literally named `--help` appeared in the repo root, containing an initialised codescout project

## Summary

A directory named `--help` exists at the codescout repo root, containing
`.codescout/index-state.json`. Something resolved the string `--help` as a
**project path** and initialised a project there. The directory is untracked, so
it is one incautious `git add -A` away from being committed — which, on a branch
where two sessions share a working tree and one has already swept staged files
twice today, is not hypothetical.

Filed with the mechanism **unknown**: the obvious candidates were tested and none
reproduce it.

## Symptom (Effect)

```
$ ls -la -- './--help'
drwxr-xr-x 1 marius marius 20 Aug 16 13:30 .
drwxr-xr-x 1 marius marius 32 Aug 16 13:30 .codescout

$ find './--help' -maxdepth 3
./--help
./--help/.codescout
./--help/.codescout/index-state.json
```

`git status --short` reports it as `?? --help/`.

## Reproduction

**Not yet reproducible — best lead below.** Four plausible invocations were run
against the release binary in a scratch directory (2026-08-16), and every one
left the directory clean:

```
codescout --help                  -> clean
codescout index --help            -> clean
codescout symbols --help          -> clean
codescout start --project --help  -> clean
```

So the arg-leak hypothesis — a `--help` flag consumed as a positional path — is
**refuted for these four forms**. The creating command is still unidentified.

Best lead: the directory's mtime is 13:30 on 2026-08-16. Any shell history or
session transcript covering that minute would name the command directly. Two
sessions were active in this working tree at the time.

## Environment

codescout `experiments`, repo root `/home/marius/work/claude/codescout`. Linux.
Directory created 2026-08-16 13:30 local.

## Root cause

Unknown — see *Hypotheses tried*. What is established is only the effect: some
code path treated `--help` as a project root and ran project initialisation
there, since `.codescout/index-state.json` is written by the indexer rather than
by `mkdir`.

## Evidence

### It is an initialised project, not an empty directory

`.codescout/index-state.json` is index state, so whatever created it got far
enough to activate and index. That rules out a bare `mkdir -- --help` and points
at codescout itself or a wrapper invoking it.

### The four refuted forms

Probe script run 2026-08-16 in a `mktemp -d`, each invocation followed by a
check for a `--help` directory; all four reported `clean`. Recorded here so a
later session does not re-walk them.

## Hypotheses tried

1. **Hypothesis** — a `--help` flag was consumed as a positional project path by
   the CLI. **Test** — ran `codescout --help`, `codescout index --help`,
   `codescout symbols --help`, `codescout start --project --help` in a scratch
   dir. **Verdict** — **rejected** for those four forms; none created the
   directory.
2. **Hypothesis** — a wrapper (the `mcpo` bridge, a shell script, an editor
   integration) passed `--help` through to a path argument. **Test** — none run.
   **Verdict** — deferred; this is the surviving candidate.

## Fix

Two separable pieces:

1. **Remove the artifact.** `rm -rf -- './--help'` — it is untracked scratch,
   nothing references it.
2. **Decide whether there is a defect to fix at all.** That depends on
   hypothesis 2. If a wrapper passed it, the fix belongs there, not in codescout.
   If some codescout subcommand not yet tried does resolve a leading-dash
   argument as a path, that subcommand should reject arguments beginning with
   `--` as project roots.

Do not do 1 before 2 — the directory's mtime is currently the only evidence tying
it to a time window.

## Tests added

N/A — no mechanism identified yet, so there is nothing to regression-test. If
hypothesis 2 resolves to a codescout path, the test is: a project path beginning
with `--` is refused.

## Workarounds

Do not `git add -A` in this repo while it is present. Stage explicit paths.

## Resume

Search the two sessions' transcripts for commands issued around 13:30 on
2026-08-16 containing `--help` — `/home/marius/.claude-kat/projects/-home-marius-work-claude-codescout/*.jsonl`
is the store for this profile. That names the command or rules codescout out.
Only then remove the directory.

## References

- `docs/issues/archive/2026-08-16-librarian-runtime-guide-claims-move-preserves-id.md` — unrelated, but same session's find
