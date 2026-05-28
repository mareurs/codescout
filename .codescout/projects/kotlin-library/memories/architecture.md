## Architecture

**Module structure:**

```
src/main/kotlin/library/
  models/
    Book.kt          — data class Book(title, isbn, genre, copiesAvailable); companion with factory methods
    Genre.kt         — enum class Genre { FICTION, NON_FICTION, SCIENCE, HISTORY, BIOGRAPHY }; label()
  interfaces/
    Searchable.kt    — interface with searchText(): String + default relevance(): Double = 0.0
  services/
    Catalog.kt       — generic class Catalog<T : Searchable>; add/search/stats; free + extension fns
  extensions/
    Advanced.kt      — ISBN (value class), LazyBook (delegated property), createBookWithDefaults (scope fns)
    Results.kt       — SearchResult (sealed class), BookRegistry (singleton object)
```

**Key abstractions:**

- `Searchable` — interface that all catalog items must implement; `searchText()` is the indexing contract;
  `relevance()` provides default `0.0` score, overridable for custom ranking
- `Catalog<T : Searchable>` — generic, name-bearing container; maintains a mutable internal list; exposes
  `add`, `search` (filter by `searchText().contains`), and `stats` (returns nested `CatalogStats`)
- `Book` — primary domain entity as a data class; immutable by default; companion object provides
  `create` and `fromJson` factory methods
- `SearchResult` — sealed class hierarchy for typed search outcomes: `Found(book, score)`, `NotFound`
  (object), `Error(message, code)`; `isMatch()` checks `is Found`
- `BookRegistry` — top-level singleton (`object`) keyed by ISBN; `register(book)` + `lookup(isbn): Book?`

**Data flows:**

1. **Search flow:**
   `Catalog<T>.search(query)` → `items.filter { it.searchText().contains(query) }` → `List<T>`
   Items must implement `Searchable`; the search is purely string-contains on `searchText()` output.
   Async variant: `suspend fun Catalog<T>.searchAsync(query)` — delegates to the synchronous `search`.

2. **Registry lookup flow:**
   `BookRegistry.register(book)` → `books[book.isbn] = book` (mutable map, ISBN as key)
   `BookRegistry.lookup(isbn)` → `books[isbn]` returning `Book?` (null if absent)

**Design patterns present:**
- Generic service with type-bound (`Catalog<T : Searchable>`) — classic bounded-type pattern
- Sealed class for result types (`SearchResult`) — Kotlin idiom replacing checked exceptions
- Companion object factory (`Book.create`, `Book.fromJson`) — named constructors
- Singleton object (`BookRegistry`) — thread-safe by JVM class-loading semantics
- Delegated property (`LazyBook.formattedTitle by lazy`) — lazy evaluation demo
- Scope function chaining (`let { }.copy(...)`) in `createBookWithDefaults`

**Semantic search examples (project_id="kotlin-library"):**
- `semantic_search("generic catalog search interface", project_id="kotlin-library")`
- `semantic_search("sealed class result type error handling", project_id="kotlin-library")`
- `semantic_search("value class ISBN delegated property lazy", project_id="kotlin-library")`
- `semantic_search("singleton registry companion object factory", project_id="kotlin-library")`
- `semantic_search("suspend extension function coroutine", project_id="kotlin-library")`

Note: semantic index may be empty for this fixture; fall back to
`grep(pattern, path="tests/fixtures/kotlin-library/src")`.
