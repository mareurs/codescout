# kotlin-library — Architecture

## Module Structure
```
library/
  interfaces/Searchable.kt     — Searchable interface (searchText, relevance)
  models/Book.kt               — Book data class + companion factory, MAX_RESULTS const
  models/Genre.kt              — Genre enum (FICTION, NON_FICTION, SCIENCE, HISTORY, BIOGRAPHY)
  services/Catalog.kt          — Catalog<T:Searchable> generic class + free functions
  extensions/Results.kt        — SearchResult sealed class, BookRegistry singleton
  extensions/Advanced.kt       — ISBN value class, LazyBook delegated property, factory fn
```

## Key Abstractions

### `Searchable` (interface)
The central interface. Declares `searchText(): String` (abstract) and `relevance(): Double`
(default = 0.0). All catalog items must implement it.

### `Book` (data class, implements nothing directly)
Primary domain type. Fields: `title`, `isbn`, `genre: Genre`, `copiesAvailable: Int = 1`.
Has `isAvailable(): Boolean` and a companion object with `create(...)` and `fromJson(...)` factory methods.
Note: `Book` does NOT explicitly implement `Searchable` — `Catalog<Book>` works via the
extension function `Book.toSearchText()` provided in Catalog.kt; the LSP fixture intentionally
exercises this structural pattern.

### `Catalog<T : Searchable>` (generic class)
Core service. Holds a `mutableListOf<T>`. Methods: `add(item)`, `search(query): List<T>`,
`stats(): CatalogStats`. `search` delegates to `it.searchText().contains(query)`.
Nested: `CatalogStats(totalItems, name)` data class.

### `SearchResult` (sealed class)
Three subtypes: `Found(book, score)`, `NotFound` (singleton object), `Error(message, code)`.
`isMatch(): Boolean = this is Found`. Models operation outcomes without exceptions.

### `BookRegistry` (object / singleton)
Global ISBN→Book map. `register(book)` and `lookup(isbn): Book?`. Demonstrates Kotlin
`object` declaration.

### `ISBN` (`@JvmInline value class`)
Wraps a `String` with zero overhead at runtime. Demonstrates Kotlin value classes.

### `LazyBook`
Demonstrates Kotlin delegated properties: `formattedTitle by lazy { title.uppercase() }`.

## Data Flow: Catalog Search
1. Caller builds a `Book` (via constructor or `Book.create(...)`)
2. `Catalog<Book>` is created via `createDefaultCatalog()` or `createNamedCatalog(name, maxItems)`
3. `catalog.add(book)` appends to internal list
4. `catalog.search(query)` filters by `book.searchText().contains(query)` — relies on
   `Book.toSearchText()` extension (`"$title ($isbn)"`) or a custom `Searchable` implementation
5. Returns `List<T>` directly (no SearchResult wrapping at this layer)

## Data Flow: Registry Lookup
1. `BookRegistry.register(book)` stores by `book.isbn`
2. `BookRegistry.lookup(isbn)` returns `Book?` (null if not found)
3. Callers may wrap the nullable result in `SearchResult.Found`/`NotFound` for typed outcomes

## Design Patterns Demonstrated
- Generic type constraints (`T : Searchable`)
- Sealed class for ADT-style result types
- Kotlin object declaration (singleton)
- Companion object factory methods
- Delegated properties (`by lazy`)
- `@JvmInline value class`
- Coroutine extension: `suspend fun <T : Searchable> Catalog<T>.searchAsync(query)` (thin wrapper)
- Extension functions on existing types (`Book.toSearchText()`)
- Scope functions (`let` in `createBookWithDefaults`)
- KDoc comments throughout

## Good Semantic Search Queries
- `semantic_search("sealed class result handling kotlin-library")`
- `semantic_search("generic catalog searchable interface")`
- `semantic_search("book isbn registry lookup")`
- `semantic_search("value class ISBN inline")`
- `semantic_search("lazy delegated property book title")`
