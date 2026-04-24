# kotlin-library — Conventions

## Language & Style
- All files use KDoc (`/** ... */`) on every declaration — required for LSP hover tests
- Package names mirror directory structure: `library.models`, `library.interfaces`,
  `library.services`, `library.extensions`
- `val` everywhere; no `var` in the main source
- Named arguments for `Book(...)` construction in scope functions
- Default parameter values used in primary constructors (`copiesAvailable = 1`)
  and free functions (`maxItems: Int = 100`)

## Naming
- Classes/interfaces: PascalCase — `Book`, `Catalog`, `SearchResult`, `BookRegistry`
- Functions/properties: camelCase — `searchText()`, `isAvailable()`, `copiesAvailable`
- Constants: SCREAMING_SNAKE_CASE — `MAX_RESULTS`
- Enum variants: SCREAMING_SNAKE_CASE — `FICTION`, `NON_FICTION`, `SCIENCE`

## Kotlin Feature Coverage (designed for LSP test breadth)
The fixture deliberately demonstrates one of each major Kotlin construct:
- `data class` with companion object (`Book`)
- `enum class` with member function (`Genre`)
- `interface` with default implementation (`Searchable.relevance()`)
- Generic class with bounded type param (`Catalog<T : Searchable>`)
- Nested data class (`Catalog.CatalogStats`)
- Sealed class with data class / object / data class variants (`SearchResult`)
- `object` singleton (`BookRegistry`)
- `@JvmInline value class` (`ISBN`)
- Delegated property (`by lazy`) (`LazyBook.formattedTitle`)
- Suspend extension function (`Catalog<T>.searchAsync()`)
- Scope function (`let`) with lambda receiver
- Top-level free functions (`createDefaultCatalog`, `createNamedCatalog`, `createBookWithDefaults`)
- `require()` precondition with message lambda

## Testing Approach
No unit tests in this fixture. All verification is done by codescout's Rust integration
tests (`tests/`) which spin up kotlin-language-server against this fixture and assert
correct LSP responses (symbol positions, hover content, go-to-definition targets, etc.).

## Build
- Gradle wrapper via `./gradlew`; Kotlin JVM plugin 2.1.0
- `./gradlew build` compiles; `./gradlew test` runs (no tests, exits cleanly)
- No Gradle plugins beyond the core Kotlin JVM plugin
