---
id: ae511945b43ba0c8
kind: bug
status: fixed
title: 'BUG: setting up codescout on a new project does not set up the librarian — the user must find and apply each knob by hand'
owners:
- marius
tags:
- cluster/unclassified
closed: 2026-10-14
opened: 2026-10-14
related: []
severity: high
unverified: 'Nine review minors (Residual, #4-#12) are open and not fixed. The AlreadyExists race branch of create_default has no unit test. server-stack and macOS/Windows paths were not run. The follow-up brainstorm (a librarian that starts but cannot search says nothing) is not started.'
---

# BUG: setting up codescout on a new project does not set up the librarian — the user must find and apply each knob by hand

## Summary

Setting up codescout on a new project is harder than it should be. In particular, nothing configures the librarian for that project. The user must read several docs and edit several files in several places. The request: make setup easy, and configure the librarian automatically on a new project.

This is a usability gap, not a crash. It was opened at the user's explicit request (feature-shaped, so no cluster class fits: `cluster/unclassified`).

## Symptom (Effect)

A new project gets the code-intelligence side of codescout from the documented first-run flow (`onboarding`, then `index`). It gets no librarian setup from that flow. The librarian side (artifact catalog, trackers, bug files, embedder, scope) needs separate manual work.

The first-project walkthrough has no librarian step. `docs/manual/src/getting-started/first-project.md` lists: start Claude Code, onboarding, `index(action: build)`, then the basic tools. The word "librarian" does not appear in it (read in full, 2026-10-14).


Measured 2026-10-14, clean git repository, no `~/.config/librarian/workspace.toml`, exit code 1:

```
Error: opening librarian tool context

Caused by:
    0: Load workspace from <XDG_CONFIG_HOME>/librarian/workspace.toml. Run `librarian-mcp import-codescout` to seed.
    1: reading <XDG_CONFIG_HOME>/librarian/workspace.toml
    2: No such file or directory (os error 2)
```

The suggested fix does not exist. No `librarian-mcp` binary is on `PATH` or in `target/release/`. The design docs call `import-codescout` obsolete (`docs/superpowers/specs/2026-05-08-librarian-project-model-redesign.md:60`). The hint text is still compiled in at `src/librarian/mod.rs:178`.


Measured 2026-10-14 on the MCP path (stdio JSON-RPC client, same clean repo and isolated XDG directories, `codescout start`):

```
TOOLS: ['read_file', 'tree', 'grep', 'create_file', 'edit_file', 'run_command', 'onboarding',
        'approve_write', 'symbols', 'edit_code', 'memory', 'semantic_search', 'index',
        'workspace', 'library', 'get_guide']
--- doc {"action": "find"}
{"error": {"code": -32602, "message": "unknown tool: 'doc'"}}
```

The server log holds the only trace, at INFO level:

```
INFO codescout::librarian::adapter: librarian disabled: Load workspace from <XDG_CONFIG_HOME>/librarian/workspace.toml. Run `librarian-mcp import-codescout` to seed.: ... No such file or directory (os error 2)
```

So on a clean machine the agent sees no `doc` tool and no `librarian` tool. Nothing in the tool list, the instructions or the tool error says why. The manual says the librarian is "enabled by default". The same call also fails after `onboarding` returned successfully (call 6 of the probe).

## Reproduction

Reproduced through the CLI on 2026-10-14, at `experiments` with `target/release/codescout`. The MCP path was run too: a stdio JSON-RPC client (`initialize`, `tools/list`, then `workspace` activate, `doc find`, `onboarding`, `doc find`) against `codescout start` with the same isolated environment. The `tools/list` call was made once, right after `initialize`; it was not repeated after `onboarding`.

```bash
S=<scratch dir>
mkdir -p $S/clean $S/xdg/{data,config,home}
cd $S/clean && git init -q && printf '# demo\n' > README.md && mkdir docs && printf '# Plan\n' > docs/plan.md
git add -A && git -c user.email=a@b -c user.name=t commit -qm init
export XDG_DATA_HOME=$S/xdg/data XDG_CONFIG_HOME=$S/xdg/config HOME=$S/xdg/home
<repo>/target/release/codescout doc find      # exit=1
```

The three XDG variables isolate the run from the real catalog and the real `~/.config/librarian/`. Still not run: a clean machine where the user already installed the plugin (`codescout-companion`), and `onboarding` completed through its prompt phases.

## Environment

Branch `experiments`. Read from source and docs only. Nothing here was run on a clean project.

## Root cause

One mechanism is measured on both paths (mechanism 1 below). Two are read from the source only (mechanisms 2 and 3). Separately, the librarian configuration is spread over seven places, and none of them is written by the first-run flow:

| Surface | What it controls | Where (verified 2026-10-14) |
|---|---|---|
| `LIBRARIAN_ENABLED` env | Disable or force-enable the tools for the process | `docs/manual/src/concepts/librarian-embedded.md`, "Runtime override" |
| `[librarian] enabled` in `<project>/.codescout/project.toml` | Per-project opt-out; the default is enabled | `src/server.rs:1671-1672` (doc comment), same manual page |
| `[librarian] chunk_grain`, `vector_backend` in `project.toml` | Chunk grain; Qdrant or sqlite-vec backend | `src/librarian/catalog/chunk.rs:139,190`; `src/librarian/artifact_store.rs:12,39` |
| `<project>/.codescout/librarian.toml` | Per-project classifier rules (`[[rule]]` blocks) | `librarian-embedded.md`, "Per-project classifier overrides" |
| `~/.config/librarian/workspace.toml` | Global roots, ignore list, rules, umbrellas | `src/librarian/workspace.rs:9-18` (`WorkspaceConfig`), `default_config_path` |
| `<project>/.codescout/workspace.toml` `[[umbrella]]` | Project-local umbrellas; wins over the global file | `src/librarian/current_project.rs:81-111` |
| `LIBRARIAN_EMBED_*` env, usually from a `.env.<profile>` file | The artifact embedder, separate from `CODESCOUT_EMBEDDER_*` | `librarian-embedded.md`, "Migration" section |

The librarian is on by default according to `librarian_enabled_at_runtime` (`src/server.rs:1674-1701`) and the manual. That function only checks the opt-out switches. A later step can still drop the tools, as mechanism 1 shows, so "enabled by default" is not true on a clean machine. The gap is that nothing guides or writes the rest.

Not verified: where the onboarding prompt handles artifact tracking. A comment at `src/tools/onboarding.rs:36-45` says the onboarding slice has an "artifact-tracking step". A grep of `src/prompts/` for that wording found only the two librarian guides (`src/prompts/guides/librarian.md`, `librarian-runtime.md`), not an onboarding step. Read the onboarding prompt source before working this bug.

Mechanisms, all in `build_tool_context_with` (`src/librarian/mod.rs:165-335`):

1. **A missing `workspace.toml` is a hard error, and on the MCP path it removes the tools.** `workspace::load(&cfg_path).with_context(...)` at `src/librarian/mod.rs:174-180` has no default and no creation step. The server then drops the failure: `try_build_runtime_with` returns `None` and `src/server.rs:434-446` skips `adapters_for`, so `doc` and `librarian` are never registered. Measured 2026-10-14: CLI `doc find` → exit 1; MCP `tools/list` → 16 tools, no `doc`, no `librarian`; `doc` call → `unknown tool: 'doc'`; the reason appears only as an INFO line in the server log. Nothing in the first-run flow writes the file. The message names a command that no longer exists.
2. **A missing embedder is silent.** When `env.embed_model` is `None`, `embedding` is `None` with no log line (the `else` branch of the embedding block). The comment says "When absent (CI, tests, first-run) we skip embedding silently". Inferred from the source — not measured.
3. **An unreachable Qdrant is a `warn` only.** The artifact store becomes `None` and artifact semantic search is disabled. The message does name the `sqlite-vec` escape hatch. Inferred from the source — not measured.

Mechanisms 2 and 3 mean a new project can reach a librarian that starts but cannot search, with no failure the user must act on.

## Evidence

- `docs/manual/src/getting-started/first-project.md` — no librarian step (read in full 2026-10-14).
- `docs/manual/src/concepts/librarian-embedded.md` — the knobs are documented in a concept page, not in the getting-started flow. The same page still describes the librarian as "opt-in" in its first paragraph and "enabled by default" later, so it contradicts itself (IC-11 candidate; check before fixing).
- This repository's own librarian layout (`docs/issues/_TEMPLATE.md`, `docs/trackers/`, `docs/TAXONOMY.md`, cluster files) was built by hand over time. A new project has no equivalent starting point.


### Classification under an empty `workspace.toml` (measured 2026-10-14)

Setup: clean git repo, isolated XDG directories, MCP `codescout start`, empty `<XDG_CONFIG_HOME>/librarian/workspace.toml`, 14 markdown files committed, then `librarian(action="reindex", scope="project")`.

```
REINDEX: added 14, embedded 14, vectorless 0, unknown_count 12
tracker  active  docs/issues/x.md        <- built-in rule matched (control)
doc      draft   CHANGELOG.md            <- built-in rule matched (control)
unknown  unknown docs/superpowers/specs/a.md, docs/superpowers/plans/b.md,
                 docs/research/2026-01-01-c.md, docs/audits/d.md, docs/handoffs/e.md,
                 docs/runbooks/f.md, docs/adrs/g.md, docs/manual/h.md, ROADMAP.md,
                 README.md, docs/plan.md, docs/misc/notes.md
```

So `classify` returns `None` for an unmatched path (`src/librarian/classify.rs:167-179`) and the artifact is stored with `kind: unknown`, `status: unknown`. The artifacts are still embedded and searchable (`vectorless: 0`). What is lost is the `kind` filter: `doc(action="find", kind="spec")` finds nothing.

Then the same repo with the nine rules of `import_codescout` written into the file, reindex WITHOUT `force`:

```
REINDEX: updated 9, unchanged 5, embedded 0
spec docs/superpowers/specs/a.md | plan .../plans/b.md | memory docs/research/..-c.md |
audit | handoff | runbook | adr | doc docs/manual/h.md | roadmap ROADMAP.md
unknown: README.md, docs/plan.md, docs/misc/notes.md   (no rule covers them)
```

Two consequences. All nine rules classify correctly. A rule change reaches already-indexed files on a plain reindex; no `force` is needed.

The maintainer's own `~/.config/librarian/workspace.toml` holds 18 `[[rule]]` blocks (grep of `^glob =`, 2026-10-14). They cover the nine plus `docs/plans`, `docs/content`, `README.md`, `CLAUDE.md`, `AGENTS.md`, `docs/adr`, `docs/trackers`, and two tracker name patterns. That file is the de facto default today, grown by hand.


### After the fix (measured 2026-10-14)

Setup as above: clean git repo, `XDG_CONFIG_HOME`/`XDG_DATA_HOME`/`HOME` pointed at a scratch dir with no `librarian/` directory, a release binary built from `3454e094` in a leased slot.

```
CLI  codescout doc find        -> exit 0; <XDG_CONFIG_HOME>/librarian/workspace.toml created, 645 bytes, comments only
MCP  tools/list                -> doc and librarian both listed
MCP  reindex + doc find        -> 11 of 14 files classified; unknown: README.md, docs/plan.md, docs/misc/notes.md (no rule covers them)
                                  (before: 12 of 14 unknown; spec, plan, memory, audit, handoff, runbook, adr, doc, roadmap, tracker all correct)
```

The Opus review ran the same binary a second time and found the file byte-identical (same sha256): it is created once and never rewritten.

Gate on the final tree (`./scripts/gate.sh --check`, `50b2ac12`): `PYTHON_LIGHT=0 FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0`, no failing test. The gate prints what green does not cover: the three heavy Python suites, `server-stack` (no lane compiles it), librarian code under LEAN, and the two ONNX weight tests.

Mutation probe (`scripts/mutation-probe.sh`, isolated worktree):

```
should_init_default_workspace -> true            KILLED  init_is_requested_only_for_the_default_path
create failure returns Err instead of default    KILLED  an_unwritable_location_falls_back_to_the_in_memory_default
from_env() asks for false                        KILLED  from_env_requests_the_default_file
load_or_init(&cfg_path, false) in the wiring     SURVIVED all 2151 librarian tests -> fixed in f61ac4b6
same mutant after the fix                        KILLED  the_default_path_is_created_only_when_the_env_asks
```

Opus whole-branch review of `86dba52d..f61ac4b6`: no Critical, two Important (both fixed in `50b2ac12`, each reproduced before the fix), nine Minor (Residual).

## Hypotheses tried

1. **Hypothesis:** The defaults are already right, so a clean project works with no setup. **Test:** `codescout doc find` on a clean repo with isolated XDG directories (2026-10-14). **Verdict:** rejected for the CLI path. It exits 1 on the missing `workspace.toml`. **Evidence:** "Symptom (Effect)".
2. **Hypothesis:** The MCP server path behaves like the CLI path. **Test:** stdio JSON-RPC probe on the clean repo (2026-10-14). **Verdict:** confirmed, and worse: the tools vanish from `tools/list` instead of failing loudly. **Evidence:** "Symptom (Effect)".
3. **Hypothesis:** The missing piece is only documentation. **Test:** compare the error with the docs. **Verdict:** rejected as the whole story. The error itself misleads (it names a removed command), so code must change too.

## Fix

Plan only. Nothing is implemented. Options to evaluate, cheapest first:

0. **Smallest first fix:** when `workspace.toml` is absent, create it instead of failing. Replace the `import-codescout` hint at `src/librarian/mod.rs:178` with a command that exists. If the librarian stays disabled, say so where the agent looks (the server `instructions`, or a `doc`/`librarian` stub that returns the reason), not only in an INFO log line.
    - Creation must be an explicit field on `LibrarianEnv`, true only from `from_env()`. Six tests in `src/server.rs` leave `workspace` unset and would otherwise write into the real home directory (W-151 in `docs/trackers/bug-fix-session-log.md`).
    - Content: an empty file loads, but classifies no spec, plan, ADR, audit, handoff, runbook, ROADMAP or manual page (measured in Evidence; F-195). Decision needed: write the nine rules into the created file, or move them into `DEFAULT_RULES_TOML` (`src/librarian/classify.rs:66-130`) so every project gets them with no file.
    - Two files are called `workspace.toml`: the global `~/.config/librarian/workspace.toml` and the project-local `.codescout/workspace.toml` that `doctor` reads (`src/librarian/tools/doctor.rs:2801`). The created file's header comment and the error text must name which one.
1. Add a librarian step to `first-project.md` and to the onboarding output. It lists the knobs above with the defaults, and says which ones a new project must set.
2. Make `onboarding` detect and write the librarian settings. It writes `[librarian]` defaults to `project.toml`, checks the embedder, and runs the first `librarian(action="reindex")`.
3. Add an optional scaffold. It creates `docs/issues/` with a bug template, one session-log tracker, and the `.codescout/librarian.toml` classifier rules. Keep it opt-in, because this repository's tracker taxonomy is project-specific.
4. Add a `librarian(action="doctor")` check for "configured for this project?" so a half-set-up project reports what is missing.

Constraints to respect when working this:

- Adding or changing a tool: read `docs/PROGRESSIVE_DISCOVERABILITY.md` first.
- A change to onboarding or any prompt surface: read `src/prompts/README.md`, and bump `ONBOARDING_VERSION` if the rendered prompt changes.
- Any "nothing is missing" result from the new check must name its scope (`docs/adrs/2026-08-27-negative-results-name-their-scope.md`).


### Implemented

All on `experiments`. Each commit is recorded with its patch-id (`git show <sha> | git patch-id --stable`), which survives a rebase; the SHA does not.

| Commit (experiments) | patch-id | What |
|---|---|---|
| `cb1b9bcf` | `1560c51e026ebe79383fdc489577165d13b56ff6` | Ten built-in classifier rules in `DEFAULT_RULES_TOML` (`src/librarian/classify.rs`); frontmatter-precedence test |
| `3454e094` | `1e32b2a99497ad354a4e98561b4a4ad6f450178e` | `workspace::load_or_init` and `DEFAULT_WORKSPACE_TOML` (`src/librarian/workspace.rs`); `LibrarianEnv::init_default_workspace`, set only by `from_env()`; new error text (`src/librarian/mod.rs`) |
| `8af07570` | `2f313c9d54cb5b2d84228acd78e6802288cc8cde` | Manual: First run section, all 25 built-in rules, no more "opt-in" |
| `f61ac4b6` | `f040ef4b560aede6e63b80c0343fce0067accd83` | `load_workspace_config(env, default_path)` extracted, so the wiring is testable |
| `50b2ac12` | `59f986883a2a2f87cd5509ea7ca771e6dd98c24e` | Review fixes: `confirmed_absent` (no replacing an unreadable user file); spawned-binary test no longer writes the real config dir |

Decisions taken with the user: create the file only at the default path; the nine workflow rules plus `docs/trackers` live in `DEFAULT_RULES_TOML`, not in the created file; the created file is comments only. Not done, by agreement: the "librarian starts but cannot search" visibility problem (separate brainstorm).

## Tests added

- `classify::tests::default_rules_classify_the_workflow_directories` (ten paths, kind/status/time_scope, plus an unmatched control)
- `indexer::tests::a_frontmatter_kind_beats_a_built_in_rule` (mutant: rule before frontmatter -> fails)
- `workspace::tests`: `init_creates_the_file_when_absent`, `the_default_text_parses_to_the_default_config`, `init_never_overwrites_an_existing_file`, `without_init_a_missing_file_is_an_error_and_stays_missing`, `an_unwritable_location_falls_back_to_the_in_memory_default`, `a_malformed_existing_file_is_still_an_error`, `an_unreadable_existing_file_is_an_error_not_an_empty_default`
- `librarian::librarian_env_tests`: `init_is_requested_only_for_the_default_path`, `from_env_requests_the_default_file`
- `librarian::tests`: `a_missing_explicit_workspace_is_an_error_that_names_a_real_remedy`, `the_default_path_is_created_only_when_the_env_asks`
- `tests/cross_process_write_lock.rs`: the spawned child now gets a scratch `LIBRARIAN_WORKSPACE` and `LIBRARIAN_DB` (verified: config dir empty after the run)

Each new decision was mutated in an isolated worktree; see Evidence, "After the fix".


## Fix provenance

Primary fix (the commit that makes the librarian register on a clean machine). The other four commits of the series are in Fix, "Implemented", each with its own patch-id.

- **SHA:** `3454e094` (`experiments`)
- **patch-id:** `1e32b2a99497ad354a4e98561b4a4ad6f450178e`

## Workarounds

None needed after the fix. Before it: create an empty `~/.config/librarian/workspace.toml` by hand (an empty file is valid).

## Resume

Fixed and verified on `experiments` (see Fix, "Implemented"). Open follow-ups are under Residual.

## Residual

Deferred review minors (Opus review of `86dba52d..f61ac4b6`), none blocking:

- **#4** `create_default`'s `AlreadyExists` branch can be unit-tested (call it on an existing file: `Ok`, bytes unchanged). The plan said it could not; that was wrong, and the mutant `AlreadyExists => Err(e)` survives.
- **#5** The error hint says "create an empty file there", which is wrong advice when the existing file is malformed. Tailor it to the cause.
- **#6** `a_missing_explicit_workspace_is_an_error_that_names_a_real_remedy` never asserts the remedy text; add `contains("LIBRARIAN_WORKSPACE")`.
- **#7** `a_frontmatter_kind_beats_a_built_in_rule` could pass if the spec rule were absent; anchor it with a `classify` assertion first (IC-9 shape).
- **#8** The manual and the file header say `~/.config/librarian/workspace.toml`; `dirs::config_dir()` differs on macOS and Windows.
- **#9** `dirs::config_dir()` returning `None` is still fatal and unexplained (`default_path()?` sits outside `with_context`).
- **#10** Comment on `DEFAULT_WORKSPACE_TOML` that it must stay comments-only (a race loser may read a prefix).
- **#11** `docs/trackers/**/*.md` stamps `status: active` on archive files that lack a status (same precedent as `docs/issues/**`).
- **#12** "## First run" follows a paragraph without a blank line (renders fine).

Out of scope by agreement, needs its own brainstorm: a librarian that starts but cannot search (no embedder, Qdrant down) reports nothing to the agent beyond an INFO log line (`src/librarian/adapter.rs:34`).

## References

- `docs/manual/src/concepts/librarian-embedded.md`
- `docs/manual/src/getting-started/first-project.md`
- `src/tools/onboarding.rs:36-45`
- `src/librarian/workspace.rs`, `src/librarian/current_project.rs`
