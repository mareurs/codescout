# kotlin-library — Architecture

## Module Structure
Single Gradle module (`kotlin-library`). All source under `src/main/kotlin/library/`:

```
library/
  interfaces/   Searchable          — search contract
  models/       Book, Genre         — core domain types
  services/     Catalog<T>          — primary collection type + factory fns
  extensions/   SearchResult        — result ADT
                BookRegistry        — singleton registry
                ISBN, LazyBook      — advanced language demos
```

## Key Abstractions

### `Searchable` (interface)
Implemented by any type that participates in catalog search. Two methods:
- `searchText(): String` — required; returns the text the catalog filters on
- `relevance(): Double` — optional, defaults to 0.0

### `Book` (data class, implements nothing directly)
Core domain entity. Fields: `title`, `isbn`, `Genre`, `copiesAvailable` (default 1).
- `isAvailable()` — `copiesAvailable > 0`
- Companion object: `create(title, isbn)` and `fromJson(json)` factory methods
- `Book.toSearchText()` extension in `Catalog.kt` wraps `"$title ($isbn)"`

### `Catalog<T : Searchable>` (generic class)
Primary collection. Holds `mutableListOf<T>()` internally (private).
- `add(item)` — append
- `search(query)` — `filter { it.searchText().contains(query) }`
- `stats()` — returns nested `CatalogStats(totalItems, name)`
- `searchAsync(query)` — suspend extension delegating to `search()`
Factory functions at file level: `createDefaultCatalog()`, `createNamedCatalog(name, maxItems)`

### `SearchResult` (sealed class)
Three variants: `Found(book, score)`, `NotFound` (object), `Error(message, code)`.
`isMatch()` checks `this is Found`. No connection to `Catalog.search()` in this fixture —
`SearchResult` exists as a type-system demonstration, not wired to the search pipeline.

### `BookRegistry` (object / singleton)
`MutableMap<String, Book>` keyed by ISBN. `register(book)` and `lookup(isbn): Book?`.
Independent of `Catalog` — a parallel registry pattern.

## Data Flows

### Search flow
`Catalog.add(book)` → internal list → `Catalog.search(query)` →
`items.filter { it.searchText().contains(query) }` → `List<T>`

The caller is responsible for making `T` implement `Searchable`. `Book` is not declared to
implement `Searchable` in the fixture — `Book.toSearchText()` is an extension, not an
interface conformance. To put a Book in a `Catalog<Book>`, Book would need to implement
Searchable; this fixture demonstrates the pattern without closing the loop.

### Registry flow
`BookRegistry.register(book)` → `books[book.isbn] = book` →
`BookRegistry.lookup(isbn)` → `books[isbn]` → `Book?`

## Design Patterns Demonstrated
- Generic bounded type parameter: `Catalog<T : Searchable>`
- Sealed class as ADT: `SearchResult`
- Companion object factory: `Book.Companion`
- Singleton object: `BookRegistry`
- Inline/value class: `@JvmInline value class ISBN`
- Delegated property: `val formattedTitle by lazy { ... }`
- Scope function: `.let { }` in `createBookWithDefaults()`
- Suspend extension function: `Catalog<T>.searchAsync()`
- `require()` precondition: `createNamedCatalog(name, maxItems)`
- KDoc with `@param`/`@return` (multi-line): `createNamedCatalog` — used for LSP hover tests

## Good Semantic Search Queries
- `semantic_search("Catalog search implementation generic type", project_id="kotlin-library")`
- `semantic_search("sealed class result type error handling", project_id="kotlin-library")`
- `semantic_search("singleton object registry book lookup", project_id="kotlin-library")`
- `semantic_search("delegated property lazy scope function", project_id="kotlin-library")`
- `semantic_search("extension function coroutine suspend", project_id="kotlin-library")`
