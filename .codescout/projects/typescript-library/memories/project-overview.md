# Project Overview — typescript-library

## Purpose
A TypeScript fixture library that models a book catalog system. It serves as a test fixture
for the codescout code intelligence engine, covering a wide range of TypeScript language
features: generics, interfaces, enums, decorators, function overloads, discriminated unions,
type guards, mapped types, conditional types, namespace merging, and index signatures.

## Tech Stack
- **Language:** TypeScript (target ES2022, CommonJS modules)
- **Compiler:** tsc with strict mode, experimentalDecorators, emitDecoratorMetadata
- **Runtime:** Node.js (no runtime dependencies beyond Node built-ins)
- **Build output:** `dist/` directory (rootDir: `src/`)
- **Entry point:** `src/index.ts`

## Key Dependencies
None — this is a zero-dependency fixture project.

## Project Structure
```
src/
  index.ts                  # Public barrel export
  models/
    book.ts                 # Book class + MAX_RESULTS constant
    genre.ts                # Genre enum + genreLabel helper
  interfaces/
    searchable.ts           # Searchable interface
    types.ts                # SearchResult union + type utilities
  services/
    catalog.ts              # Catalog<T> generic class + CatalogStats
  extensions/
    advanced.ts             # Overloads, decorators, namespaces, default export
```

## Runtime Requirements
- TypeScript compiler (tsc) for build
- No runtime framework, test runner, or external deps present
