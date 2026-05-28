## Project Overview

**Purpose:** Test fixture for codescout's Kotlin language support. Exercises idiomatic
Kotlin and JVM language features to validate codescout's tree-sitter parsing, symbol
extraction, and navigation tools on Kotlin code.

**Tech Stack:**
- Kotlin 2.1.0 (JVM target)
- Gradle build system (Kotlin DSL — `build.gradle.kts`)
- Single dependency: `kotlin("stdlib")`
- Group `library`, version `0.1.0`

**Structure:** Single Gradle module (`kotlin-library`) with 6 source files under
`src/main/kotlin/library/`. No tests, no README — this is a codescout test fixture,
not a production project.

**Packages:**
- `library.models` — domain entities: `Book` (data class), `Genre` (enum with method)
- `library.interfaces` — contracts: `Searchable` (interface with default `relevance()`)
- `library.services` — business logic: `Catalog<T : Searchable>` (generic service),
  free functions (`createDefaultCatalog`, `createNamedCatalog`), extension functions
- `library.extensions` — advanced features: `ISBN` (value class), `LazyBook` (delegated
  property), `SearchResult` (sealed class), `BookRegistry` (object/singleton),
  `createBookWithDefaults` (scope functions demo)

**Kotlin Features Exercised:**
- Data classes (`Book`, `Found`, `Error`, `CatalogStats`)
- Enum class with member function (`Genre.label()`)
- `@JvmInline` value class (`ISBN`)
- Sealed class hierarchy (`SearchResult` with `Found`, `NotFound`, `Error` variants)
- Object declarations — companion (`Book.Companion`) and top-level singleton (`BookRegistry`)
- Delegated properties (`by lazy` in `LazyBook`)
- Generic class with upper bound (`Catalog<T : Searchable>`)
- Extension functions on generic receiver (`Catalog<T>.searchAsync`, `Book.toSearchText`)
- Suspend (coroutine) extension function (`searchAsync`)
- Scope functions (`let`, `copy` in `createBookWithDefaults`)
- Precondition helpers (`require` in `createNamedCatalog`)
- Nested data class inside a class (`CatalogStats` inside `Catalog`)
- Multi-line KDoc with `@param`/`@return` tags (`createNamedCatalog`)
