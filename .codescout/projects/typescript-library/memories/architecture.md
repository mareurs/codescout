# Architecture — typescript-library

## Module Structure
- **`models/`** — Domain types: `Book` (class with private fields, accessor methods) and
  `Genre` (string enum with 5 values + `genreLabel` helper).
- **`interfaces/`** — Contracts and type utilities:
  - `searchable.ts`: `Searchable` interface (`searchText(): string`, optional `relevance?(): number`)
  - `types.ts`: discriminated union `SearchResult = FoundResult | NotFoundResult | ErrorResult`,
    type guard `isFound()`, `ReadonlyBook` (mapped type), `IsAvailable<T>` (conditional type),
    `BookIndex` (index signature interface)
- **`services/`** — Runtime logic: generic `Catalog<T extends Searchable>` with `add/search/stats`
  methods; `CatalogStats` value object; `createDefaultCatalog()` factory function.
- **`extensions/`** — Advanced TypeScript patterns: function overloads (`findBook`), experimental
  decorator (`@logged`), `BookService` with decorated method, `BookMetadata` interface + namespace
  merging, `DefaultCatalog` default export class.

## Key Abstractions
1. **`Searchable`** (`src/interfaces/searchable.ts`) — The core constraint used by `Catalog<T>`.
   Any type that implements `searchText(): string` can be stored and searched.
2. **`Book`** (`src/models/book.ts`) — Primary domain model. Holds title, ISBN, genre,
   copiesAvailable. Exposes `isAvailable()` used by the `IsAvailable<T>` conditional type.
3. **`Catalog<T extends Searchable>`** (`src/services/catalog.ts`) — Generic container.
   Filters items via `item.searchText().includes(query)` on search.
4. **`SearchResult`** (`src/interfaces/types.ts`) — Discriminated union (`kind` field) with
   three variants; `isFound()` type guard narrows to `FoundResult`.
5. **`advanced.ts`** (`src/extensions/advanced.ts`) — Showcase of TypeScript extension points:
   overloads, decorators, namespace/interface merging, default exports.

## Data Flow — Catalog Search
1. Create `new Catalog<Book>('name')` (or `createDefaultCatalog()` for defaults)
2. Call `catalog.add(book)` — pushes Book into internal `items: T[]` array
3. Call `catalog.search(query)` — filters via `book.searchText().includes(query)`
   (Note: `Book` does not implement `Searchable` in the current fixture; the generic
   constraint is exercised by design but Book lacks `searchText()` — intentional for tests)
4. Call `catalog.stats()` — returns `CatalogStats { totalItems, name }`

## Data Flow — SearchResult Discriminated Union
1. An operation returns `SearchResult` (union of `FoundResult | NotFoundResult | ErrorResult`)
2. Call `isFound(result)` — type guard returns `result is FoundResult` by checking `result.kind === 'found'`
3. In the `true` branch, TypeScript narrows `result` to `FoundResult` giving access to `.book` and `.score`
4. Other branches handled via `kind === 'not_found'` or `kind === 'error'`

## Design Patterns
- Generic constraint pattern: `class Catalog<T extends Searchable>`
- Discriminated union + type guard for safe variant access
- Declaration merging: `BookMetadata` interface and namespace share the same name
- Decorator pattern (experimental): `@logged` applied to class method
- Factory function: `createDefaultCatalog()` returns pre-configured `Catalog<any>`
- Barrel export: `src/index.ts` re-exports only the public API surface

## Good `semantic_search` Queries for This Project
```
semantic_search("catalog search items filter", project_id="typescript-library")
semantic_search("type guard discriminated union", project_id="typescript-library")
semantic_search("decorator method logging", project_id="typescript-library")
semantic_search("namespace declaration merging", project_id="typescript-library")
semantic_search("generic constraint extends interface", project_id="typescript-library")
```
