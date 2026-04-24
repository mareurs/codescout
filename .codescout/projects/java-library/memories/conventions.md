# Conventions

## Naming
| Entity | Convention | Example |
|---|---|---|
| Packages | lowercase, dot-separated | `library.models`, `library.services` |
| Classes/Interfaces | PascalCase | `Book`, `Searchable`, `Catalog` |
| Methods | camelCase | `searchText()`, `isAvailable()`, `createDefault()` |
| Constants | UPPER_SNAKE_CASE | `MAX_RESULTS` |
| Enum members | UPPER_SNAKE_CASE | `FICTION`, `NON_FICTION` |

## Patterns
- **Records for data:** `Book` is a Java 21 record with a compact constructor override.
- **Sealed interfaces for ADTs:** `SearchResult` uses `sealed` + `permits` with record variants — pattern-match safe.
- **Default interface methods:** `Searchable.relevance()` returns 0.0 by default; override for custom ranking.
- **Static factory:** `Catalog.createDefault()` — conventional Java factory pattern.
- **Annotations as markers:** `@Indexed` (defined in `Advanced.java`) is a custom annotation used to tag methods.

## Code Quality
- No linting configuration beyond Gradle's default Java compile warnings.
- Java 21 source/target compatibility (`JavaVersion.VERSION_21`).
