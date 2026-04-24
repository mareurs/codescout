# kotlin-library — Project Overview

## Purpose
A minimal Kotlin fixture used by the codescout (code-explorer) project for LSP integration
testing. It is not a real application library — it exists to provide a realistic Kotlin
codebase with enough type diversity that the LSP tests can exercise symbol navigation,
go-to-definition, hover, find-references, and related features.

## Tech Stack
- Language: Kotlin (JVM), Kotlin 2.1.0
- Build: Gradle with Kotlin DSL (`build.gradle.kts`)
- Dependencies: `kotlin("stdlib")` only — no test framework, no external libraries
- Group/version: `library:0.1.0`

## Package Structure
All sources live under `src/main/kotlin/library/`:
- `models/` — core domain types: `Book` (data class), `Genre` (enum)
- `interfaces/` — `Searchable` interface
- `services/` — `Catalog<T>` generic class, catalog factory functions, extension functions
- `extensions/` — advanced Kotlin features: `SearchResult` (sealed class), `BookRegistry`
  (singleton object), `ISBN` (value/inline class), `LazyBook` (delegated property)

## No Tests
There is no `src/test/` directory. This fixture is exercised exclusively by codescout's
own Rust test suite via the Kotlin LSP.

## Key Constants
- `MAX_RESULTS = 50` (top-level constant in `Book.kt`)
