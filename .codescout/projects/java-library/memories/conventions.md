# java-library — Conventions

## Language Patterns

- **Records** for immutable value types: `Book` uses `record` syntax with
  compact constructors.
- **Sealed interfaces** for exhaustive type hierarchies: `SearchResult` uses
  `sealed interface` with `permits` + nested `record` implementations.
- **Generics with bounded wildcards**: `Catalog<T extends Searchable>` and
  `List<? extends Searchable>` in `BookProcessor`.
- **Default interface methods**: `Searchable.relevance()` provides a default
  that subclasses may override.
- **Static factory methods**: `Catalog.createDefault()` instead of constructors
  for named construction.
- **Static nested vs inner classes**: `CatalogStats` (static — no outer ref),
  `ProcessingContext` (inner — holds outer ref), demonstrated side-by-side.

## Naming Conventions

- Packages: all lowercase, dot-separated (`library.models`, `library.services`)
- Classes/Interfaces: PascalCase (`BookProcessor`, `SearchResult`)
- Methods: camelCase (`searchText`, `isAvailable`, `createDefault`)
- Constants: UPPER_SNAKE_CASE (`MAX_RESULTS`)
- Enum values: UPPER_SNAKE_CASE (`NON_FICTION`)
- Annotations: PascalCase with `@` (`@Indexed`)

## Javadoc Style

All public types and methods have `/** ... */` Javadoc comments. The fixture
uses the `/** Extension: ... */` prefix pattern to annotate which Java feature
each element demonstrates (e.g. `/** Extension: sealed interface hierarchy. */`).

## Testing

No tests in this fixture (`src/test` directory does not exist). The project
exists purely for symbol-extraction and LSP integration testing of the
codescout tooling.

## Build

Gradle with only the `java` plugin — no Kotlin, no Spring, no JUnit declared.
Java 21 source and target compatibility. No external dependencies beyond JDK.
