# typescript-library — Architecture

## Module Structure

```
src/
  index.ts                     # public re-export barrel
  models/
    book.ts                    # Book class + MAX_RESULTS constant
    genre.ts                   # Genre enum + genreLabel() helper
  interfaces/
    searchable.ts              # Searchable interface (search protocol)
    types.ts                   # Union result types + type utilities
  services/
    catalog.ts                 # Catalog<T> generic service + CatalogStats
  extensions/
    advanced.ts                # Advanced TS feature demos (overloads, decorators, namespaces)
```

## Key Abstractions

### `Searchable` interface (`interfaces/searchable.ts`)
Protocol for items that can live in a `Catalog`. Two methods:
- `searchText(): string` — required; returns text to filter against
- `relevance?(): number` — optional; score (default 0)

NOTE: `Book` does NOT implement `Searchable` in this fixture — the interface is defined
but no concrete class wires up to `Catalog<Book>` within the fixture source. The generic
constraint exists for structural completeness.

### `Book` class (`models/book.ts`)
Core domain entity. Constructor-injected private fields (title, isbn, genre,
copiesAvailable). All fields exposed via same-name methods (no public fields). Has
`isAvailable(): boolean` checking `_copiesAvailable > 0`. Not plugged into `Searchable`.

### `Genre` enum (`models/genre.ts`)
String enum: `Fiction='fiction'`, `NonFiction='non_fiction'`, `Science='science'`,
`History='history'`, `Biography='biography'`. Plus free function `genreLabel()` that
calls `genre.replace('_', ' ')` for display.

### `Catalog<T extends Searchable>` (`services/catalog.ts`)
Generic catalog with:
- `add(item: T): void` — appends to private `items: T[]`
- `search(query: string): T[]` — filters by `item.searchText().includes(query)`
- `stats(): CatalogStats` — returns a `CatalogStats` value object

`CatalogStats` is a simple data class (totalItems, name).
`createDefaultCatalog()` is a free function returning `Catalog<any>` named 'Main Library'.

### SearchResult union types (`interfaces/types.ts`)
Discriminated union via literal `kind` field:
- `FoundResult` — `kind:'found'`, `book: Book`, `score: number`
- `NotFoundResult` — `kind:'not_found'`, `query: string`
- `ErrorResult` — `kind:'error'`, `message: string`, `code: number`
- `SearchResult = FoundResult | NotFoundResult | ErrorResult` (union alias)
- `isFound(result): result is FoundResult` — type guard (checks `kind === 'found'`)

Advanced type utilities also in this file:
- `ReadonlyBook = Readonly<Pick<Book, 'title' | 'isbn'>>` — mapped/utility type
- `IsAvailable<T>` — conditional type (`T extends { isAvailable(): boolean } ? true : false`)
- `BookIndex` — index signature interface (`[isbn: string]: Book`)

### `extensions/advanced.ts` — TypeScript Feature Showcase
Each export is labeled "Extension:" in its JSDoc to signal fixture intent:
- `findBook` — function overload signatures (two call signatures + implementation)
- `logged` — method decorator (experimental; just returns descriptor)
- `BookService` — class with `@logged`-decorated `process(book): void` method
- `BookMetadata` — interface + namespace declaration merging (same name, different kinds)
- `DefaultCatalog` — `export default class` (default export pattern)

## Design Patterns

- **Generic service with constraint:** `Catalog<T extends Searchable>` — pluggable catalog
- **Discriminated union + type guard:** `SearchResult` + `isFound()` — exhaustive narrowing
- **Namespace/interface merging:** `BookMetadata` interface + `BookMetadata` namespace coexist
- **Constructor private injection:** `Book` fields are all `private` and exposed via getter methods
- **Barrel re-exports:** `index.ts` is a clean public API surface; internal modules are additive

## Import Graph

```
advanced.ts  →  models/book.ts
types.ts     →  models/book.ts
book.ts      →  models/genre.ts
catalog.ts   →  interfaces/searchable.ts
index.ts     →  models/book, models/genre, interfaces/searchable, services/catalog
```

`extensions/advanced.ts` and `interfaces/types.ts` are NOT re-exported from `index.ts`.

## Good Semantic Search Queries (when index is built)

```
semantic_search("Searchable interface protocol", project_id="typescript-library")
semantic_search("discriminated union type guard", project_id="typescript-library")
semantic_search("generic catalog search items", project_id="typescript-library")
semantic_search("function overload signatures", project_id="typescript-library")
semantic_search("decorator experimental method", project_id="typescript-library")
```

Note: semantic index may not be pre-built; fall back to `grep` or `symbols` if results are empty.
