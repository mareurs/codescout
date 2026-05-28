# typescript-library — Conventions

## TypeScript Configuration

- **Strict mode on** (`"strict": true`) — all strict type checks enforced
- **Target:** ES2022 with CommonJS modules
- **Decorators:** `experimentalDecorators: true` + `emitDecoratorMetadata: true`
- **No path aliases** — imports use relative paths only (`../models/book`, `./genre`)

## Naming Conventions

- **Classes:** PascalCase (`Book`, `Catalog`, `BookService`, `DefaultCatalog`)
- **Interfaces:** PascalCase (`Searchable`, `FoundResult`, `BookIndex`)
- **Enums:** PascalCase name, PascalCase members (`Genre.Fiction`, `Genre.NonFiction`)
- **Enum values:** string literals in snake_case (`'fiction'`, `'non_fiction'`)
- **Type aliases:** PascalCase (`SearchResult`, `ReadonlyBook`, `IsAvailable`)
- **Functions:** camelCase (`isFound`, `genreLabel`, `createDefaultCatalog`)
- **Constants:** SCREAMING_SNAKE_CASE (`MAX_RESULTS`)
- **Private fields:** underscore prefix (`_title`, `_isbn`, `_genre`, `_copiesAvailable`)

## Code Patterns

- **Private fields exposed as same-name methods** (not getters/properties): `title(): string`,
  `isbn(): string`, `genre(): Genre`, `isAvailable(): boolean`
- **Constructor injection:** private fields declared via constructor parameter shorthand
- **Type guards:** `isFound(result): result is FoundResult` pattern for narrowing unions
- **Discriminated unions:** `kind` literal field on all result variants
- **Generic constraints:** `T extends Searchable` on `Catalog<T>`
- **Free factory functions:** `createDefaultCatalog()` alongside the class

## Documentation Style

- JSDoc `/** ... */` on every exported symbol
- Advanced/fixture-specific features labeled with `/** Extension: <feature-name>. */`

## Module Organization

- `models/` — domain data (value objects + enums)
- `interfaces/` — protocols (`Searchable`) and shared type utilities (`types.ts`)
- `services/` — business logic classes
- `extensions/` — advanced TypeScript feature examples (NOT part of the public API barrel)
- `index.ts` — re-export barrel for public API only

## Testing Approach

No test files exist in this fixture. This codebase is a test TARGET for codescout's own
tests, exercised from the parent `code-explorer` project's test suite. There is no Jest,
Vitest, or other test runner configured. The `package.json` has no `scripts` field.

## What This Fixture Covers (for codescout test authors)

Symbol types present: class, interface, enum, function, const, type alias, namespace,
method, property, constructor. Feature coverage: overloads, decorators, namespace merging,
default exports, generics, discriminated unions, mapped types, conditional types, index
signatures, type guards. All TypeScript features that a symbol-navigation tool should handle.
