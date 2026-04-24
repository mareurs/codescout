# typescript-library — Conventions

## Language & Compiler
- TypeScript strict mode; ES2022 target; CommonJS modules
- `experimentalDecorators` and `emitDecoratorMetadata` enabled (needed for `@logged`)
- No external dependencies — pure TypeScript standard library

## Naming
- Classes: PascalCase (`Book`, `Catalog`, `CatalogStats`, `BookService`)
- Interfaces: PascalCase (`Searchable`, `FoundResult`, `BookIndex`, `BookMetadata`)
- Enums: PascalCase members (`Genre.Fiction`, `Genre.NonFiction`, ...)
- Functions: camelCase (`genreLabel`, `createDefaultCatalog`, `isFound`, `findBook`)
- Type aliases: PascalCase (`SearchResult`, `ReadonlyBook`, `IsAvailable`)
- Private fields: underscore prefix (`_title`, `_isbn`, `_genre`, `_copiesAvailable`)
- Constants: SCREAMING_SNAKE_CASE (`MAX_RESULTS`)

## Design Patterns
- **Discriminated union**: `SearchResult` uses a `kind` literal field for exhaustive narrowing
- **Type guard**: `isFound(result): result is FoundResult` — checks `kind === 'found'`
- **Generic constraint**: `Catalog<T extends Searchable>` — T must implement `searchText()`
- **Namespace merging**: `BookMetadata` interface + `BookMetadata` namespace coexist
- **Function overloads**: `findBook` has two call signatures + one implementation
- **Barrel export**: `src/index.ts` re-exports public API; internal modules import directly
- **Default export**: `DefaultCatalog` in advanced.ts uses `export default class`

## Testing Approach
No unit tests in the fixture. Correctness is validated externally by
`tests/fixtures/typescript-extensions.toml` in the parent codescout project.
Each TOML entry specifies:
- `tool`: which codescout tool to invoke (`get_symbols_overview`, `find_referencing_symbols`)
- `path`: file to target
- `symbol` (for ref tests): symbol to resolve
- `contains_symbols` or `expected_refs_contain`: what the response must include

## File Organization
- `src/models/` — domain entities (Book, Genre)
- `src/interfaces/` — contracts (Searchable) and type compositions (types.ts)
- `src/services/` — service layer (Catalog)
- `src/extensions/` — advanced TypeScript feature showcase (not production patterns)
- No `tests/` directory inside the fixture itself
