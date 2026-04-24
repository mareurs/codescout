# kotlin-library — Conventions

## Language Patterns

### Naming
- Classes: PascalCase (`Book`, `Catalog`, `SearchResult`)
- Functions/properties: camelCase (`searchText`, `isAvailable`, `formattedTitle`)
- Constants: SCREAMING_SNAKE_CASE (`MAX_RESULTS`, `FICTION`)
- Packages: lowercase (`library.models`, `library.services`)

### Data Modeling
- Immutable data types: `data class` with `val` fields (all model types)
- Enums for closed sets: `Genre` enum with a `label()` helper
- Sealed classes for typed outcomes: `SearchResult` (Found/NotFound/Error)
- Singletons: `object` declaration (`BookRegistry`, `SearchResult.NotFound`)
- Value semantics with zero overhead: `@JvmInline value class ISBN`

### Kotlin-Specific Idioms
- Default parameter values: `copiesAvailable: Int = 1`, `maxItems: Int = 100`
- Expression bodies: single-expression functions use `= expr` syntax throughout
- `by lazy` for computed-once properties
- Scope functions: `let` used in `createBookWithDefaults` to transform and return
- Extension functions: `Book.toSearchText()` and `Catalog<T>.searchAsync()` add
  behavior without modifying original classes
- Companion objects for factory methods instead of static methods

### Documentation
- All public types and methods have KDoc `/** */` comments
- Comments describe Kotlin feature intent (e.g. "Extension: inline/value class",
  "Extension: delegated property") — this is deliberate for LSP testing purposes

### Build/Project Conventions
- Kotlin DSL for Gradle (`build.gradle.kts`, `settings.gradle.kts`)
- Single-module project, no subprojects
- No test source sets — testing is done externally via codescout's Rust test suite
- Package hierarchy mirrors directory hierarchy under `src/main/kotlin/`

## Testing Notes (from codescout perspective)
- This fixture is referenced by codescout LSP tests to verify symbol navigation on Kotlin
- `createNamedCatalog` has a KDoc `@param`/`@return` block — specifically to test whether
  kotlin-language-server includes KDoc in hover responses
- The variety of Kotlin constructs (sealed, value class, generics, extensions, coroutines)
  ensures broad LSP feature coverage
