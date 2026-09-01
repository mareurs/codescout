---
id: c04ed720336733d4
kind: bug
status: fixed
title: 'BUG: ProjectStatus''s embedding backend classifier misreports two configs that backend_is_local doesn''t recognize'
tags:
- embedding
- status
- diagnostics
- retrieval
closed: 2026-08-14
opened: 2026-08-11
owner: marius
severity: medium
---

# BUG: ProjectStatus's embedding backend classifier misreports two configs that backend_is_local doesn't recognize

## Summary

`ProjectStatus`'s backend classifier routes through
`RetrievalClient::backend_is_local`, which recognised only `local:`/`local-dir:`
model prefixes. Two real configurations fell outside that recognition.

**The original filing called both "status-string only". That was wrong for the
first one, and the error inverted the fix.** `backend_is_local` is also read by
`guard_sparse` and `dense_only` — in the same function, three lines above the
embedder construction the file said was unaffected — so a bare-name local config
ran a **hybrid query against an embedder that emits no sparse vector**. That is
the silent recall degradation `guard_sparse` exists to turn into a loud error.
Fixed by widening the classifier; the status string was fixed as a consequence.

| | case | effect | fixed by |
|---|---|---|---|
| 1 | bare model name, no url, `local-embed` compiled | **live path**: sparse leg left on against a local embedder; `resolve_model_dim` sized collections from the default instead of the model | widening `backend_is_local` |
| 2 | `--no-default-features` build, `ollama:`/`openai:` model, no url | status said `remote-http` for a config that cannot build anything | a new `ProjectStatus` arm reporting `unavailable` |
## Symptom (Effect)

### Case 1 — bare unprefixed model name, `local-embed` compiled

`create_embedder_with_config` arm 6 (`crates/codescout-embed/src/lib.rs:286-293`)
resolves a bare model name as a local ONNX model via `LocalEmbedder::new(model)`
— the embedder actually constructed IS local. `backend_is_local` tested only the
`local:`/`local-dir:` prefixes, so a bare name never matched. Three consequences,
not one:

1. **`guard_sparse` did not fire** (`src/retrieval/client.rs`, read in
   `build_embedder`). Its own doc comment says a local backend emits no sparse
   vector and that "silently dropping to dense would show up as degraded recall
   and never as a failure, so it is an error". For a bare-name model the guard
   stayed quiet and the hybrid sparse leg ran anyway.
2. **`dense_only` returned false**, so the query was composed as hybrid.
3. **`resolve_model_dim` took its no-op branch**, sizing a fresh collection from
   `DEFAULT_MODEL_DIM` instead of asking the model — wrong for any bare-name
   model whose real dimension differs.
4. `workspace(action="status")` reported `"embedding_backend": "remote-http"`.

Only (4) is cosmetic. Measured failure string, from the pre-fix run of the new
status test:

```
assertion `left == right` failed: a bare model name with no url resolves through
arm 6 to a local ONNX embedder, so the status must name it, got: String("remote-http")
  left: String("remote-http")
 right: String("local-onnx")
```

### Case 2 — `--no-default-features` build, `ollama:`/`openai:` model, no url

`remote-embed` is a default feature (`Cargo.toml`), so `--no-default-features`
compiles out arms 3/4 and `create_embedder_with_config` falls through to
`anyhow::bail!("Unknown model …")` — nothing can be built. Status nevertheless
reported `"remote-http"`, as if the config would work over the network. This one
really is status-string-only: any real query fails loudly.
## Reproduction
- Case 1: build with `local-embed` compiled in, set
  `[embeddings].model = "AllMiniLML6V2Q"` (no `local:` prefix), no `url`. Call
  `workspace(action="status")` → `embedding_backend` reads `"remote-http"`,
  even though `create_embedder_with_config` would build a `LocalEmbedder` for
  that exact model string.
- Case 2: build with `cargo build --no-default-features` (or
  `--features local-embed --no-default-features`), set
  `[embeddings].model = "ollama:nomic-embed-text"`, no `url`. Call
  `workspace(action="status")` → `embedding_backend` reads `"remote-http"`,
  even though `create_embedder_with_config` has no compiled path that can
  build anything for that model (arms 3/4 need `remote-embed`, which is off)
  and would `bail!` with "Unknown model" if actually invoked.

Not run end-to-end in this session — derived from reading the two functions
against each other (see Root cause).

## Environment
Branch `feat/local-onnx-query-path`, commit `4962126ee10947e5b874bd64949b8e75c7078ca3`
at time of filing. Applies to any codescout build; case 2 is specifically a
`--no-default-features` build (a real CI matrix row, not hypothetical).

## Root cause

**Measured 2026-08-14 at `141b2cbf`.** Both premises re-verified at HEAD before
any change: `backend_is_local` still had the two-prefix body, and arm 6 still
resolves bare names (`crates/codescout-embed/src/lib.rs:286-293`). The filing's
line numbers had shifted; its mechanism held.

`backend_is_local` was a **narrower** recognizer than
`create_embedder_with_config`'s resolution ladder: it tested `local:`/`local-dir:`
and had no branch for arm 6.

### The filing's own reasoning was falsified

The original Root cause said:

> Both are status-string-only: the actual embedder construction … goes through
> `create_embedder_with_config` directly, never through `backend_is_local`.

True about *which embedder is built* — and irrelevant, because it conflated that
with *how the query is composed*. `build_embedder`
(`src/retrieval/client.rs`) does both, in this order:

```rust
Self::guard_local_model_with_url(config)?;
Self::guard_sparse(config, lite)?;              // <-- reads backend_is_local
let dense_only = Self::dense_only(config, lite); // <-- reads backend_is_local
…
codescout_embed::create_embedder_with_config(&config.model, …)  // arm 6 -> local
```

So for case 1 the sparse/dense decision and the construction disagree, three
lines apart. `references(symbol="backend_is_local")` finds four consumers, not
the one the filing considered: `guard_sparse`, `dense_only`, `resolve_model_dim`,
and `ProjectStatus`. Three of the four are live behaviour.

### Which inverts the fix argument

The filing argued against widening `backend_is_local` because "widening … changes
what those two functions decide for real queries — strictly riskier than a
status-string residual." But what those functions decided for case 1 was already
wrong, so widening is what **fixes** the live path rather than what endangers it.
The risk the filing weighed was the risk of *not* widening.

Widening is also narrow in blast radius: it changes the answer only when
`embedder_url.is_none()` **and** the model carries none of the known prefixes
**and** a local backend is compiled. Every previously-classified case — url set,
`local:`, `local-dir:`, `ollama:`, `openai:`, `custom:` — is bit-for-bit
unchanged, which the 19 pre-existing `selection_tests` confirm.
## Evidence
### `create_embedder_with_config`'s resolution order (doc comment)
`crates/codescout-embed/src/lib.rs:154-162`:
```
/// Resolution order:
/// 1. `url` set → RemoteEmbedder targeting that URL
/// 2. `model` starts with `local-dir:` → local ONNX loaded from a directory, no network;
///    or starts with `local:` → local ONNX via fastembed
/// 3. `model` starts with `ollama:` → Ollama (errors loudly if unreachable)
/// 4. `model` starts with `openai:` → OpenAI API
/// 5. `model` starts with `custom:` → hard error with migration hint
/// 6. No url, no known prefix → try `model` as a bare local model name; else
///    the unknown-model error (there is no silent default)
```

### Arm 6 — bare name resolves local
`crates/codescout-embed/src/lib.rs:265-271`:
```rust
// 6. No prefix — try as local model name
#[cfg(any(feature = "local-embed", feature = "local-embed-dynamic"))]
{
    // Try parsing as a local model name directly
    if local::LocalEmbedder::new(model).await.is_ok() {
        return Ok(Box::new(local::LocalEmbedder::new(model).await?));
    }
}
```

### `backend_is_local` — narrower recognizer, shared with the live query path
`src/retrieval/client.rs:37-40`:
```rust
pub(crate) fn backend_is_local(config: &RetrievalConfig) -> bool {
    config.embedder_url.is_none()
        && (config.model.starts_with("local:") || config.model.starts_with("local-dir:"))
}
```
No branch for a bare name, no branch reading compiled features. Called by
`guard_sparse` (`:48`) and `dense_only` (`:68`), which decide real
query-path behavior — this is why widening it is riskier than the residual.

### CI's `--no-default-features` matrix rows
`.github/workflows/ci.yml:71-76`:
```yaml
- name: default
  flags: ""
- name: local-embed
  flags: "--features local-embed --no-default-features"
- name: no-features
  flags: "--no-default-features"
```
Both the `local-embed` row (`:74`) and the `no-features` row (`:76`) disable
the default `remote-embed` feature (`Cargo.toml:175`).

## Hypotheses tried
N/A — root cause fully determined by reading the resolution ladder against
`backend_is_local`; no dead ends worked in this session.

## Fix

Implemented in `5b7536f5` (`experiments`). `master` is a strict ancestor
(`git rev-list --left-right --count master...experiments` → `0 660`), so promotion
is a fast-forward and this SHA already is the master-side SHA — there is no
second SHA to record.

**Case 1 — `src/retrieval/client.rs`.** `backend_is_local` now delegates to a new
module-level `model_names_local_backend`, which mirrors the resolver's no-url
ladder: an explicit `local:`/`local-dir:` prefix, **or** a bare name when
`cfg!(any(feature = "local-embed", feature = "local-embed-dynamic"))` — because
arm 6 is then the only arm that can build anything for such a string. The prefix
set lives in one place: `REMOTE_MODEL_PREFIXES` (`ollama:`, `openai:`) plus
`HARD_ERROR_MODEL_PREFIX` (`custom:`, arm 5).

**Case 2 — `src/tools/config/mod.rs`.** A new arm, the mirror of the existing
local one: when the model names a remote backend and `remote-embed` is not
compiled, report `unavailable` rather than `remote-http`. The `embedding_hint`
now branches too — telling someone with an `ollama:` model to rebuild with
`--features local-embed` would send them to the wrong backend. This stays inside
`ProjectStatus` and does **not** re-create the drift risk finding I1 removed: it
answers "can this binary build the remote backend this model names", a question
`backend_is_local` deliberately does not answer.

### Deliberately not done

`backend_is_local` is **not** made feature-aware for the remote prefixes. On a
lean build an `ollama:` model has no constructible arm, and bailing with "Unknown
model" is the right error; classifying it local would instead raise
`guard_sparse`'s "local backend produces no sparse vector", a false explanation.
Pinned by `a_remote_prefixed_model_is_never_local_regardless_of_compiled_features`.

### Accepted cost

A bare name that is not a real local model (a typo) now trips `guard_sparse`
before construction reports "Unknown model", so diagnosing a typo can take two
steps instead of one. Worth it: a typo fails either way, whereas the config this
fixes was working and silently degraded. Documented at the predicate.

### One existing test was amended, deliberately

`status_reports_remote_http_for_an_urlless_ollama_model_regardless_of_compiled_backends`
(finding I1's regression guard) asserted `remote-http` *unconditionally*. Its
stated rationale — "the config works fine over the network" — holds only when
`remote-embed` is compiled, which I1 implicitly assumed because it is a default
feature. The assertion is now feature-aware; **I1's original expectation is
preserved verbatim for every build that has the feature**, which is every default
build including CI's. The test's name still reads true: whether a *local* backend
is compiled cannot change the answer for an `ollama:` model. Verified in both
directions (see § Tests added).
## Tests added

**`src/retrieval/client.rs`, `selection_tests`** — five new:

- `no_url_with_a_bare_model_name_selects_the_local_backend_when_compiled_in`
  (`#[cfg(any(feature = "local-embed", feature = "local-embed-dynamic"))]`).
  Asserts the predicate **and both live-path consequences** — `dense_only` true
  and `guard_sparse` erroring. Asserting the predicate alone would not catch a
  future caller that stops consulting it.
- `no_url_with_a_bare_model_name_selects_nothing_local_without_a_local_backend`
  — the `cfg(not(…))` half, so both sides of the gate are covered by some lane.
- `a_remote_prefixed_model_is_never_local_regardless_of_compiled_features` —
  pins the deliberate non-fix.
- `the_custom_prefix_is_not_classified_local` — arm 5 must not fall through the
  bare-name branch.

**`src/tools/config/tests.rs`** — `status_reports_local_onnx_for_an_urlless_bare_model_name`
(feature-gated), plus the feature-aware amendment to I1's guard.

### Discrimination, verified rather than assumed

Mutating `model_names_local_backend`'s bare-name branch to `false` (the pre-fix
behaviour) failed exactly the two case-1 tests, on the right assertions:

```
no_url_with_a_bare_model_name_selects_the_local_backend_when_compiled_in ... FAILED
  arm 6 resolves a bare name locally, so the classifier must agree
status_reports_local_onnx_for_an_urlless_bare_model_name ... FAILED
  … got: String("remote-http")  left: String("remote-http")  right: String("local-onnx")
```

**A caveat worth recording.** On the first mutation run the status test *passed*.
It and its I1 sibling both begin with ambient-env skip guards that `return` early
when `CODESCOUT_EMBED_MODEL` / `CODESCOUT_EMBEDDER_MODEL` / an embedder url is
set — and this developer machine sets three of them
(`CODESCOUT_EMBED_URL`, `CODESCOUT_EMBED_MODEL`, `CODESCOUT_EMBEDDER_URL`). Both
tests are therefore **vacuous on this host** and only assert anything in CI,
where those vars are unset. Discrimination was proved by re-running under
`env -u` for each var. The guards are deliberate and documented, but a skip that
announces itself only through `eprintln!` is invisible in a passing run — logged
as a friction, not fixed here.

### Gate

All three CI feature configurations, tests and clippy:

| config | `cargo test --lib` | `clippy --all-targets -D warnings` |
|---|---|---|
| default | 3610 passed / 0 failed / 7 ignored | clean |
| `--no-default-features --features local-embed` | 2690 / 0 / 7 | clean |
| `--no-default-features` | 2687 / 0 / 7 | clean |

The amended I1 guard was additionally run with the ambient vars stripped in both
directions: `remote-http` in the default build, `unavailable` in the lean one.
## Workarounds

N/A — fixed. For anyone on a pre-fix binary hitting case 1: set
`CODESCOUT_DISABLE_SPARSE=1`, or give the model its explicit `local:` prefix,
which the old classifier recognised.
## Resume

N/A — fixed and gated across all three feature configurations.
## References
- `src/tools/config/mod.rs:251-455` — `ProjectStatus::call`; embedding backend
  section at `:297-351`
- `src/retrieval/client.rs:37-69` — `backend_is_local`, `guard_sparse`, `dense_only`
- `crates/codescout-embed/src/lib.rs:154-292` — `create_embedder_with_config`,
  the full resolution ladder
- `.github/workflows/ci.yml:71-76` — the `--no-default-features` CI matrix rows
- `src/tools/config/tests.rs:356-418` —
  `status_reports_remote_http_for_an_urlless_ollama_model_regardless_of_compiled_backends`,
  the sibling test covering the adjacent (already-fixed) misreport this
  residual sits next to
- `docs/superpowers/plans/2026-08-11-local-onnx-embedding-query-path.md` —
  the plan that introduced `backend_is_local` as the shared classifier
