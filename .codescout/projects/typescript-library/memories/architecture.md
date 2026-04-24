# typescript-library — Architecture

## Module Structure

### models/book.ts
- `Book` class — constructor takes `(title, isbn, genre, copiesAvailable=1)` as private fields.
  Accessors: `title()`, `isbn()`, `genre()`, `isAvailable()`. JSDoc-annotated.
- `MAX_RESULTS` — exported numeric constant (value: implicit from file context).

### models/genre.ts
- `Genre` enum — `Fiction | NonFiction | Science | History | Biography`
- `genreLabel(genre)` — replaces underscores with spaces for display

### interfaces/searchable.ts
- `Searchable` interface — `searchText(): string` (required), `relevance?(): number` (optional)
- Used as the generic constraint on `Catalog<T extends Searchable>`

### interfaces/types.ts
A collection of TypeScript type-system extension showcases:
- `SearchResult` — union type alias: `FoundResult | NotFoundResult | ErrorResult`
- `FoundResult` — `{ kind: 'found', book: Book, score: number }`
- `NotFoundResult` — `{ kind: 'not_found', query: string }`
- `ErrorResult` — `{ kind: 'error', message: string, code: number }`
- `isFound(result)` — type guard: `result is FoundResult` (checks `result.kind === 'found'`)
- `ReadonlyBook` — mapped type: `Readonly<Pick<Book, 'title' | 'isbn'>>`
- `IsAvailable<T>` — conditional type: `T extends { isAvailable(): boolean } ? true : false`
- `BookIndex` — index signature interface: `{ [isbn: string]: Book }`

### services/catalog.ts
- `CatalogStats` — plain value class: `{ totalItems: number, name: string }`
- `Catalog<T extends Searchable>` — generic class; holds `items: T[]`; methods:
  - `add(item)` — push to items
  - `search(query)` — filter by `item.searchText().includes(query)`
  - `stats()` — returns `new CatalogStats(items.length, name)`
- `createDefaultCatalog()` — free function returning `new Catalog('Main Library')`

### extensions/advanced.ts
Advanced TypeScript constructs specifically for LSP edge-case testing:
- `findBook` — function overloads: `(isbn) => Book|undefined` and `(title, author) => Book[]`
- `logged` — method decorator (stub: returns descriptor unchanged)
- `BookService` — class with `@logged`-decorated `process(book)` method
- `BookMetadata` — interface + namespace merging: interface `{ title, pages }` merged with
  namespace providing `BookMetadata.create(title, pages)`
- `DefaultCatalog` — default-exported class with `readonly name = 'default'`

## Data Flow

### Search path (typical)
1. Caller creates `Catalog<Book>('My Library')`
2. Adds books via `catalog.add(book)`
3. Calls `catalog.search(query)` — dispatches `book.searchText().includes(query)` per item
4. Receives `Book[]` filtered result
5. Caller may call `isFound(result)` on a wrapped `SearchResult` for type-narrowed access

### Type-guard narrowing path
1. An operation returns `SearchResult` (union of FoundResult | NotFoundResult | ErrorResult)
2. Caller invokes `isFound(result)` — narrows to `FoundResult`
3. Accesses `result.book` and `result.score` safely

## Import Graph
```
index.ts → book.ts, genre.ts, searchable.ts, catalog.ts
catalog.ts → searchable.ts
book.ts → genre.ts
types.ts → book.ts
advanced.ts → book.ts
```
No circular imports.

## Good semantic_search queries (when index is built)
- `"type guard narrowing FoundResult"` — finds isFound in types.ts
- `"generic catalog searchable constraint"` — finds Catalog class
- `"decorator method logged"` — finds logged + BookService in advanced.ts
- `"namespace interface merging BookMetadata"` — finds advanced.ts namespace
- `"union discriminated kind field"` — finds SearchResult family in types.ts
