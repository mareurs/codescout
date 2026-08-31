# Workspace Domain Glossary

## Shared across all 5 language fixture libraries

| Term | Definition |
|---|---|
| `Searchable` | Interface/trait requiring `search_text() -> str/String` — the universal indexing contract across Java, Kotlin, Python, Rust, TypeScript fixtures |
| `Catalog<T>` | Generic container service for `Searchable` items; exposes `add`, `search` (substring filter on `search_text()`), and `stats` |
| `Book` | Primary domain entity: title, isbn, genre, copiesAvailable; used across all 5 fixture languages |
| `Genre` | Enum of book categories (Fiction, NonFiction, Science, History, Biography) with a humanizing `label()` method |
| `SearchResult` | Discriminated union / sealed class / enum with `Found`, `NotFound`, `Error` variants |
| `CatalogStats` | Value object returned by `Catalog.stats()` — totalItems + name |
| `AudioBook` | Appears in Python fixture only — extends Book + Playable mixin; the only concrete `Searchable` in that fixture |

## codescout specific

| Term | Definition |
|---|---|
| `RecoverableError` | `isError: false` MCP error for expected input failures — sibling tool calls survive |
| `OutputGuard` | Enforces progressive disclosure: Exploring mode (compact, capped at 200) / Focused mode (full, paginated) |
| `ToolContext` | Per-call context carrying Agent, LspManager, output buffer, and progress reporter |
| `Agent` / `ActiveProject` | Project state holder (config, memory, write locking); tools call `with_project(|p| ...)` |
| `call_content()` | MCP entry point for tools — handles buffer routing; `call()` is the inner logic |
| `server_instructions` | MCP session-start injected prompt surface (live on every connect; no cache) |
| `onboarding_prompt` | Stored per-project system prompt surface (cached; bump `ONBOARDING_VERSION` to refresh) |
| `librarian` | SQLite-backed artifact registry indexing markdown docs (specs, plans, ADRs, trackers) |

## codescout-embed specific

| Term | Definition |
|---|---|
| `Embedder` | Async trait: `embed(&[&str]) -> Vec<Embedding>` + `embed_query(&str)` |
| `RawChunk` | Pre-embedding text chunk with 1-indexed `start_line` / `end_line` provenance |
| `model_spec` | Full model identifier including prefix: `local:AllMiniLML6V2Q`, `ollama:<name>`, `openai:<name>` |
| `chunk_size` | In characters (not tokens); `floor(max_tokens × 0.85 × 3)` (`chunk_size_for_model`, `crates/codescout-embed/src/lib.rs:72`). **Local models are then CLAMPED to fastembed's real 512-token ceiling = 1305 chars** — the formula alone would over-chunk a large-context model against a hard cap. `ollama:` specs are not clamped. Pinned by `local_models_are_clamped_to_fastembeds_actual_token_ceiling` |

## Eval fixture specific (edit-eval-rust)

| Term | Definition |
|---|---|
| `EditCase` | One eval scenario: input JSON for `edit_code` + expected disk invariants + compiler expectation |
| `ContentInvariant` | Assertion that a file Contains or NotContains a needle string after the edit |
| `CompilerExpected` | `Builds` (fixture compiles after edit) or `Breaks` (intentional compile failure) |
| `Verdict` | Eval outcome — **seven** variants (`tests/e2e/eval_common/verdict.rs:2`): `Correct`, `Partial`, `CleanError`, `SilentWrong`, `Corrupt`, `Hung`, `Panic`. The three middle ones are the discriminating cases: `CleanError` is a refusal that reported itself, `SilentWrong` is the dangerous one, `Corrupt` is a damaged file. Verified 2026-08-31 |