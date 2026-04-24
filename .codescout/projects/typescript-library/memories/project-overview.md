# typescript-library — Project Overview

## Purpose
Test fixture for the code-explorer (codescout) project. Provides a realistic
TypeScript library codebase with a variety of language features used to validate
LSP navigation, symbol extraction, and semantic search on TypeScript/JavaScript.

## Domain
A library catalog system: books, genres, search functionality.

## Tech Stack
- Language: TypeScript (strict mode, ES2022 target)
- Module system: CommonJS
- Compiler: tsc with experimentalDecorators + emitDecoratorMetadata enabled
- No runtime dependencies; no test framework (pure fixture)

## Key Files
- `src/index.ts` — public barrel exports; re-exports Book, MAX_RESULTS, Genre,
  Searchable, Catalog, createDefaultCatalog
- `src/models/book.ts` — `Book` class + `MAX_RESULTS` constant
- `src/models/genre.ts` — `Genre` enum + `genreLabel` utility
- `src/interfaces/searchable.ts` — `Searchable` interface
- `src/interfaces/types.ts` — union types, type guards, mapped/conditional types
- `src/services/catalog.ts` — generic `Catalog<T>` class + `CatalogStats`
- `src/extensions/advanced.ts` — overloads, decorators, namespace merging, default export

## No Tests
This fixture has no test files of its own; it is exercised by the host
code-explorer test suite (integration tests, LSP symbol tests).

## Build Config
tsconfig.json: `strict: true`, `outDir: dist`, `rootDir: src`,
`experimentalDecorators: true`, `emitDecoratorMetadata: true`.
