# kotlin-library — Architecture

## Module Structure

Single Gradle subproject (`rootProject.name = "kotlin-library"`). No multi-module setup.
All sources live under `src/main/kotlin/library/`, split into four sub-packages:

```
interfaces/   — abstract contracts (Searchable)
models/       — value types (Book, Genre)
services/     — business logic (Catalog<T>, factory/extension fns)
extensions/   — advanced Kotlin idioms (ISBN, LazyBook, SearchResult, BookRegistry)
```

There are no test sources in this fixture.

## Key Abstractions

### `Searchable` (interface)
Defines the contract for catalog entries:
- `searchText(): String` — required; text used for substring filtering
- `relevance(): Double` — optional; default returns 0.0

### `Book` (data class)
Primary domain type. Fields: `title`, `isbn`, `genre` (Genre enum), `copiesAvailable` (default 1).
- `isAvailable()` — copiesAvailable > 0
- `companion object` with `create(title, isbn)` and `fromJson(json)` factory methods

### `Genre` (enum)
Five values: FICTION, NON_FICTION, SCIENCE, HISTORY, BIOGRAPHY.
`label()` formats the name for display (lowercase, capitalize first char).

### `Catalog<T : Searchable>` (generic class)
- Holds a mutable list of `T`
- `add(item)` / `search(query): List<T>` (substring match on `searchText()`)
- `stats()` → nested `CatalogStats(totalItems, name)` data class
- Top-level: `createDefaultCatalog()`, `createNamedCatalog(name, maxItems)`
- Extension: `suspend fun Catalog<T>.searchAsync(query)` (delegates to `search`)
- Extension on Book: `fun Book.toSearchText()` → `"$title ($isbn)"`

### `SearchResult` (sealed class)
Three variants:
- `Found(book: Book, score: Double)` — data class
- `NotFound` — object
- `Error(message: String, code: Int)` — data class
- `isMatch()` — returns `this is Found`

### `BookRegistry` (object / singleton)
Thread-unsafe in-memory map from ISBN → Book.
- `register(book)` / `lookup(isbn): Book?`

### `ISBN` (value class)
`@JvmInline value class ISBN(val value: String)` — wraps a raw string with zero overhead.

### `LazyBook`
Demonstrates `by lazy` delegated properties: `formattedTitle` computed once on first access.

### `createBookWithDefaults`
Demonstrates `.let {}` scope function with `copy()` to build a modified instance.

## Data Flow

### Search path (typical use)
1. Caller creates `Catalog<Book>("name")`
2. Books added via `catalog.add(book)`
3. `catalog.search("kotlin")` → `items.filter { it.searchText().contains("kotlin") }`
   (`Book` doesn't directly implement `Searchable`; `searchText()` would need to be provided
   by an implementing class or extension — the fixture uses `Book.toSearchText()` as a hint)
4. Returns filtered `List<T>`

### Registry lookup path
1. `BookRegistry.register(book)` stores `isbn → book` in the singleton map
2. `BookRegistry.lookup(isbn)` returns `Book?` (null if not found)
3. Caller pattern-matches on `SearchResult` variants for richer error surfacing

## Design Patterns Used

- **Value object:** `data class Book`, `data class CatalogStats`
- **Sealed hierarchy for typed errors:** `SearchResult`
- **Singleton via object:** `BookRegistry`
- **Factory via companion object:** `Book.Companion.create / fromJson`
- **Generic bounded type parameter:** `Catalog<T : Searchable>`
- **Delegated property:** `LazyBook.formattedTitle by lazy`
- **Value/inline class:** `ISBN`
- **Extension functions & coroutines:** `searchAsync`, `toSearchText`

## Useful Semantic Search Queries

```
semantic_search("sealed class result variants", project_id="kotlin-library")
semantic_search("generic catalog bounded type parameter search", project_id="kotlin-library")
semantic_search("companion object factory create", project_id="kotlin-library")
semantic_search("singleton object registry lookup", project_id="kotlin-library")
semantic_search("coroutine suspend extension function", project_id="kotlin-library")
```
