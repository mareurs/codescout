# kotlin-library — Project Overview

## Purpose
A minimal Kotlin fixture used by the codescout (code-explorer) project for LSP integration
testing. It is not a real application library — it exists to provide a realistic Kotlin
codebase with enough type diversity that the LSP tests can exercise symbol navigation,
go-to-definition, hover, find-references, and related features.

## Tech Stack
- Language: Kotlin (JVM), Kotlin 2.1.0
- Build: Gradle with Kotlin DSL (`build.gradle.kts`)
- Group/artifact: `library:kotlin-library:0.1.0`
- Dependencies: `kotlin("stdlib")` only — no test framework, no external libraries
- Repository: Maven Central

## Package Structure
All 6 sources live under `src/main/kotlin/library/`:
- `models/Book.kt` — `Book` data class with companion object; `MAX_RESULTS = 100` constant
- `models/Genre.kt` — `Genre` enum with 5 values and a `label()` display method
- `interfaces/Searchable.kt` — `Searchable` interface with `searchText()` and `relevance()`
- `services/Catalog.kt` — `Catalog<T>` generic class, nested `CatalogStats`, factory functions,
  suspend extension `searchAsync`, extension `Book.toSearchText()`
- `extensions/Results.kt` — `SearchResult` sealed class (Found/NotFound/Error variants);
  `BookRegistry` singleton object
- `extensions/Advanced.kt` — `ISBN` value/inline class; `LazyBook` with delegated property;
  `createBookWithDefaults` scope function

## No Tests
There is no `src/test/` directory. This fixture is exercised exclusively by codescout's
own Rust test suite via the Kotlin LSP (kotlin-language-server).

## Key Constants
- `MAX_RESULTS = 100` (top-level constant in `Book.kt`, `library.models` package)
