# kotlin-library — Conventions

## Language & Style

- **Kotlin 2.1.0** targeting JVM; no Java interop layer beyond `@JvmInline`
- All public types and functions carry KDoc `/** ... */` doc comments
- Expression-body functions preferred for single-expression logic:
  `fun isAvailable(): Boolean = copiesAvailable > 0`
- Named arguments used when constructing data classes with multiple params

## Naming

- Classes/interfaces: `PascalCase` (Book, Catalog, Searchable, SearchResult)
- Functions/properties: `camelCase` (searchText, isAvailable, copiesAvailable)
- Constants / enum entries: `UPPER_SNAKE_CASE` (MAX_RESULTS, FICTION, NON_FICTION)
- Packages: all-lowercase, dot-separated (`library.models`, `library.services`)
- File names match the primary type they declare (`Book.kt`, `Genre.kt`)

## Type Design

- `data class` for value objects (Book, CatalogStats, SearchResult.Found)
- `enum class` with member functions for categorization (Genre.label())
- `sealed class` for exhaustive result/error hierarchies (SearchResult)
- `object` for singletons and companion factories (BookRegistry, Book.Companion)
- `@JvmInline value class` for typed wrappers with no runtime overhead (ISBN)

## Extension Pattern

Advanced.kt and the bottom of Catalog.kt follow a deliberate "extension showcase" pattern:
each declaration is annotated with a `/** Extension: ... */` KDoc explaining what Kotlin
feature it demonstrates. This is intentional test-fixture commentary, not production style.

## Build

- Gradle Kotlin DSL only — no `build.gradle` (Groovy)
- No test source set configured; `./gradlew test` would be a no-op
- `./gradlew build` compiles to `build/` (only `build/reports/` present in repo)

## Error Handling

No exception-based error handling in this fixture. Errors are modelled as
`SearchResult.Error(message, code)` — a value in the sealed hierarchy — following
the Kotlin "railway-oriented" convention of avoiding thrown exceptions for expected failures.

## No Tests

This project is a codescout fixture, not a shipping library. There are no test sources.
All verification happens via codescout's own integration tests in the parent project.
