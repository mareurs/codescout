# Conventions

## Language & Version
Java 21 — the codebase uses modern Java features deliberately as a navigation test surface:
- **Records** for immutable data (`Book`, `SearchResult.Found`, `SearchResult.NotFound`, `SearchResult.Error`)
- **Sealed interfaces** for closed hierarchies (`SearchResult permits Found, NotFound, Error`)
- **Default methods** on interfaces (`Searchable.relevance()`, `SearchResult.isMatch()`)
- **Pattern matching** (`instanceof Found` in `isMatch()`)
- **Streams + `toList()`** (Java 16+) in `Catalog.search`

## Naming
- Classes/interfaces/enums: `UpperCamelCase` — `Book`, `Catalog`, `SearchResult`, `BookProcessor`
- Enum constants: `SCREAMING_SNAKE_CASE` — `FICTION`, `NON_FICTION`
- Fields/methods: `lowerCamelCase` — `searchText()`, `isAvailable()`, `copiesAvailable`
- Packages: all lowercase, dot-separated — `library.models`, `library.services`
- Static constants: `SCREAMING_SNAKE_CASE` — `MAX_RESULTS`

## Documentation Style
Every public type and method has a Javadoc comment. Extension/demo features are explicitly
labeled: `/** Extension: sealed interface hierarchy. */`, `/** Extension: custom annotation. */`.
This labeling convention signals which constructs are present specifically to exercise LSP
edge cases.

## Structural Patterns
- Static nested classes for closely related data (`CatalogStats` inside `Catalog`)
- Non-static inner class for context-bearing helpers (`ProcessingContext` inside `BookProcessor`)
- Static factory methods preferred over raw constructors for named construction (`createDefault()`)
- Compact constructors in records for defaulting fields (`Book(String, String, Genre)`)

## Build
Plain Gradle `java` plugin, no wrapper, no test source set. The project intentionally has
no external dependencies — every type is defined in-project or from the JDK.

## Testing
No unit tests in this fixture. It is consumed by codescout's own integration tests
(in `tests/`) which navigate its symbols via LSP and tree-sitter.
