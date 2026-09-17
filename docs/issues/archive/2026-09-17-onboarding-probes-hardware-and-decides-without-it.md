---
id: f5151cd081ed3a01
kind: bug
status: fixed
title: 'BUG: onboarding probes hardware and then decides without it'
tags:
- cluster/assertion-that-cannot-fail
- embeddings
- onboarding
- hardware
---

## Summary

`detect_hardware_context` probes GPU vendor + VRAM, RAM, and CPU core count.
`model_options_for_hardware` — the only consumer — reads **none of them**. It
branches on `ollama_available` alone and always recommends
`local:AllMiniLML6V2Q`. Onboarding then takes `options.first()` and writes that
constant into `project.toml`. Four of the five detected hardware facts are
computed and discarded, and the tests cannot notice because every fixture varies
hardware while asserting a constant.

## Symptom (Effect)

Onboarding on a 32 GB / 16-core / RTX 3080 host and on an 8 GB / 4-core
CPU-only host produce the **same** `[embeddings].model`. The probe cost is paid
(a `nvidia-smi` / ROCm / RAM read per onboarding) and buys nothing. The
recommendation offered to the user never reflects the machine it ran on.

## Reproduction

Tree `35b622f8f53cb61fa301733403f3a325fcf17199`. Read
`model_options_for_hardware` (`src/hardware.rs:37-86`) and search its body for
`ctx.gpu`, `ctx.ram_gb`, `ctx.cpu_cores`: zero occurrences. Only
`ctx.ollama_available` (twice) and `ctx.ollama_host` (once) are read.

## Environment

Any host. `src/hardware.rs`, tree `35b622f8`.

## Root cause

`HardwareContext` (`src/hardware.rs:7-13`) carries five fields.
`model_options_for_hardware` (`:37-86`) reads two. The first element of its
returned vector is an unconditional literal:

```rust
let mut options = vec![ModelOption {
    id: "local:AllMiniLML6V2Q".into(),
    ...
    recommended: true,
}];
```

Nothing later reorders it, and `recommended` is set `true` on that entry only.
`src/tools/onboarding.rs:1024-1029` takes `model_options.first()` as
`recommended_model` and writes it to `EmbeddingsSection::model` at `:1057-1058`.
So the whole path is a constant with a hardware probe attached.

**The tests cannot detect this, and their names say otherwise.**
`model_options_cpu_only_recommends_jina` (`src/hardware.rs:289-303`) is named for
a Jina recommendation and asserts `opts[0].id == "local:AllMiniLML6V2Q"` — a
fossil name from an earlier ranking, now describing the opposite of its body.
`model_options_exactly_one_recommended` (`:306-320`) constructs
`Some(GpuInfo::Nvidia { vram_mb: 10240 })`, `ram_gb: 32`, `cpu_cores: 16` and
asserts only that exactly one entry is recommended — a property that holds for
every possible hardware input, so the fixture's richness is decorative. Across
the five tests, `gpu` / `ram_gb` / `cpu_cores` are set to four different
combinations and **no assertion varies with any of them**. This is
`CLAUDE.md` § Testing Discipline's aggregate/monotone pair in one place: the
suite pins a constant while appearing to test a function of hardware.

**Measured 2026-09-17**: read of `model_options_for_hardware`'s body (enumerated
every `ctx.` access) and of all five tests in `src/hardware.rs:267-381`.

## Evidence

### A pseudo-option that would corrupt project.toml

`model_options_for_hardware` emits entries with `id: "url"` (twice — one branch
per `ollama_available` arm). `"url"` is not a model identifier. Today it is
inert because onboarding only ever reads `.first()`, which is always the
AllMiniLM literal. The moment onboarding becomes interactive — which is the
stated goal — selecting that entry writes `model = "url"` into `project.toml`,
which resolves to the unknown-model bail. The option list and the field it
feeds have different grammars, and nothing marks the boundary.

### The fossil test name

```rust
#[test]
fn model_options_cpu_only_recommends_jina() {
    ...
    assert_eq!(opts[0].id, "local:AllMiniLML6V2Q");
```

## Hypotheses tried

1. **Hypothesis:** hardware is consumed elsewhere, and
   `model_options_for_hardware` is only one reader.
   **Test:** `references` / grep for `HardwareContext` field reads outside
   `src/hardware.rs`.
   **Verdict:** rejected — `detect_hardware_context` is called once, from
   `src/tools/onboarding.rs:1023`, and its result is passed only to
   `model_options_for_hardware` (`:1024`). The `gpu` / `ram_gb` / `cpu_cores`
   values do reach the system-prompt draft as serialised JSON
   (`src/tools/onboarding.rs:1186-1189`), so they are *displayed*; they are never
   *decided on*.

## Fix

Implemented 2026-09-17 as Task 7 of `docs/plans/2026-09-17-embedding-config-consolidation.md`.

1. `model_options_for` is the ranking proper; `model_options_for_hardware`
   delegates to it with `CompiledBackends::current()`. `ram_gb` and
   `cpu_cores` choose between the two local models at a 16 GB / 8-core
   crossover; `gpu` promotes the Ollama entry.
2. Compiled features are carried as **data** (`CompiledBackends`), not read
   from `cfg!` inside the ranking. A `cfg!` in the body would make the
   no-local case unreachable in the default lane and the local case
   unreachable in the lean one — each lane would exercise half the branches
   and report a full pass, the same vacuity shape `CLAUDE.md` records for the
   lean and `server-stack` lanes.
3. `ModelOption.id: String` became `ModelOption.target: OptionTarget`, a tagged
   enum of `Model { model }` / `Server { url, model }`. `ModelOption::embeddings_section()`
   performs the mapping onboarding used to do inline.
4. `model_options_cpu_only_recommends_jina` → `a_small_host_leads_with_the_light_model`.

**Two corrections the reproduction forced on this plan** — both found by running
it before reading the plan, per `CLAUDE.md` § Bug Tracking:

- **Step 1 named VRAM, and VRAM must not rank a `local:` model.** The local
  ONNX path is CPU-only in every shipped configuration:
  `crates/codescout-embed/Cargo.toml` selects `ort`'s CPU prebuilt
  (`ort-download-binaries-native-tls`) or the dynamic C ABI, and `local.rs`
  registers no execution provider. Ranking a local model on GPU presence would
  have replaced a constant with a confidently wrong recommendation. `gpu` ranks
  the **Ollama** entry instead, which is the one thing on the list a GPU
  actually accelerates.
- **Step 2, done naively, is a panic.** `--no-default-features` drops
  `remote-embed` as well as `local-embed`, so a feature-gated list can come back
  **empty** — and `onboarding.rs` calls `.first().expect(...)`. The lean lane
  ships that combination. A terminal fallback entry names the missing backend
  and still writes the built-in default, so a later rebuild finds a usable
  config.

A third defect was introduced by the fix and caught by its own new test before
commit: `recommended: options.is_empty()` on the external-server entry was
evaluated *after* the OpenAI entry had been pushed, so a `local:false,
remote:true` build recommended **nothing**. That is the configuration
`scripts/build-windows.sh` shipped before `local-embed-dynamic` became its
default.

**SHA:** `3aa12d8c` · **patch-id:** `40bdc6e202a50137561f80c495802f14b96f5526`

Gate green at that commit: `FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0`. Not credited from
that run: `local::tests::from_dir_produces_a_stable_384d_vector` and
`from_dir_matches_the_hub_path_for_the_same_model` printed `ok` under
`CODESCOUT_SKIP_ONNX_TESTS=1` and asserted nothing — their real lane is CI's
`local-embed` matrix job.

## Tests added

`src/hardware.rs` — 15 tests, up from 7. The load-bearing one is the
differential this file asked for:

```rust
assert_ne!(small[0].target, large[0].target)   // 8GB/4-core vs 32GB/16-core
```

It is the only shape that is not monotone under deleting the ranking: a
constant makes the two sides equal and reds it. `a_gpu_promotes_ollama_over_a_cpu_bound_local_model`
is the same shape on the GPU axis, and `a_gpu_without_ollama_changes_nothing`
pins the other direction — a GPU with nothing to serve it must change nothing.

**Mutation-probed once per guarded site** (`./scripts/mutation-probe.sh`,
isolated worktree, 2026-09-17). Eight sites, eight KILLs, each by a distinct
named test:

| site | mutation | killed by |
|---|---|---|
| the ranking | `let roomy = false` | `model_options_rank_differs_across_hosts` |
| RAM bound | drop the conjunct | `either_bound_alone_is_not_enough_for_the_code_model` |
| cores bound | drop the conjunct | `either_bound_alone_is_not_enough_for_the_code_model` |
| GPU gate | `if ollama_usable` | `a_gpu_promotes_ollama_over_a_cpu_bound_local_model` |
| feature gate | `if true` | `a_build_without_a_local_backend_never_recommends_a_local_model` |
| empty fallback | `if false` | `every_backend_combination_yields_at_least_one_option` |
| mapping → url | `url: None` | `taking_an_option_writes_the_fields_that_option_names` |
| mapping → model | `model: None` | `taking_an_option_writes_the_fields_that_option_names` |

The two bounds were mutated **separately** rather than as a conjunction, per
`CLAUDE.md` § Testing Discipline's guard-ordering twin: a pair of tests that
kills only the conjunction leaves each bound individually uncovered.

`taking_an_option_writes_the_fields_that_option_names` exists because
onboarding probes the real host, so on any machine with a local backend the
recommended entry is a `Model` and the url-writing branch never runs — coverage
of it would otherwise have been a property of the test machine.

## Workarounds

Set `[embeddings].model` by hand after onboarding.

## Resume

Rewrite `model_options_for_hardware` to consume `ctx.gpu` / `ctx.ram_gb`, and
add the differential test **first** — observe it RED against the current
constant-returning implementation, since that red is the only evidence the new
assertion discriminates.

## References

- `src/hardware.rs:7-13`, `:37-86`, `:267-381`
- `src/tools/onboarding.rs:1022-1029`, `:1057-1058`, `:1186-1189`
- Sibling bug: the default build cannot run the default embedding model.
