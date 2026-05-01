# Conventions — typescript-library

## Language & Compiler
- TypeScript strict mode (`"strict": true`) — all implicit `any`, nullability, and
  type-safety checks enforced.
- `experimentalDecorators` and `emitDecoratorMetadata` enabled — decorators are used
  in `extensions/advanced.ts`; avoid relying on this in non-extension code.
- Target ES2022, CommonJS modules.

## Naming
- Classes: PascalCase (`Book`, `Catalog`, `CatalogStats`, `BookService`)
- Interfaces: PascalCase (`Searchable`, `FoundResult`, `BookIndex`)
- Enums: PascalCase name, PascalCase members (`Genre.Fiction`, `Genre.NonFiction`)
- Type aliases: PascalCase (`SearchResult`, `ReadonlyBook`, `IsAvailable`)
- Functions: camelCase (`genreLabel`, `isFound`, `createDefaultCatalog`, `findBook`)
- Constants: SCREAMING_SNAKE_CASE (`MAX_RESULTS`)
- Private class fields: underscore-prefixed camelCase (`_title`, `_isbn`, `_genre`)

## Class Pattern
- Constructor parameters use `private _field` naming; public accessor methods are
  plain methods (not getters) returning the field value.
- No public field exposure; all state behind accessor methods.

## Interface / Type Design
- Discriminated unions use a `kind` string literal field for safe narrowing.
- Type guards follow the `function isFoo(x: T): x is FooType` pattern.
- Mapped types (`Readonly`, `Pick`) preferred over manual re-declaration.
- Conditional types used for structural capability checks (`IsAvailable<T>`).

## Module / Export Conventions
- Each domain concept lives in its own file under `models/`, `interfaces/`, `services/`, or `extensions/`.
- `src/index.ts` is a barrel that re-exports only the stable public API; extension types from
  `extensions/advanced.ts` are NOT re-exported (internal/test-only).
- Default exports used only in `extensions/advanced.ts` (`DefaultCatalog`) as an explicit
  demonstration of the feature — elsewhere named exports only.

## Testing
- No test files are present in this fixture project. It is used as a code intelligence test
  target, not a tested application.

## Documentation
- JSDoc `/** ... */` comments on all exported classes, interfaces, and methods.
- Extension-specific patterns are labelled with "Extension: <pattern-name>" in their JSDoc.
