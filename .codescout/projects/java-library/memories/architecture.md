# java-library — Architecture

## Package Layout

```
src/main/java/library/
  models/
    Book.java        — Java record: title, isbn, Genre, copiesAvailable
    Genre.java       — Enum: FICTION, NON_FICTION, SCIENCE, HISTORY, BIOGRAPHY
  interfaces/
    Searchable.java  — Interface: searchText() + default relevance()
  services/
    Catalog.java     — Generic class Catalog<T extends Searchable>
  extensions/
    Advanced.java    — @Indexed annotation + BookProcessor class
    Results.java     — SearchResult sealed interface hierarchy
```

## Key Abstractions

### `Book` (record, library.models)
Immutable value type. Fields: title, isbn, genre (Genre), copiesAvailable.
Compact constructor defaults copiesAvailable to 1. `isAvailable()` checks
copiesAvailable > 0. MAX_RESULTS = 100 constant.

### `Genre` (enum, library.models)
Five values. `label()` produces human-readable form by replacing underscores
and title-casing.

### `Searchable` (interface, library.interfaces)
Contract for catalog entries. `searchText()` is abstract. `relevance()` has a
default returning 0.0 — subclasses override for custom ranking.

### `Catalog<T extends Searchable>` (class, library.services)
Generic bounded-type container. Stores items in an ArrayList. `search(String)`
streams and filters by `searchText()` containment. `stats()` returns a
`CatalogStats` (static nested class with totalItems + name). Static factory
`createDefault()` returns `Catalog<Book>` named "Main Library".

### `SearchResult` (sealed interface, library.extensions)
Three permitted record implementations:
- `Found(Book book, double score)` — matched result
- `NotFound(String query)` — empty result
- `Error(String message, int code)` — failure
Default `isMatch()` uses `instanceof Found` pattern matching.

### `BookProcessor` + `@Indexed` (library.extensions)
`@Indexed` is a runtime-retention annotation with a `value()` attribute.
`BookProcessor` demos: annotated methods, anonymous Searchable, wildcard
generics (`List<? extends Searchable>`), static inner class `BatchResult`,
and non-static inner class `ProcessingContext`.

## Data Flow — Catalog Search

1. Create: `Catalog<Book> cat = Catalog.createDefault()`
2. Populate: `cat.add(book)` — appends to ArrayList
3. Query: `cat.search("java")` — stream filter on `book.searchText()`
   (Note: `Book` does NOT implement `Searchable` in this fixture — this
   would fail at compile time. The fixture demos the pattern, not full wiring.)
4. Stats: `cat.stats()` returns `CatalogStats{totalItems, name}`

## Data Flow — SearchResult Dispatch

1. Caller receives `SearchResult` (sealed)
2. Switch/pattern-match on type: `Found`, `NotFound`, `Error`
3. Or use `isMatch()` for a quick boolean check

## Semantic Search Examples

Good queries for this project:
- `semantic_search("book record immutable value", project_id="java-library")`
- `semantic_search("sealed interface pattern matching result type", project_id="java-library")`
- `semantic_search("generic catalog searchable bounded type", project_id="java-library")`
- `semantic_search("annotation retention runtime indexed", project_id="java-library")`
- `semantic_search("enum genre label display", project_id="java-library")`
