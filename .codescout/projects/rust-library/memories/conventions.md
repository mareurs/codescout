# rust-library — Conventions

## Purpose Note

This is a test fixture, not a production codebase. Every convention here is chosen to maximize coverage of Rust language constructs for the codescout symbol-discovery and retrieval engines — not for production software quality.

## Naming

- Structs: PascalCase (`Book`, `Catalog`, `BookRef`, `BookIterator`, `CatalogStats`)
- Enums: PascalCase with PascalCase variants (`Genre::Fiction`, `SearchResult::Found`)
- Traits: PascalCase (`Searchable`)
- Functions and methods: snake_case (`search_text`, `borrow_title`, `available_titles`, `is_available`)
- Constants: SCREAMING_SNAKE_CASE (`MAX_RESULTS`)
- Type re-exports with alias: `pub use ... as BookGenre` (demonstrates `as` aliasing)

## Module Organization

- Each type lives in its own file within a named module directory
- Each directory has a `mod.rs` that declares sub-modules with `pub mod`
- `lib.rs` re-exports the four core public types for ergonomic access
- Extensions (advanced Rust features) are isolated in `extensions/` to keep core types clean

## Documentation

- Every public type, constant, and function has a `///` doc comment
- Comments frequently note which Rust feature is being demonstrated (e.g., "Extension: lifetime annotations", "Extension: derive macros generate code")

## Testing

- No unit tests inside the library itself (`#[cfg(test)]` blocks are absent)
- Testing is done externally via codescout's retrieval e2e suite (`tests/retrieval_e2e.rs`), gated behind the `retrieval-e2e` feature flag
- The e2e tests exercise: sync-then-query roundtrip, idempotency of sync, file modification detection, and search recall

## Error Handling

- No `Result` or `Option` returns in the core types — kept intentionally simple
- `SearchResult` enum models success/failure/error as explicit variants rather than using `Result<T, E>`
- `BookIterator::next()` returns `Option<Book>` (Iterator contract) but always returns `None` — skeletal implementation

## Derive Usage

- `#[derive(Debug, Clone, PartialEq)]` used on `Genre` (models) and `BookRef` (extensions)
- `Book` does NOT derive — fields are private, accessors are hand-written (deliberate contrast)

## Visibility

- All module-level types are `pub`
- `Book` fields are private (accessed via methods); `BookRef` fields are `pub`; `CatalogStats` fields are `pub`
- `Catalog` internals (`items`, `name`) are private
