# Architecture

## Package Structure
All source lives under `src/main/java/library/` — no test sources exist.

- `library.models` — core domain types
- `library.interfaces` — shared contracts
- `library.services` — business logic (catalog management)
- `library.extensions` — advanced Java features: sealed interfaces, generics, annotations

## Key Abstractions

| Type | File | Role |
|---|---|---|
| `Searchable` (interface) | `library/interfaces/Searchable.java` | Core contract: `searchText()` + `default relevance()`. The generic bound used everywhere. |
| `Book` (record) | `library/models/Book.java` | Primary domain object: title, isbn, Genre, copiesAvailable. Java 21 record. |
| `Genre` (enum) | `library/models/Genre.java` | FICTION, NON_FICTION, SCIENCE, HISTORY, BIOGRAPHY — each with a `label()` method. |
| `Catalog<T extends Searchable>` | `library/services/Catalog.java` | Generic container: add/search/stats. `search(String)` filters by `searchText().contains(query)`. Has nested static `CatalogStats`. |
| `SearchResult` (sealed interface) | `library/extensions/Results.java` | Java 17+ sealed hierarchy: `Found(Book, double)`, `NotFound(String)`, `Error(String, int)` as records. |
| `BookProcessor` | `library/extensions/Advanced.java` | Demonstrates annotations (`@Indexed`), anonymous classes, wildcard generics, static vs non-static inner classes. |

## Design Patterns
- **Fixture coverage intent:** Each file demonstrates a distinct Java language feature for LSP testing:
  - `Searchable.java` — interface with default method
  - `Book.java` — Java record (compact constructor)
  - `Genre.java` — enum with method
  - `Catalog.java` — generic class with static nested class and static factory
  - `Results.java` — sealed interface with record permits
  - `Advanced.java` — annotations, anonymous classes, bounded wildcards, static/non-static inner classes

## Invariants
| Rule | Why |
|---|---|
| No test sources | This is a pure fixture; test coverage lives in codescout's Rust test suite |
| No external deps | Keeps the fixture self-contained for CI reproducibility |
