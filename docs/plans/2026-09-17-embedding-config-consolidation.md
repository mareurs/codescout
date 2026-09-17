---
id: d6df53b33661d4d2
kind: plan
status: draft
title: Embedding configuration consolidation — two places, one resolver
tags:
- embeddings
- config
- onboarding
- retrieval
topic: embedding configuration
---

**Status:** approved in outline 2026-09-17 (three rulings below). **Task 1 landed** —
`654e1f18` / patch-id `2a45699854551182`, bug archived as `aae547c917c329b5`.
**Bugs this closes:** `aae547c917c329b5` (**fixed**), `da26a27026bb9f41`,
`6ec9c313893fd21f`, `efd14d6c5eb56905`, `c09210c5bcd74208`, `eb3417ec7e02c66e`.

## The problem, measured

At tree `35b622f8f53cb61fa301733403f3a325fcf17199` an embedding setting can be
written in **15 places** feeding **3 mutually-independent consumers**. Derived,
not cited: `grep(env::var(...EMBED|OLLAMA|OPENAI|QUERY_PREFIX...))` over `src/` +
`crates/`, production sites only, plus the two TOML layers, the startup dotenv
and the built-in default.

| Consumer | Selected when | Takes the model from |
|---|---|---|
| `create_embedder_with_config` | no url | `RetrievalConfig.model` |
| `EmbedderHttp` | **url set** | `CODESCOUT_EMBEDDER_MODEL_NAME` **only** |
| `LibrarianEnv` | always, for artifacts | `LIBRARIAN_EMBED_*` **only** |

The three never consult each other. Four model-naming env vars, three url vars
and three key vars exist because each consumer grew its own.

**This is a half-finished migration, not carelessness.** The 2026-03-31 design
(`docs/superpowers/specs/2026-03-31-unified-embedding-config-design.md` § 1)
specified *"`url` is set → `RemoteEmbedder` targeting that URL. `model` is the
model name sent in the request body"* — still exactly what
`create_embedder_with_config` does. The v0.12 retrieval-stack work inserted
`EmbedderHttp` in front of that resolver for the url case, to gain the sparse
leg. The branch moved; the model did not follow.

## Decision (ADR)

**Decision:** one `EmbeddingSettings` type deserialised at both TOML levels, one
pure resolver, and env demoted from *a place settings live* to *a warned escape
hatch*.

```
built-in default
  └─ ~/.config/codescout/config.toml  [embeddings]     ← place 1: global default
       └─ <proj>/.codescout/project.toml [embeddings]  ← place 2: per-project
            └─ CODESCOUT_EMBEDDING_*                    ← escape hatch, warns on shadow
```

**Context:** the operator's requirement is two places. Env cannot simply be
removed — `RetrievalConfig::from_env_and_project`'s own doc records that
benchmark matrix cells and `scripts/sweep-*.sh` depend on env winning, and
docker-compose wiring sets it. What *can* change is that the machine's
**defaults** stop being expressed as env.

**Alternatives considered:**
- *project.toml beats env* — strictly matches "two places", rejected: breaks the
  sweep scripts, CI matrix cells and compose wiring in one commit.
- *env only fills gaps* — rejected: silently ignores a deliberately-exported
  variable, which is the same class of failure as the bug being fixed, inverted.
- *one breaking consolidation* — rejected in favour of deprecation windows;
  every `.env.*` in the tree and on operators' machines would have to move
  atomically.

**Consequences:**
- *now easier:* adding an embedding setting touches one struct; asking "what is
  my effective model and where did it come from" has an answer; a project can
  differ from the machine.
- *now harder:* two env names per setting during the deprecation window, and a
  provenance channel must be threaded from `load_startup_env` to the resolver
  that does not exist today.

**Change scenarios absorbed:**
1. *A new embedding setting is added* — today it must be added to
   `EmbeddingsSection` **and** `GlobalEmbeddingsSection`, and the second is
   already four fields behind (bug `da26a27026bb9f41` is that drift realised).
2. *A second HTTP-backed embedder is introduced* — today it would re-read env
   inside its own constructor, which is exactly how bug `aae547c917c329b5`
   arose.
3. *A project needs a different model from the machine* — today impossible when
   the machine's value came from the startup dotenv.

**Revisit-when:** a consumer genuinely needs a different model from the code
index (the librarian may; step 6 keeps that expressible rather than assuming it
away).

**Confidence:** high on steps 1–4 (each closes a runtime-verified defect and is
locally scoped); medium on step 5, where the provenance channel is new code on a
startup path.

## Phase 1 — make the defects impossible (no config-surface change)

Ordered so each lands independently and the tree is shippable between steps.

### Task 1 — the configured model reaches the wire

**Landed 2026-09-17** — `654e1f187c63c79673c771ea25890bf517da6f43`, patch-id
`2a456998545511824432cf69d9351f5af434b2ff`. Closed `aae547c917c329b5`.

What shipped, and the one place it diverged from the plan above:

- `EmbedderHttp::new` gained a `dense_model_name` parameter and **stopped reading**
  `CODESCOUT_EMBEDDER_MODEL_NAME`. The plan said "keep the env read, add a fallback";
  that was wrong, and the test caught it. A fallback leaves the read inside a
  constructor, where exercising it needs `set_var` — so the assertion could not be
  written, which is the property that let the defect survive. The read moved to
  `RetrievalConfig::dense_model_name_override`, resolved by a pure
  `dense_model_name()`. **The plan's own step 1 would have closed the symptom and left
  the cause.**
- Prefix stripping extracted to `codescout_embed::bare_model_name`, shared with
  `create_embedder_with_config`'s url arm.
- Three tests in `selection_tests` via a new `dense_model_name_for_test()`; two observed
  RED before the fix. The third's red came from the override path rather than from
  stripping, so it was settled by mutation instead — `scripts/mutation-probe.sh` on
  `bare_model_name` reports KILLED.
- `tests/embedder_env_isolation.rs`'s rationale updated: its guard stops *tests* taking
  the env-reading path, and this removes the cause it was policing for the model var.

Still owed from this task, deliberately deferred rather than forgotten: the shadowing
warning. `RetrievalConfig` cannot yet distinguish a chosen `model` from a defaulted one,
so a warning keyed on difference alone would fire on every deployment that exists. It
needs the provenance channel and lands with Task 5.

`require_model`'s message was **not** changed. On this path it is now unreachable —
`RetrievalConfig.model` always carries a value — so rewording it would have been a
change no caller can reach; it stays live for `RemoteEmbedder::from_url`'s other
constructors.

### Task 2 — one settings type at both levels

**Landed 2026-09-17** (SHA recorded at commit). Closes `da26a27026bb9f41`.

`GlobalEmbeddingsSection` is now a **type alias** for
`crate::config::project::EmbeddingsSection`, not a second struct kept in step — because
"kept in step" is exactly what failed. Two structs describing one config block drift in
only one direction, silently: the level a developer is editing gains the field and the
other does not, and neither the type system nor the tests object. One type cannot drift
from itself.

`EmbeddingsSection::model` became `Option<String>`, and that was **forced, not
cosmetic**. `serde(default)` fires for an absent `[embeddings]` table, so with the old
`default = "default_embed_model"` the shared struct would have made every global config
serialise `model = "local:AllMiniLML6V2Q"` into the merge base — a global layer
asserting a model it never mentioned, and pinning every project to it. The default moved
to resolution: `EmbeddingsSection::model_or_default()` and `merge_embed_config`.

Verified at runtime, both directions of the thing the task is actually for:

| global sets | project sets | reaches the wire |
|---|---|---|
| model + url + api_key | *(no `[embeddings]`)* | all three — previously `Unknown model`, zero requests |
| model + url + api_key | `model` only | model **from project**, api_key **from global** |

The second row is the deliverable: field-by-field layering inside `[embeddings]`, not
whole-table replacement.

**Both tests were written after the fix, so neither had an observed red.** Settled by
mutation instead, once per guarded field rather than once for the feature — `url` and
`api_key` are separate sites and a single probe would have proved only one. Adding
`skip_serializing` to `url` kills both new tests with `left: None`, the exact historical
symptom; adding it to `api_key` kills only the assertion that names it, leaving the
gap-filling test green. Each assertion catches its own field, and the twelve
pre-existing merge tests stayed green under both, confirming the mutations were isolated
to the global round trip.

One test-design note worth keeping, because the obvious version of this test is vacuous:
both go through `GlobalConfig::load_from_dir` + `to_toml_value`, never a hand-built
`toml::Value`. The defect lived in the **type** — serde discarded the keys at parse time
and `to_toml_value` re-serialised the struct — so a test that constructs the merge base
directly bypasses the broken component entirely and passes before and after the fix,
asserting only about `merge_toml`, which was never broken.

**Deferred out of this task, deliberately: the unknown-key warning.** It is listed below
and is not done. With the shared type, the remaining silent drop is a genuinely unknown
key (a typo like `modle = …`), which is a different residual from the one this bug
names, and the warning needs machinery Task 5 introduces anyway — the provenance channel
and a warn-once surface. Doing half of it here would mean building that surface twice.
Moved to Task 5; `doctor` is the other candidate host.

### Task 3 — the default build runs the default model

Closes `c09210c5bcd74208`. Operator ruling: add `local-embed` to `default`.

- `Cargo.toml`: `default = ["remote-embed", "http", "librarian", "local-embed"]`.
- Guard: a test in the **default** lane calling
  `create_embedder_with_config(default_embed_model(), None, None)` and asserting
  it is not the unsupported-feature bail. Annotate the fixture — this test is
  vacuous under `--features local-embed` and meaningful under plain `default`,
  the opposite polarity from the librarian case in `CLAUDE.md`.
- Verify the Windows / EDR lane stays green:
  `docs/manual/src/configuration/embeddings-edr-windows.md` and
  `docs/issues/archive/2026-08-08-cyberark-epm-blocks-ort-sys-build-script.md`
  are the constraints; `local-embed-dynamic` must remain the escape hatch.

### Task 4 — correct the docs

Closes `efd14d6c5eb56905`. **Sequenced after tasks 1–3**, or the pages get
rewritten twice.

- `configuration/embeddings.md`: the banner currently states the inverse of
  reality in both directions. Rewrite against the then-current ladder.
- `configuration/global-config.md` § Merge semantics: states "no deep-merge"
  where `merge_toml` recurses and `merge_toml_base_fills_missing_key` pins it.
  Replace the non-resolvable `jinaai/jina-embeddings-v2-base-code` example.
- `src/config/project.rs:82`: two conflicting `(default)` markers in one doc
  block — `ollama:` is not the default.
- Document the API-key path somewhere that is not self-disclaiming. Today
  `[embeddings].api_key` is documented on exactly one page, and that page's
  banner says to treat it as historical.

## Phase 2 — consolidate the surface behind deprecations

### Task 5 — one env family, provenance-aware

Closes `6ec9c313893fd21f`.

New family, one name per field: `CODESCOUT_EMBEDDING_{MODEL,URL,API_KEY,DIM,QUERY_PREFIX}`.

Deprecated aliases keep working for ≥1 release, each warning once on first read:

| deprecated | → |
|---|---|
| `CODESCOUT_EMBED_MODEL`, `CODESCOUT_EMBEDDER_MODEL`, `CODESCOUT_EMBEDDER_MODEL_NAME` | `CODESCOUT_EMBEDDING_MODEL` |
| `CODESCOUT_EMBED_URL`, `CODESCOUT_EMBEDDER_URL` | `CODESCOUT_EMBEDDING_URL` |
| `EMBED_API_KEY` | `CODESCOUT_EMBEDDING_API_KEY` |
| `CODESCOUT_MODEL_DIM` | `CODESCOUT_EMBEDDING_DIM` |
| `CODESCOUT_QUERY_PREFIX` | `CODESCOUT_EMBEDDING_QUERY_PREFIX` |

`OPENAI_API_KEY` stays as-is — it is a third-party convention, not ours.

**The provenance half is the actual fix.** `startup_env_assignments`
(`src/config/global.rs:113-118`) already returns exactly the keys it injected
and currently discards that list. Capture it, and have the resolver warn when a
**dotenv-injected** value shadows a value a TOML layer set. An
operator-exported value still wins silently — that is the escape hatch working.

Migrate `.env.amd` / `.env.gpu` / `.env.cpu` / `.env.example`,
`contrib/pi/mcp.json.example`, `scripts/sweep-*.sh`, `docker-compose.yml` in the
same commit as the aliases, so the tree exercises the new names while the old
ones stay supported for others.

### Task 6 — the librarian shares the resolution

`LIBRARIAN_EMBED_{MODEL,URL,API_KEY}` (`src/librarian/mod.rs:65-67`) default to
the resolved `EmbeddingSettings`, with an explicit `[librarian.embeddings]`
override retained for the case where artifacts genuinely want a different model.
The 2026-07-10 outage came from the split being mandatory; making it *optional*
keeps the capability and removes the trap.

## Phase 3 — onboarding and visibility

### Task 7 — onboarding decides on facts it actually has

Closes `eb3417ec7e02c66e`.

- `model_options_for_hardware` (`src/hardware.rs:37-86`) must consume `ctx.gpu`
  and `ctx.ram_gb` — today it reads only `ollama_available` and returns a
  constant first entry.
- Rank on **compiled features** too: a binary without `local-embed` must not
  recommend a `local:` model.
- Replace the `id: "url"` pseudo-option with a typed variant. It is inert today
  only because onboarding reads `.first()`; the moment selection becomes
  interactive it writes `model = "url"` into `project.toml`.
- Rename `model_options_cpu_only_recommends_jina` to match its assertion, and
  add a test whose expectation **varies with hardware**. Every existing test is
  monotone under "delete the ranking" — the mutation that must fail.
- Offer the options rather than silently writing `.first()`.

### Task 8 — show the resolved value and its source

The durable cure for "settings in too many places" is not only fewer places; it
is being able to see which one won. Add effective-settings reporting to
`workspace(action="status")` and a `librarian(action="doctor")` check:

```
embeddings:
  model   local:AllMiniLML6V2Q   <- .codescout/project.toml
  url     http://127.0.0.1:48081 <- ~/.config/codescout/.env  ⚠ shadows project.toml
  api_key (set)                  <- ~/.config/codescout/config.toml
  dim     768                    <- model
```

`src/tools/config/mod.rs:407-413` already surfaces
`p.config.embeddings.model` — note this is the **second copy** of the setting
that `src/main.rs:342` warns can diverge from `client.config.model`. Task 8
should report the *resolved* value, and `src/tools/memory/mod.rs:208-214` (which
sizes its chunk budget from the same divergent copy) should move to the resolved
one.

## Verification

Every task: `./scripts/gate.sh` (per-session `CARGO_TARGET_DIR`).

Two lane-vacuity traps apply here specifically and are the reason this section
is not just "run the gate":

- Task 3's guard is **meaningful only in the default lane** and vacuous under
  `--features local-embed`. Read that lane's own test names, never a total.
- `server-stack` is not in `default`, so the four-command gate never compiles
  `EmbedderHttp`'s Qdrant-side callers. Anything touching
  `from_config_only` must be read from the CI `test-server-stack` job, not
  reported green from the local gate.

Plus one end-to-end probe per phase, reusing the harness from this
investigation: a Python `/v1/embeddings` server that logs each request body,
`codescout index --force` against a scratch project, and an assertion on the
`model` and `Authorization` that actually went on the wire. That probe is what
found all four config defects; reading the source found none of them.

## References

- `src/retrieval/client.rs:209-250`, `:312-348`, `:402-432`
- `src/retrieval/config.rs:184-219`, `:251-267`, `:304-327`
- `src/retrieval/embedder.rs:280-345`, `:456-468`
- `src/config/global.rs:16-19`, `:113-118`, `:132-161`
- `src/config/project.rs:80-158`, `:461-476`, `:491-547`
- `src/hardware.rs:7-13`, `:37-86`; `src/tools/onboarding.rs:1019-1060`
- `crates/codescout-embed/src/lib.rs:197-326`; `crates/codescout-embed/src/remote.rs:313-454`
- `docs/superpowers/specs/2026-03-31-unified-embedding-config-design.md`
