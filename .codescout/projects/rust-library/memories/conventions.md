# rust-library — Conventions

## Language & Patterns

- **Rust edition 2021**, no external dependencies
- All public types use `pub struct` / `pub enum`; fields are private by default
  (e.g. `Book` fields), exposed via `&self` accessor methods
- Constructors named `new()` returning `Self`; free functions named
  `create_default_*` for preconfigured instances
- Trait implementations in the same file as the trait definition
  (`Searchable` + `impl Searchable for Book` both in `searchable.rs`)

## Naming Conventions

- Types: `PascalCase` — `Book`, `Genre`, `Catalog`, `SearchResult`, `BookRef`
- Methods/functions: `snake_case` — `search_text`, `is_available`, `borrow_title`
- Constants: `SCREAMING_SNAKE_CASE` — `MAX_RESULTS`
- Modules: `snake_case` — `models`, `traits`, `services`, `extensions`
- Re-export aliases use `as` to rename: `Genre as BookGenre`

## Documentation

- Every public type and method has a `///` doc comment
- Comments on extensions explicitly label the Rust feature being demonstrated
  (e.g., `/// Extension: lifetime annotations.`, `/// Extension: derive macros`)

## Testing Approach

- No in-fixture tests; this library exists as a test target for the parent
  codescout crate
- The parent suite tests symbol discovery, LSP navigation, and semantic search
  against this fixture's symbols

## Error / Result Handling

- No `Result`/`Error` types in the fixture; `SearchResult::Error` variant is a
  domain error representation, not a Rust `std::error::Error` impl
- The `is_match()` method uses `matches!` macro as idiomatic boolean check on enum

## Rust Features Explicitly Exercised (for codescout testing)

| Feature | Location |
|---|---|
| Trait with default method | `traits/searchable.rs` — `relevance()` |
| Generic struct with trait bound | `services/catalog.rs` — `Catalog<T: Searchable>` |
| `impl Trait` return type | `extensions/advanced.rs` — `available_titles` |
| Explicit lifetime annotation | `extensions/advanced.rs` — `borrow_title<'a>` |
| Enum with struct/tuple variants | `extensions/results.rs` — `SearchResult` |
| Custom `Iterator` impl + associated type | `extensions/results.rs` — `BookIterator` |
| Derive macros | `models/genre.rs`, `extensions/advanced.rs` |
| `pub use` re-export alias | `extensions/advanced.rs`, `lib.rs` |
| `matches!` macro | `extensions/results.rs` — `is_match()` |
