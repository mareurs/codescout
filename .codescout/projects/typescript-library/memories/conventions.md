# typescript-library — Conventions

## Language Patterns

### Naming
- Classes: PascalCase (`Book`, `Catalog`, `CatalogStats`, `BookService`)
- Interfaces: PascalCase, no `I` prefix (`Searchable`, `FoundResult`, `BookIndex`)
- Enums: PascalCase, string values are snake_case (`Fiction = 'fiction'`, `NonFiction = 'non_fiction'`)
- Constants: SCREAMING_SNAKE_CASE (`MAX_RESULTS`)
- Private fields: underscore-prefixed (`_title`, `_isbn`, `_genre`, `_copiesAvailable`)
- Methods: camelCase (`searchText`, `isAvailable`, `genreLabel`, `createDefaultCatalog`)
- Factory functions: `create*` or `createDefault*` prefix

### TypeScript Features Demonstrated
- `strict: true` — all fields typed, no implicit any
- Private constructor parameters via shorthand (`constructor(private name: string) {}`)
- Discriminated unions with `kind` literal discriminant
- Type guards using `is` predicate return type
- Mapped types (`Readonly<Pick<...>>`)
- Conditional types (`T extends ... ? true : false`)
- Index signatures (`[isbn: string]: Book`)
- Generic bounded type parameters (`<T extends Searchable>`)
- Function overloads (multiple signatures + single implementation)
- Experimental decorators (`@logged` on method)
- Declaration merging (interface + namespace with same name)
- Default export alongside named exports (`export default class DefaultCatalog`)
- Barrel/index re-export pattern

### Module Organization
- One class/concept per file; related utilities in the same file
- Imports are relative (`../models/book`, `../interfaces/searchable`)
- Public API defined solely in `src/index.ts` — consumers import from the barrel

### Docstring Style
- JSDoc-style block comments on exported symbols (`/** ... */`)
- Inline section labels mark TypeScript feature intent (`/** Extension: ... */`)
- No runtime test framework; no test files in the fixture

### No Error Handling at Runtime
This is a fixture, not production code. Functions like `findBook` return `undefined`
and `Catalog.search` silently returns empty arrays — no thrown errors.
