# Architecture

## Module Structure
Six source files across four packages under `src/main/java/library/`:

| Package | File | Key Type |
|---|---|---|
| `library.interfaces` | `Searchable.java` | `Searchable` interface |
| `library.models` | `Book.java`, `Genre.java` | `Book` record, `Genre` enum |
| `library.services` | `Catalog.java` | `Catalog<T extends Searchable>` generic class |
| `library.extensions` | `Results.java`, `Advanced.java` | `SearchResult` sealed interface, `BookProcessor`, `@Indexed` annotation |

## Key Abstractions

### `Searchable` (interface)
Core contract: any type with a `searchText(): String` method can participate in catalog
search. Also provides a `default relevance(): double` (returns 0.0) for optional ranking
override. Acts as the generic bound in `Catalog<T extends Searchable>`.

### `Book` (record)
Immutable value type: `title`, `isbn`, `Genre`, `copiesAvailable`. Has a compact
constructor (defaults copies to 1) and `isAvailable()`. Declares `MAX_RESULTS = 100`
as a static constant.

### `Genre` (enum)
Five values: `FICTION`, `NON_FICTION`, `SCIENCE`, `HISTORY`, `BIOGRAPHY`. Has a `label()`
method producing a human-readable string via `name()` manipulation.

### `Catalog<T extends Searchable>` (generic class, `library.services`)
In-memory store backed by `List<T>`. Operations: `add(T)`, `search(String)` (stream filter
on `item.searchText().contains(query)`, returns `List<T>`), `stats()` (returns static
nested `CatalogStats`). Static factory `createDefault()` returns `Catalog<Book>`.
Contains `CatalogStats` as a static nested class (public final fields, no builder).

### `SearchResult` (sealed interface, `library.extensions`)
Sealed hierarchy with three record variants:
- `Found(Book book, double score)` — successful match with relevance
- `NotFound(String query)` — query returned no results
- `Error(String message, int code)` — search failure
Default method `isMatch()` uses `instanceof Found` pattern match check.

### `BookProcessor` + `@Indexed` (class + annotation, `library.extensions`)
Demonstrates advanced Java features: `@Indexed("isbn")` runtime-retained custom annotation,
anonymous class creation (`createAnonymousSearchable()`), wildcard generics
(`List<? extends Searchable>`), and static vs non-static inner classes (`BatchResult` /
`ProcessingContext`).

## Data Flows

### Search flow
`Catalog.search(query)` → streams `items` → calls `item.searchText()` on each `T` →
filters by `contains(query)` → returns `List<T>`. The search contract is entirely
delegated to `Searchable.searchText()`.

### Result classification flow
A search result is represented as `SearchResult`: callers switch on the sealed hierarchy
(`Found` / `NotFound` / `Error`) using pattern matching or `isMatch()`. `Found` carries
the matched `Book` and a `double score`; error states carry diagnostic info.

## Design Patterns
- **Interface-bounded generics** — `Catalog<T extends Searchable>` for open extensibility
- **Sealed interface hierarchy** — `SearchResult` for exhaustive, type-safe result handling
- **Java records** — `Book`, `Found`, `NotFound`, `Error` as immutable data carriers
- **Static nested class** — `CatalogStats` for cohesive but independent data grouping
- **Static factory** — `Catalog.createDefault()` for canonical construction

## Good `semantic_search` Queries
- `semantic_search("search catalog filter items by text", project_id="java-library")`
- `semantic_search("sealed interface result type pattern matching", project_id="java-library")`
- `semantic_search("generic type bounded wildcard searchable", project_id="java-library")`
- `semantic_search("custom annotation retention runtime", project_id="java-library")`
- `semantic_search("record immutable book data model", project_id="java-library")`
