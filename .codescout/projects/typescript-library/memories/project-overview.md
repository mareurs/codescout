# typescript-library — Project Overview

## Purpose

A minimal TypeScript fixture library used as a test target for codescout's navigation,
symbol-indexing, and LSP analysis tools. It is NOT a standalone production library — it
exists to provide a realistic TypeScript codebase with diverse language features for
exercising codescout symbol surveys, semantic search, and cross-reference queries.

## Tech Stack

- **Language:** TypeScript (target ES2022, strict mode, experimentalDecorators enabled)
- **Module system:** CommonJS (`"module": "commonjs"`)
- **Build:** `tsconfig.json` compiles `src/` to `dist/`; no build scripts or test runner defined
- **No external dependencies** — pure TypeScript stdlib types only

## Package Info

- **Name:** `typescript-library` (private)
- **Version:** 0.1.0
- **Entry point:** `src/index.ts`

## Domain

A library book catalog — `Book` entities with `Genre` enums, a generic `Catalog<T>` service,
and discriminated-union search result types. Domain is deliberately simple; the point is
TypeScript language feature coverage, not business logic.

## Public API Surface (`src/index.ts`)

Only four exports are re-exported at the package level:
- `Book`, `MAX_RESULTS` (from models/book)
- `Genre` (from models/genre)
- `Searchable` (from interfaces/searchable)
- `Catalog`, `createDefaultCatalog` (from services/catalog)

The `extensions/` and `interfaces/types.ts` modules are NOT re-exported from index —
they are advanced-feature examples available by direct import only.

## No Tests

This fixture has no test files. Testing codescout against this library is done from
the parent code-explorer project's integration test suite, not from within the fixture.
