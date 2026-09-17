---
id: '6ec9c313893fd21f'
kind: bug
status: open
title: 'BUG: the machine-default dotenv outranks every per-project override'
tags:
- cluster/unclassified
- embeddings
- config
- precedence
---

## Summary

`~/.config/codescout/.env` is the machine-wide **default** layer in practice —
it is where this host's embedder URL, model name and dim live. But it is
implemented as process environment, and process environment is the **highest**
precedence layer in `merge_embed_config`. So the layer whose role is "default"
behaves as "override", and silently defeats `.codescout/project.toml`, the layer
that exists specifically to let a project differ from the machine.

## Symptom (Effect)

A project's `.codescout/project.toml` sets an embedder `url`. codescout indexes
successfully against a **completely different** endpoint, with no warning that
the configured value was overridden:

```
INFO codescout::retrieval::sync: retrieval sync starting ... backend="qdrant"
INFO codescout::retrieval::sync: retrieval sync finished added=1 ... elapsed_ms=752
added=1 updated=0 deleted=0 elapsed_ms=752
```

Exit 0, success reported, and zero requests reach the URL the project asked for.
The failure mode is **silent success against the wrong backend** — worse than an
error, because the index is then populated with vectors from a model the project
did not choose, and nothing downstream can tell.

## Reproduction

Tree `35b622f8f53cb61fa301733403f3a325fcf17199`, release binary 2026-09-17.
Requires a `~/.config/codescout/.env` that sets `CODESCOUT_EMBEDDER_URL` — on
this host it is a symlink to the repo's own `.env.amd`.

```bash
mkdir -p $SP/proj/.codescout $SP/proj/src
printf 'pub fn f() -> u32 { 42 }\n' > $SP/proj/src/lib.rs
cat > $SP/proj/.codescout/project.toml <<'TOML'
[project]
name = "probeproj"
[embeddings]
model = "DISTINCTIVE-MODEL-FROM-PROJECT-TOML"
url = "http://127.0.0.1:48099/v1"     # a server you control and can log
TOML
codescout index --project $SP/proj        # succeeds; your server sees nothing
```

Setting `CODESCOUT_ENV_FILE` to an empty file is the discriminator: the same
command then reaches the project's URL (and hits a different, separately-filed
defect).

## Environment

Linux 7.2.4-zen2-1-zen, `experiments`. `ls -l ~/.config/codescout/.env` →
symlink to `/home/marius/work/claude/codescout/.env.amd`, which sets
`CODESCOUT_EMBEDDER_URL=http://127.0.0.1:48081`,
`CODESCOUT_EMBEDDER_MODEL_NAME=CodeRankEmbed-Q4_K_M.gguf`,
`CODESCOUT_MODEL_DIM=768`.

## Root cause

Two mechanisms compose, and each is individually defensible.

`load_startup_env` (`src/config/global.rs:132-161`) reads
`$CODESCOUT_ENV_FILE`, else `~/.config/codescout/.env`, and assigns each pair
into the process environment — correctly deferring to an already-set variable
(`startup_env_assignments`, `:113-118`). After this runs, a value that came from
a *file on disk* is indistinguishable from one a user exported deliberately.

`merge_embed_config` (`src/retrieval/config.rs:304-327`) then resolves
`non_empty(env.url).or(proj_url)` — env first, project second. The doc comment on
`RetrievalConfig::from_env_and_project` (`:177-178`) states the intent plainly:
*"`[embeddings]` in the project's config is the base; `CODESCOUT_*` env vars
override it. Benchmark matrix cells set env, so they are unaffected."* That is
the right rule for a variable a human exported for one run. It is the wrong rule
for a file that is read on every start.

Net effect: the ladder the user expects —
`default < global < project` — is actually `default < global-config.toml <
project.toml < global-.env`, with the *global* layer appearing at both the
bottom and the top depending on which file it was written in.

**Measured 2026-09-17**: two runs of the repro, identical except for
`CODESCOUT_ENV_FILE` pointing at an empty file. Without it, the project's URL
received nothing and the command exited 0. With it, the project's URL was
reached.

## Evidence

### The global default layer is a repo file, by symlink

```
lrwxrwxrwx ~/.config/codescout/.env -> /home/marius/work/claude/codescout/.env.amd
```

`.env.amd` documents its own role in a comment: *"Source before running
codescout"*, and *"`~/.config/codescout/.env` is a symlink to it (verified
2026-08-07), so it is what the startup dotenv actually reads."* Its content is
machine defaults — an AMD/ROCm profile — not per-invocation overrides.

### The precedence is inverted only for the dotenv half

A variable the operator exports in their shell **should** win; that is the
escape hatch. The defect is that the dotenv is indistinguishable from it by the
time `merge_embed_config` runs, because `load_startup_env` writes into the same
namespace.

## Hypotheses tried

1. **Hypothesis:** the project's `url` was malformed and rejected.
   **Test:** re-ran with `CODESCOUT_ENV_FILE` set to an empty file.
   **Verdict:** rejected — the identical `url` was then used and reached the
   server. The value was never the problem; its precedence was.
2. **Hypothesis:** this is intended — env is documented as the override.
   **Test:** read `RetrievalConfig::from_env_and_project`'s doc comment.
   **Verdict:** partially confirmed and insufficient. Env-as-override is
   intended and correct for benchmark cells. What is not intended is that a
   *file* read at startup inherits that precedence, which makes the machine
   default outrank every project. Operator ruling 2026-09-17: keep env as the
   top layer, but move machine **defaults** out of the dotenv into
   `~/.config/codescout/config.toml`, and warn when an env value shadows a value
   a config layer set.

## Fix

Plan, not yet implemented, per the 2026-09-17 ruling:

1. Machine defaults move to `~/.config/codescout/config.toml` (requires the
   global section to stop dropping `url`/`api_key` — separately filed). The
   dotenv keeps only secrets and genuine per-machine overrides.
2. `merge_embed_config` learns the **provenance** of each env value (exported vs
   injected by `load_startup_env`) so it can warn — `startup_env_assignments`
   already returns exactly the list of keys it assigned, so the provenance is
   available and currently discarded.
3. When an env value shadows a value set in either TOML layer, emit a one-time
   warning naming the field and both sources.

SHA / patch-id: pending.

## Tests added

None yet. Owed: a test that a project-level `url` survives when the *dotenv*
supplies one, and is overridden when the *process* supplies one — the two must
be distinguishable, which is the whole claim.

## Workarounds

Export the desired value in the MCP server's own env block, or set
`CODESCOUT_ENV_FILE` per project. Neither is discoverable.

## Resume

Thread `startup_env_assignments`'s returned key list into a process-global
`OnceLock<Vec<String>>` so `merge_embed_config` can distinguish dotenv-injected
keys from exported ones, then add the shadowing warning. Do **not** change the
precedence itself — the ruling keeps env on top.

## References

- `src/config/global.rs:113-118`, `:132-161`
- `src/retrieval/config.rs:177-178`, `:304-327`
- `.env.amd` (symlink target of `~/.config/codescout/.env`)
- Sibling bugs: the url-path model discard; the one-field global section.
