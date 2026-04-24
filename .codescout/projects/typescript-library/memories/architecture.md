# typescript-library — Architecture

## Module Structure

```
src/
  index.ts              — barrel: re-exports public API
  models/
    book.ts             — Book class, MAX_RESULTS constant
    genre.ts            — Genre enum, genreLabel helper
  interfaces/
    searchable.ts       — Searchable interface (searchText, relevance?)
    types.ts            — SearchResult union, type guards, utility types
  services/
    catalog.ts          — Catalog<T> generic class, CatalogStats, factory fn
  extensions/
    advanced.ts         — overloads, decorators, namespace merging, default export
```

## Key Abstractions

### `Searchable` (interface, searchable.ts)
Contract for catalog-indexable items. Requires `searchText(): string`, optional
`relevance?(): number`. The `Catalog<T>` is constrained to `T extends Searchable`.

### `Book` (class, models/book.ts)
Domain model. Four private fields (`_title`, `_isbn`, `_genre`, `_copiesAvailable`),
accessed via getter methods (`title()`, `isbn()`, `isAvailable()`, `genre()`).
`_copiesAvailable` defaults to 1. Designed to implement `Searchable` (not declared
explicitly in the class — the fixture leaves that as an exercise / LSP test surface).

### `Genre` (enum, models/genre.ts)
String enum: Fiction, NonFiction, Science, History, Biography. `genreLabel` strips
underscores for display.

### `Catalog<T extends Searchable>` (class, services/catalog.ts)
Generic collection with `add(item)`, `search(query)`, and `stats()`. Search filters
by calling `item.searchText().includes(query)` on each stored item. `CatalogStats`
is a simple data class (totalItems, name).

### `SearchResult` union (interfaces/types.ts)
Three-case discriminated union: `FoundResult` (kind='found', book, score),
`NotFoundResult` (kind='not_found', query), `ErrorResult` (kind='error', msg, code).
`isFound()` is a type guard. Also exports mapped types (`ReadonlyBook`), conditional
types (`IsAvailable<T>`), and an index signature interface (`BookIndex`).

### `BookMetadata` + namespace merging (extensions/advanced.ts)
Interface + same-name namespace — TypeScript declaration merging. The namespace adds
a `create()` factory. Also shows `@logged` method decorator and function overloads for
`findBook`.

## Data Flows

### Flow 1: Adding and searching a catalog
1. Instantiate `new Catalog<Book>('Main Library')` (or call `createDefaultCatalog()`)
2. Call `catalog.add(book)` — pushes `book` to internal `items: T[]`
3. Call `catalog.search("query")` — iterates `items`, calls `item.searchText()`,
   filters by `.includes(query)`, returns matching `T[]`
4. Caller receives typed array and iterates or displays results

### Flow 2: Discriminated union result handling
1. A function returns `SearchResult` (union of FoundResult | NotFoundResult | ErrorResult)
2. Caller calls `isFound(result)` — checks `result.kind === 'found'`
3. TypeScript narrows to `FoundResult`; caller accesses `.book` and `.score` safely
4. Otherwise handles `'not_found'` or `'error'` branches with their respective fields

## Design Patterns
- Discriminated union with exhaustive narrowing (SearchResult)
- Generic bounded type parameter (Catalog<T extends Searchable>)
- Barrel re-export pattern (index.ts)
- Declaration merging (BookMetadata interface + namespace)
- Experimental decorators (method logging)
- Function overloads (findBook)

## Good semantic_search Queries
- `semantic_search("generic catalog add search items", project_id="typescript-library")`
- `semantic_search("discriminated union type guard found not found", project_id="typescript-library")`
- `semantic_search("decorator namespace merging declaration", project_id="typescript-library")`
- `semantic_search("book genre enum string values", project_id="typescript-library")`
- `semantic_search("searchable interface searchText contract", project_id="typescript-library")`
