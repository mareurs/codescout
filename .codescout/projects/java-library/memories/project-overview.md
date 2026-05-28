# java-library

## Purpose
A minimal Java library fixture used as a test target for codescout's LSP and symbol navigation
features. It is not a real application — it exists to exercise Java/Kotlin LSP tooling
(goto definition, find references, symbol listing, hover, etc.) in codescout's integration tests.

## Tech Stack
- **Language:** Java 21 (`sourceCompatibility = JavaVersion.VERSION_21`)
- **Build:** Gradle (`build.gradle`, `settings.gradle`) — no Gradle wrapper jar checked in
- **Group/version:** `library:0.1.0`
- **Framework:** None — plain Java library, no external runtime deps
- **Key deps:** None beyond the JDK

## Runtime Requirements
- JDK 21+
- Gradle 7+ (or compatible; uses `id 'java'` plugin only)
- Build: `./gradlew build`

## What the Codebase Demonstrates
This fixture intentionally demonstrates a wide range of Java 21 language features to stress-test
codescout's symbol indexing and LSP integration:
- Records (`Book`, and nested `Found`/`NotFound`/`Error`)
- Sealed interfaces (`SearchResult`)
- Generics with bounded type parameters (`Catalog<T extends Searchable>`)
- Wildcard generics (`List<? extends Searchable>`)
- Custom runtime annotations (`@Indexed`)
- Anonymous class instantiation
- Static nested classes vs non-static inner classes
- Enum with methods (`Genre.label()`)
- Default interface methods (`Searchable.relevance()`, `SearchResult.isMatch()`)
- Stream API usage (`items.stream().filter(...).toList()`)
