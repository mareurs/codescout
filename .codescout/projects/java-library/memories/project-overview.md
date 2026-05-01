# java-library

## Purpose
A minimal Java 21 library fixture used as a test target for codescout's LSP and symbol
navigation features. Not a real application — it exercises Java LSP tooling (goto definition,
find references, symbol listing, hover, annotations, generics, sealed interfaces, records)
in the context of codescout's integration tests.

## Tech Stack
- **Language:** Java 21 (`sourceCompatibility = JavaVersion.VERSION_21`)
- **Build:** Gradle (`id 'java'` plugin only; no Gradle wrapper jar checked in)
- **Group/version:** `library:0.1.0`
- **Framework:** None — plain Java library, no external runtime deps
- **Key deps:** JDK 21+ (standard library only)
- **Build command:** `./gradlew build`

## Package Layout
```
library.interfaces   — Searchable (core interface)
library.models       — Book (record), Genre (enum)
library.services     — Catalog<T extends Searchable> (generic service)
library.extensions   — SearchResult (sealed interface), BookProcessor, Indexed (@annotation)
```

## No Tests
The fixture has no `src/test` directory. It exists purely as a navigation target for
codescout's integration tests, not as a tested library itself.
