# java-library Architecture

## Module Structure
Single Gradle module (`java-library`). All source lives under:
  `src/main/java/library/`

Four sub-packages:
- `library.models`      — domain value types (`Book` record, `Genre` enum)
- `library.interfaces`  — core abstraction (`Searchable` interface)
- `library.services`    — business logic (`Catalog<T>` generic service class)
- `library.extensions`  — advanced-feature showcase (`Results.java`, `Advanced.java`)

No test sources — this fixture has no `src/test` directory.

## Key Abstractions

### `Searchable` (library.interfaces)
Root interface that all catalog items must implement.
- `String searchText()` — required; returns search-friendly text representation
- `double relevance()` — default method returning 0.0; override for custom ranking

### `Book` (library.models)
Java record — the primary domain entity.
Fields: `String title`, `String isbn`, `Genre genre`, `int copiesAvailable`
Compact constructor `Book(title, isbn, genre)` defaults `copiesAvailable=1`.
Constant `MAX_RESULTS = 100`. Method `isAvailable()` returns `copiesAvailable > 0`.
Does NOT explicitly `implements Searchable` — that wiring is handled at the usage/test level.

### `Genre` (library.models)
Enum with 5 values: FICTION, NON_FICTION, SCIENCE, HISTORY, BIOGRAPHY.
`label()` method humanises the name: replaces underscores, lowercases after first char.

### `Catalog<T extends Searchable>` (library.services)
Generic service class. Internally holds `List<T> items` and a `String name`.
- `add(T item)` — appends to items list
- `search(String query)` — streams items, filters by `item.searchText().contains(query)`, returns `List<T>`
- `stats()` — returns a `CatalogStats` (static nested class with `totalItems`, `name`)
- `createDefault()` — static factory returning `Catalog<Book>` named "Main Library"

### `SearchResult` (library.extensions.Results)
Sealed interface with three permitted record subtypes:
- `Found(Book book, double score)` — successful match with relevance score
- `NotFound(String query)` — query yielded no results
- `Error(String message, int code)` — search error
Default method `isMatch()` uses `instanceof Found` pattern.

### `Indexed` (library.extensions.Advanced)
Custom `@interface` annotation with `@Retention(RetentionPolicy.RUNTIME)`.
Has a single `String value()` element with default `""`.

### `BookProcessor` (library.extensions.Advanced)
Demonstrates advanced Java idioms:
- `@Indexed("isbn")` on `process(Book)` method
- `createAnonymousSearchable()` returning an anonymous `Searchable` implementation
- `processAll(List<? extends Searchable>)` for wildcard generics
- Static nested `BatchResult` (int processed, int failed)
- Non-static inner `ProcessingContext` (String currentBook)

## Data Flow: Normal Search
1. Caller constructs `Catalog<Book>` (or uses `Catalog.createDefault()`)
2. Caller calls `catalog.add(book)` for each `Book` to populate the catalog
3. Caller calls `catalog.search("query")`:
   - Streams `items`
   - Filters via `book.searchText().contains(query)` — `searchText()` must be implemented by `T`
   - Collects to an immutable `List<T>` via `.toList()`
4. Caller receives matching items; empty list = no matches (no exception)

## Data Flow: Result-Type Pattern (SearchResult)
1. Some external search component produces a `SearchResult`
2. Caller uses `instanceof` pattern matching or checks `result.isMatch()`
3. Branches: `Found` → access `book` + `score`; `NotFound` → log `query`; `Error` → surface `message` + `code`
4. Sealed hierarchy guarantees exhaustive switch expressions at compile time (Java 21)

## Design Patterns Observed
- **Record types** for immutable value objects (Book, Found, NotFound, Error)
- **Sealed interfaces** for algebraic-data-type style result modeling
- **Generic bounded type parameter** constrains catalog contents to Searchable items
- **Static factory method** (`createDefault()`) on Catalog
- **Static nested class** (CatalogStats) for value grouping without coupling to outer instance
- **Default interface methods** for opt-in behaviour extensions

## Good semantic_search queries for this project
- `semantic_search("sealed interface result type pattern matching", project_id="java-library")`
- `semantic_search("generic catalog search items", project_id="java-library")`
- `semantic_search("book record domain model isbn genre", project_id="java-library")`
- `semantic_search("annotation retention runtime indexed", project_id="java-library")`
- `semantic_search("wildcard generics extends Searchable", project_id="java-library")`
