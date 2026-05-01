# kotlin-library — Project Overview

## Purpose

A small Kotlin library fixture used as a test target for codescout's LSP and symbol
navigation tools. It models a simplified book catalog domain and deliberately exercises
a range of Kotlin language features so that codescout can be verified against them.

## Tech Stack

- **Language:** Kotlin (JVM target, stdlib only)
- **Build system:** Gradle with Kotlin DSL (`build.gradle.kts`)
- **Kotlin version:** 2.1.0
- **Group / version:** `library` / `0.1.0`
- **Dependencies:** `kotlin("stdlib")` only — no third-party runtime deps
- **Test suite:** None — this is a fixture, not a production library

## Package Layout

```
library/
  interfaces/   Searchable interface
  models/       Book (data class), Genre (enum)
  services/     Catalog<T> generic class + top-level factory/extension fns
  extensions/   ISBN value class, LazyBook, SearchResult sealed class,
                BookRegistry singleton object, createBookWithDefaults scope fn
```

## Key Files

| File | Role |
|---|---|
| `models/Book.kt` | Core domain type; `data class` with companion factory |
| `models/Genre.kt` | Enum with a display-label helper |
| `interfaces/Searchable.kt` | Search contract; `Catalog` is bounded on this |
| `services/Catalog.kt` | Generic collection + search; KDoc coroutine/extension extras |
| `extensions/Results.kt` | Sealed result hierarchy + `BookRegistry` singleton |
| `extensions/Advanced.kt` | `@JvmInline` value class, `by lazy`, scope functions |
