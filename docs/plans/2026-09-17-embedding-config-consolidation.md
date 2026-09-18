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

**Status:** approved in outline 2026-09-17 (three rulings below). **Tasks 1–4 landed,
plus task 5a** — `654e1f18`, `de0a1e07`, `87e434dd`, `c76d43de`, `91dede5a`.
**Bugs this closes:** `aae547c917c329b5` (**fixed**), `4cd387ba07bc8d2a` (**fixed**),
`f720f7708e32d881` (**fixed**), `5de82c05bd449fdc` (**fixed**), `26d098a04a1613d6`
(**fixed**), `f5151cd081ed3a01`.

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
   already four fields behind (bug `4cd387ba07bc8d2a` is that drift realised).
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

**Landed 2026-09-17** — `de0a1e0703a79a458cd0293a225ce7cb970d8cb7`, patch-id
`95e9301f095b51cd862e81f304a1f64d52dd8483`. Closes `4cd387ba07bc8d2a`.

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

**Landed 2026-09-17** — `87e434dd488c8b98d600c2c8c4ba457ff259ad68`, patch-id
`0d32f175662fee9ba250ae632ba12797ecb939c8`. Closes `f720f7708e32d881`.
`default = ["remote-embed", "http", "librarian", "local-embed"]`.

**First, the defect was measured rather than left inferred** — the bug file had admitted
it was read from the cfg gates and never observed. A default-feature build indexing a
fresh project with no `[embeddings]` aborts with *"Local embedding requires the
'local-embed' feature"* and recommends, as the remedy, the model it just refused. After
the change the same command indexes and writes a store: no server, no env, no config.

The guard is `the_default_feature_set_can_construct_the_default_embedding_model`
(`tests/feature_lanes.rs`), observed RED before the Cargo.toml edit. It reads the
**manifest**, and that is the load-bearing choice: the natural runtime form must be
`#[cfg(feature = "local-embed")]` to survive the lean lane, and that cfg switches the
test OFF under exactly the change it guards — monotone under its own subject. Reading
the manifest is not cfg-dependent, so it runs and means the same thing in every lane.

**Three consequences found by checking rather than assuming, none of them predicted by
the plan.** Each would have shipped a break:

1. **`cargo rb` inherits it.** The alias passes no `--no-default-features`, so the live
   MCP binary regains `local-embed`, undoing a documented 2026-09-15 decision to keep it
   lean (~21MB + the ort/fastembed compile). Operator ruled 2026-09-17 to accept that;
   `.cargo/config.toml`'s comment now records the supersession beside the old reasoning
   rather than replacing it, since that reasoning holds a verification worth not
   repeating.
2. **`scripts/build-windows.sh` broke, and it runs in CI three times** (`ci.yml:575`,
   `:583`, `:778`). `ort` publishes no prebuilt for `x86_64-pc-windows-gnu` — which is
   why `local-embed-dynamic` exists — so taking cargo's default there fails in the build
   script: *"ort does not provide prebuilt binaries for the target"*. Measured locally
   before landing. The script's default is now the windows-gnu spelling of cargo's
   default (`local-embed-dynamic` substituted), and its `--edr` flag became a no-op
   alias. Verified with the exact CI commands, `check` and `clippy --all-targets -- -D
   warnings`.
3. **The gate itself went red, on a machine with no ONNX weights.** The default lane now
   COMPILES `local.rs`'s two weight tests, which panic unless `CODESCOUT_TEST_ONNX_DIR`
   is set. CI was already safe (it sets `CODESCOUT_SKIP_ONNX_TESTS=1` on non-`local-embed`
   lanes); `gate.sh` was not, and would have redded for every session on this checkout.
   `gate.sh` now exports the same opt-out.

Consequence 3 creates a **new vacuity, and it is the dangerous polarity**: those two
tests are the only ones that catch a correctly-shaped but silently wrong vector, and
they now print `... ok` while asserting nothing. The other two lane-vacuities in this
repo are absences — code a lane never compiled. This one is a green line a reader would
credit. Recorded in `CLAUDE.md` § *Development Commands* beside its two siblings, because
§ *Observer Blindness* says a bound living only in the enforcement layer (`gate.sh`'s own
comment) is published to an audience that never reads it.

Also revised: `docs/issues/archive/2026-08-08-cyberark-epm-blocks-ort-sys-build-script.md`,
whose documented workaround was *"build with default features — no `ort` anywhere in that
graph"*, and whose severity was lowered `high` → `low` **because** of it. That sentence is
now false. The escape still exists but must be asked for explicitly
(`--no-default-features --features remote-embed,http,librarian`), and the file says so.

### Task 4 — correct the docs

**Landed 2026-09-17** — `c76d43de37b828a5b4a946d56a6037a9fce96e63`, patch-id
`d32bb8ad57542e3b0577e4ba9ce14c25fb63dd70`. Closes `5de82c05bd449fdc`.

Sequenced after Tasks 1–3 as planned, so the pages were rewritten **once** against
settled behaviour. That sequencing paid: two of the corrections below describe fixes
that only became true earlier today.

The filed scope was three items; reading the pages against the code found **nine**, and
the two worst were not in the report. Both are surfaces that tell a reader where to put
a file, or hand them syntax to paste — the two things a doc can get wrong that cost more
than a misleading sentence:

- **`global-config.md`'s "File locations" table named the project file
  `.codescout/config.toml`.** Nothing reads that path; it is `.codescout/project.toml`.
  A reader following the table got no effect and no warning. Confirmed by grep — no
  reader exists anywhere in `src/`.
- **`embedding-backends.md` and `semantic-search-guide.md` both presented
  `custom:<model>@<url>` as a live backend** — a whole section with four
  copy-pasteable provider examples in the first, a table row in the second. That prefix
  was removed and now **hard-errors**. `embeddings.md` § *Migration* had documented the
  removal correctly all along, so this was drift between surfaces rather than an
  unknown: one page knew.

The rest: the inverted banner; a three-entry env-var table replaced by ten with their
precedence stated; a `[embeddings]` example using an unresolvable model id; the same
example setting the inert `chunk_size`; a documented 64 KB size guard the code has always
enforced at 1 MiB; and § *Merge semantics* asserting the opposite of `merge_toml`.

**No tests added, and the reason is the finding.** Pinning prose reds on every rewording.
Two of the nine errors were already refuted by tests nobody had read against the prose —
`merge_toml_base_fills_missing_key` against the no-deep-merge claim, and the `custom:`
bail against two pages of examples. `audit_doc_refs` reaches only path-shaped tokens
(run after: `exit_code=0`, zero `high`), and the one error it *could* have caught was
suppressed by an `audit-doc-refs:ignore` waiver written for a different reason — "a clean
checkout has no `.codescout/config.toml`" is a waiver about **absence**, and it covered a
**misspelling**. Left in place; recorded on the bug for whoever widens that lint.

## Phase 2 — consolidate the surface behind deprecations

### Task 5 — one env family, provenance-aware

**Split in two, and only the first half is done.**

### 5a — provenance and the shadowing warning — **LANDED 2026-09-17**

Closes `26d098a04a1613d6`, which is the complaint this whole plan started from: the
machine's `~/.config/codescout/.env` silently outranked every project's `[embeddings]`.
SHA `91dede5a9ddfa72f61c731b506f0a2077b2c2732`, patch-id
`d33ceab59d56977d00e4bbe4f91407790d178497`.

`startup_env_assignments` already computed exactly the keys it injected and threw the
list away. It is now recorded in `DOTENV_INJECTED` (`src/config/global.rs`), read once at
the edge by `EmbedEnv::from_real_env`, and carried as per-field `DotenvProvenance` data so
the pure `dotenv_shadowed_fields` can decide without touching a global — the same
discipline that keeps `merge_embed_config` testable without `set_var`.

**Precedence is unchanged.** Env still wins, per the ruling. That was never the defect:
the defect is that a *file read on every start* is indistinguishable from an export by
the time anything can act on it, so a machine DEFAULT silently acquired OVERRIDE
precedence. Half the fix was Task 2 — the global `config.toml` can now hold `url` and
`api_key`, so those values finally have a place to live *beneath* the project layer.
Before that, the dotenv was not a bad habit; it was the only place they fit.

Five tests, all on the pure predicate, mutation-settled once per **condition**: dropping
the provenance check kills exactly the two tests that assert provenance matters; dropping
the config-is-set check kills exactly the one that asserts quiet-when-nothing-shadowed.
Verified end-to-end on the real binary — the warning fires for a dotenv value and is
silent for the same value exported.

The third gate condition is what keeps this from becoming noise: it stays quiet when the
config layers set nothing, which is the state of a machine configured entirely through
`.env` — this repo's own. A warning that fired on every resolution for its largest
audience would be tuned out, and the law it would violate is the one about alarms nobody
acts on.

### 5b — the `CODESCOUT_EMBEDDING_*` family — **LANDED 2026-09-17** (`70ec79d5`)

One deliberate exception, named below.

The fix is the **declaration**, not the rename. Eight variables reached three settings
because nothing declared the set — each reader spelled its own name at its own call site,
so the duplicates were visible only to somebody grepping the whole tree at once, and the
surface could grow without a diff anyone would question.
`src/config/embedding_env.rs` is now that declaration: a table of canonical names and
their deprecated aliases, plus a pure `pick` over an injected lookup.

| canonical | deprecated, still working |
|---|---|
| `CODESCOUT_EMBEDDING_MODEL` | `CODESCOUT_EMBEDDER_MODEL`, `CODESCOUT_EMBED_MODEL` |
| `CODESCOUT_EMBEDDING_URL` | `CODESCOUT_EMBEDDER_URL`, `CODESCOUT_EMBED_URL` |
| `CODESCOUT_EMBEDDING_API_KEY` | `EMBED_API_KEY` |
| `CODESCOUT_EMBEDDING_DIM` | `CODESCOUT_MODEL_DIM` |
| `CODESCOUT_EMBEDDING_QUERY_PREFIX` | `CODESCOUT_QUERY_PREFIX` |

The worse half of the old surface is closed by this too: `CODESCOUT_EMBED_*` applied
inside `ProjectConfig::load_or_default` while `CODESCOUT_EMBEDDER_*` applied in
`merge_embed_config` beneath it, so two independently-named variables reached one
effective setting at two points in the same resolution. Both call sites now read the same
canonical name through the same function, so the layers agree by construction rather than
by coincidence.

Alias precedence is **preserved, not reinvented** — `CODESCOUT_EMBEDDER_*` still beats
`CODESCOUT_EMBED_*`, pinned by `the_older_alias_order_is_preserved`. Re-ordering that list
would change the effective model on any machine setting both: a config change disguised as
a refactor.

Verified at runtime, all three directions: old names work **and warn once**; new names
work silently; both set → canonical wins, silent.

### The exception: the live `.env.*` files are NOT migrated

`~/.config/codescout/.env` is a symlink to `.env.amd`, which configures the **running**
MCP server, and `target/release/codescout` is whatever `cargo rb` last produced — checked
at the time of writing: built from a SHA predating this rename. Renaming the variables in
that file before that binary is rebuilt would leave the server with **no embedder config
at all, silently** — precisely the failure class this whole plan exists to end. Shipping
the cleanup would have caused the defect the cleanup is about.

So `.env.amd` keeps the old names and carries a header stating why, and the migration
order: rebuild, reconnect, confirm the binary's SHA, *then* rename. `.env.example` — a
template nothing reads live — shows the target shape today.

`CODESCOUT_EMBEDDER_MODEL_NAME` is deliberately **not** folded into the family. It is a
different setting (the wire name, winning over the model spec), it is now redundant
because Task 1 made the configured model reach the wire, and every stack deployment sets
it — collapsing it would silently repoint them. It stays as a documented, deprecated
override.

Still owed from this half: `LIBRARIAN_EMBED_*` (Task 6) and the unknown-key warning
deferred from Task 2, which now has the warn-once surface it needed.

### Task 6 — the librarian shares the resolution

`LIBRARIAN_EMBED_{MODEL,URL,API_KEY}` (`src/librarian/mod.rs`) now fall back to the
resolved `CODESCOUT_EMBEDDING_{MODEL,URL,API_KEY}` family (via
`embedding_env::read`, so a deprecated alias reached only through this fallback
still warns once) when the explicit `LIBRARIAN_EMBED_*` var is absent. The
explicit var still wins when set, so a deployment that genuinely wants a
different model for artifacts than for code retrieval keeps that capability;
the fix removes the case that caused the 2026-07-10 outage — the split being
**mandatory** rather than optional.

Also fixed, discovered as a direct consequence: `src/cli/doc.rs`'s `--semantic`
pre-check read `LIBRARIAN_EMBED_MODEL` directly, bypassing the fallback — a user
with only `CODESCOUT_EMBEDDING_MODEL` configured would have hit a false
"requires the embedding service" refusal from the CLI even though the actual
context build (`open_ctx` → `LibrarianEnv::from_env`) would have worked. Fixed
by extracting the check as a pure `semantic_search_needs_an_embedder(semantic,
&LibrarianEnv)` and wiring the pre-check through the same `LibrarianEnv` the
real construction uses, so the two can no longer disagree.

**Not done, deliberately scoped out:** the `[librarian.embeddings]` project.toml
override the plan originally named. It would need a new `ProjectConfig` section
AND restructuring `build_tool_context_with` to resolve the embed model only
after `current_project` is known (today the embedding init runs before that
resolution). The env-layer fallback above already kills the *mandatory-split*
defect for the common case (anyone using env vars, which is exactly how the
outage-causing `.env.amd`-style files work); the project-scoped override is a
separate, larger change and is left as a follow-up rather than rushed in
alongside it.

SHA / patch-id: pending.

## Phase 3 — onboarding and visibility

### Task 7 — onboarding decides on facts it actually has — **LANDED 2026-09-17**

Closes `f5151cd081ed3a01` (archived). Fixed in `3aa12d8c`, patch-id
`40bdc6e202a50137561f80c495802f14b96f5526`.

- **Done.** `model_options_for` ranks on `ram_gb` / `cpu_cores` (16 GB + 8-core
  crossover between `local:AllMiniLML6V2Q` and `local:JinaEmbeddingsV2BaseCode`)
  and on `gpu`.
- **Done, with a correction to this plan.** The plan said VRAM should promote
  the local code model. It must not: the local ONNX path is CPU-only in every
  shipped build (`ort` CPU prebuilt or the dynamic C ABI; no execution provider
  registered), so a GPU accelerates only what **Ollama** serves. `gpu` promotes
  the Ollama entry instead. Ranking a local model on GPU presence would have
  swapped a constant for a confidently wrong recommendation.
- **Done.** Compiled features rank, carried as `CompiledBackends` **data**
  rather than `cfg!` inside the ranking — a `cfg!` there makes each lane
  exercise half the branches and report a full pass.
  **Second correction:** `--no-default-features` drops `remote-embed` too, so a
  naively gated list comes back **empty** and `onboarding.rs`'s
  `.first().expect(...)` panics. A terminal fallback entry names the missing
  backend and still writes the built-in default.
- **Done.** `id: "url"` → `OptionTarget::{Model, Server}`, with
  `ModelOption::embeddings_section()` owning the mapping onboarding did inline.
- **Done.** Renamed to `a_small_host_leads_with_the_light_model`; the
  differential is `model_options_rank_differs_across_hosts`. 15 tests, and
  **eight mutation sites killed by eight distinct named tests** — see the bug
  file's *Tests added* table.
- **Not done, deliberately.** *"Offer the options rather than silently writing
  `.first()`."* The ranked list is surfaced (it already travelled in
  `subagent_prompt` under **Model options**, now with a `target` and a stated
  reason per entry), so the agent can present alternatives. But onboarding still
  writes `.first()` eagerly: the MCP tool has no channel to prompt a human
  mid-call, so genuine interactivity belongs to the agent layer, not here.
  Writing a ranked default the agent can then change is the honest shape for a
  tool that cannot ask.

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
