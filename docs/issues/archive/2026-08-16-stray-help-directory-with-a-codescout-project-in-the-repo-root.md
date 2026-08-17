---
kind: bug
status: fixed
tags:
- cli
- arg-parsing
- junk-artifact
- unreproduced
closed: 2026-08-16
opened: 2026-08-16
owner: marius
related: []
severity: low
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

**Identified 2026-08-16.** `src/bin/sync_project.rs` — a **separate binary** from the main
`codescout` CLI — took its first positional argument as the project root with no validation
whatsoever:

```rust
let project_path = args
    .next()
    .expect("Usage: sync-project <project-path> [project-id]");
let root = PathBuf::from(&project_path);
```

So `sync-project --help` set `root = "--help"`, and `main` then called `sync_project` with
`record_index_state: true` — which is exactly what writes `.codescout/index-state.json`. That
accounts for every observed detail: a real directory, containing an initialised project, made
by something that got far enough to index.

The same line panicked on no arguments (`expect` on `None`), and the next line
`root.file_name().unwrap()` panicked on `.`, `..` or `/` — so `sync-project .`, the most
natural invocation there is, could never work.
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

1. **Hypothesis** — a `--help` flag was consumed as a positional project path by the CLI.
   **Test** — ran `codescout --help`, `codescout index --help`, `codescout symbols --help`,
   `codescout start --project --help` in a scratch dir.
   **Verdict** — recorded as **rejected**. It was in fact **correct**, and the probe was
   scoped wrong: all four invocations drive the *main* binary. The defect lives in
   `src/bin/sync_project.rs`, a different binary that was never constructed.

2. **Hypothesis** — a wrapper passed `--help` through to a path argument.
   **Verdict** — unnecessary; hypothesis 1 was right all along.

**The lesson is the scoping, not the arg parsing.** Four clean results read as "the
hypothesis is refuted" when they only meant "not through these four doors". That is R-86
again — *name every entry point the behaviour has, then ask which one the probe actually
ran* — and it is the same shape as `cross-cutting-side-effects-at-the-chokepoint`, where
`sync_project` had **five** entry points and a feature shipped dead in its primary one. This
repo has now been bitten twice by under-counting that exact function's callers.
## Fix

Fixed on `experiments`, in `src/bin/sync_project.rs`.

Argument parsing is extracted into `parse_args`, which returns `Parsed::Usage` for
`--help`/`-h` and **refuses any other leading dash** with a message that names the argument
and says why. `main` exits 2 on a refusal instead of indexing a flag.

Two adjacent panics fixed in the same pass, both reachable by ordinary use:

- no arguments → was `expect(...)`, now prints usage and exits 2;
- `derive_project_id` replaces `root.file_name().unwrap()`, which panicked on `.`, `..` and
  `/`. It falls back to the canonicalised directory name, then to the path itself.

The stray directory was already gone by the time this was fixed — removed by another session
— so the mtime evidence this file's Resume depended on was lost. It did not matter: the
transcript search named the mechanism directly.
## Tests added

Four in `src/bin/sync_project.rs`:

- `a_flag_is_never_accepted_as_a_project_path` — `--help`/`-h` route to usage; `--force`,
  `--project`, `-x` and `--` are all refused, and the refusal must name the argument.
- `a_real_path_still_parses_with_an_optional_id`
- `no_arguments_reports_usage_instead_of_panicking`
- `deriving_an_id_from_a_dotted_path_does_not_panic` — `.`, `..`, `/`

**Mutation-verified.** Disabling the dash guard killed exactly the first test, and its
failure message printed the bug verbatim:

```
a leading dash must be refused, not indexed: Run { root: "--force", project_id: None }
```

**Then verified end to end against the built binary**, which is what this file could never do
before — the whole bug was that it had no reproduction:

```
sync_project --help    -> Usage, exit 0
sync_project --force   -> refuses, names the argument, exit 2
sync_project           -> Usage, exit 2      (was a panic)
contents after:  .  ..                        (nothing created)
```

Gate: **3979 tests**, `cargo clippy --all-targets -- -D warnings`, `cargo fmt`.
## Workarounds

Do not `git add -A` in this repo while it is present. Stage explicit paths.

## Resume

None. The mechanism is identified, fixed, unit-tested, mutation-verified and reproduced
end-to-end against the built binary.

The transcript search this section originally prescribed is what found it — worth keeping as
a method. Grepping the three profiles' JSONL stores for `codescout[^"]{0,60}--help` surfaced
a prior session's own conclusion (*"`--help` wasn't a flag — `sync_project` … created a
project"*) in one call. When two sessions share a working tree, one of them may already have
the answer written down.
## References

- `docs/issues/archive/2026-08-16-librarian-runtime-guide-claims-move-preserves-id.md` — unrelated, but same session's find
