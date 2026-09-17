---
id: 26d098a04a1613d6
kind: bug
status: fixed
title: 'BUG: the machine-default dotenv outranks every per-project override'
tags:
- cluster/unclassified
- embeddings
- config
- precedence
claimed_at: 2026-09-17
claimed_by: 458a8a26-c380-4f5b-b967-2181f592917e
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

Landed in two halves, both already in the tree.

**1. The global layer can now hold what the dotenv was holding.** Until 2026-09-17 the
global `[embeddings]` was a one-field struct, so `url` and `api_key` could only be
expressed as env — the dotenv was not a bad habit, it was the *only* place those values
fit. Fixed by the shared settings type (`de0a1e07`). Machine defaults now belong in
`~/.config/codescout/config.toml`, which sits BENEATH the project layer where a default
belongs.

**2. Provenance, so the remaining overrides can be told apart.** `startup_env_assignments`
already computed exactly the keys it injected and discarded the list; it is now recorded
in `DOTENV_INJECTED` (`src/config/global.rs`) and read once at the edge by
`EmbedEnv::from_real_env`, which carries it as per-field `DotenvProvenance` data.
`dotenv_shadowed_fields` (pure) names the fields where a **dotenv-injected** value beat a
configured one, and `resolve_embed_fields_from` warns.

**The precedence itself is unchanged, deliberately.** Env still wins. That was the
2026-09-17 ruling and it is right: benchmark cells, `scripts/sweep-*.sh`, CI and
docker-compose wiring all depend on it, and an operator export is the sanctioned escape
hatch. What was wrong was never that env wins — it is that a *file read on every start*
is indistinguishable from an export by the time anything can act on it, so a machine
DEFAULT silently acquired OVERRIDE precedence over the layer that exists to differ from
the machine.

Three conditions gate the warning, and dropping any one makes it noise rather than
signal: the value came from the dotenv (an export is silent); the env value is present
and non-blank (a blank never wins anyway); and the config layers actually set that field
(nothing shadowed → nothing to say). The third is why this stays quiet on a machine
configured entirely through `.env` with projects that set no `[embeddings]` — which is
this repo's own setup, and the population that would otherwise see the warning on every
resolution and learn to ignore it.

The remedy the message names is performable by the person reading it — move the value to
`config.toml`, or unset it — which is the check `CLAUDE.md` § *Testing Discipline* asks
for when shipping a guard: name the next action its message produces, and ask whether
that party can perform it.

**Not done, and carried rather than dropped:** the `CODESCOUT_EMBEDDING_*` family
consolidation. Eleven env vars still name a model, url or key across three independent
consumers; this bug is about *precedence*, not about the count, and the consolidation is
the other half of Task 5 in
`docs/plans/2026-09-17-embedding-config-consolidation.md`.

- **SHA:** `91dede5a9ddfa72f61c731b506f0a2077b2c2732` (on `experiments`) — the provenance
  half. The global-layer half is `de0a1e0703a79a458cd0293a225ce7cb970d8cb7`.
- **patch-id:** `d33ceab59d56977d00e4bbe4f91407790d178497`

## Tests added

Five in `src/retrieval/config.rs` `merge_tests`, all against the pure
`dotenv_shadowed_fields`:

- `a_dotenv_value_that_overrides_a_configured_one_is_named` — the defect.
- `an_exported_value_that_overrides_a_configured_one_is_silent` — byte-identical inputs
  except `from_dotenv`, which is the entire claim.
- `a_dotenv_value_with_no_configured_counterpart_is_silent` — the ordinary case on a
  `.env`-configured machine.
- `provenance_is_per_field_not_per_struct` — one export among two dotenv values.
- `a_blank_dotenv_value_shadows_nothing`.

**Written alongside the implementation, so no observed red.** Settled by mutation
instead, once per condition in the predicate rather than once for the feature:

| mutation | killed |
|---|---|
| ignore `from_dotenv` (warn on every env override) | `an_exported_…_is_silent`, `provenance_is_per_field…` — and **only** those two |
| ignore whether the config set the field | `a_dotenv_value_with_no_configured_counterpart_is_silent` — and only that one |

Each condition is caught by exactly the assertions written for it, with the others
staying green: the tests discriminate individually rather than as a block.

**End-to-end on the real binary**, which is what makes the warning a reached path rather
than a decoration:

```
dotenv supplies the url, project.toml sets its own
  -> WARN [embeddings].url is set in your config but was overridden by the startup dotenv …

same value EXPORTED instead
  -> warnings emitted: 0
```

## Workarounds

Export the desired value in the MCP server's own env block, or set
`CODESCOUT_ENV_FILE` per project. Neither is discoverable.

## Resume

N/A — fixed and verified.

The `CODESCOUT_EMBEDDING_*` family consolidation is carried on Task 5 of
`docs/plans/2026-09-17-embedding-config-consolidation.md`, not on this bug. It is a
different complaint about the same surface: this one was *precedence*, that one is the
eleven-variable *count*.

## References

- `src/config/global.rs:113-118`, `:132-161`
- `src/retrieval/config.rs:177-178`, `:304-327`
- `.env.amd` (symlink target of `~/.config/codescout/.env`)
- Sibling bugs: the url-path model discard; the one-field global section.
