---
id: eb3417ec7e02c66e
kind: bug
status: open
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

Plan, not yet implemented; part of the 2026-09-17 embeddings-config work.

1. Make the ranking a real function of hardware: VRAM and RAM should move
   `local:JinaEmbeddingsV2BaseCode` (768d, ~300 MB, code-specialised) above the
   22 MB default on a machine that can afford it, and should keep the small
   model first on a constrained one.
2. Rank on **compiled features** as well — a binary without `local-embed` must
   not recommend a `local:` model at all (see the sibling bug on default
   features).
3. Give the url entry a distinct variant rather than `id: "url"`, so a selection
   cannot be written into `model` verbatim.
4. Rename `model_options_cpu_only_recommends_jina` to match its assertion, and
   make at least one test's expectation **vary with hardware** — otherwise the
   ranking added in step 1 is uncovered by construction.

SHA / patch-id: pending.

## Tests added

None yet. The owed shape is specific: a test that asserts *different* hardware
yields *different* `opts[0].id`. Every existing test is monotone under "delete
the hardware ranking", which is exactly the mutation that would have to fail.

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
