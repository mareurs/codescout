# rust-library — Architecture

## Module Structure

```
src/
  lib.rs             — crate root; re-exports Book, Genre, Searchable, Catalog
  models/
    book.rs          — Book struct + impl + MAX_RESULTS const
    genre.rs         — Genre enum + label() impl
    mod.rs           — pub mod book; pub mod genre
  traits/
    searchable.rs    — Searchable trait + impl for Book
    mod.rs           — pub mod searchable
  services/
    catalog.rs       — Catalog<T: Searchable> generic struct + CatalogStats + create_default_catalog()
    mod.rs           — pub mod catalog
  extensions/
    results.rs       — SearchResult enum + BookIterator (Iterator impl)
    advanced.rs      — BookRef (derive macros), borrow_title (lifetime), available_titles (impl Trait), BookGenre re-export
    mod.rs           — pub mod results; pub mod advanced
```

## Key Abstractions

### `Searchable` trait (`traits/searchable.rs`)
The central interface. Required: `search_text() -> String`. Defaulted: `relevance() -> f64` (returns 0.0).
`Book` implements it: `search_text` returns `"<title> (<isbn>)"`, `relevance` returns 1.0 if available, 0.5 otherwise.

### `Catalog<T: Searchable>` (`services/catalog.rs`)
Generic container. Holds `Vec<T>` where `T: Searchable`. Methods: `new(name)`, `add(item)`, `search(query) -> Vec<&T>`, `stats() -> CatalogStats`.
`search()` is a substring match on `search_text()` output — no ranking.

### `Book` (`models/book.rs`)
Core domain type. Fields: `title: String`, `isbn: String`, `genre: Genre`, `copies_available: u32`.
Private fields with public accessors. `is_available()` checks `copies_available > 0`.
`MAX_RESULTS: usize = 100` is a public constant.

### `SearchResult` (`extensions/results.rs`)
Enum with three variants: `Found { book: Book, score: f64 }`, `NotFound(String)`, `Error { message: String, code: u32 }`.
Demonstrates struct variant, tuple variant, and the `matches!` macro in `is_match()`.

### `BookIterator` (`extensions/results.rs`)
Struct wrapping `Vec<Book>` + index. Implements `Iterator<Item = Book>`.
Note: `next()` currently always returns `None` — intentionally skeletal for symbol-discovery testing.

## Data Flows

### Typical Search Flow
1. Caller creates `Catalog::new("Main Library".to_string())`
2. Calls `catalog.add(book)` one or more times
3. Calls `catalog.search("query")` — iterates internal `Vec<T>`, calls `item.search_text()` via the `Searchable` trait, filters by `contains(query)`, returns `Vec<&T>`
4. Caller inspects returned references

### SearchResult / Error Path
1. External code constructs `SearchResult::Found { book, score }` or `SearchResult::NotFound(query_string)` or `SearchResult::Error { message, code }`
2. `is_match()` uses `matches!(self, SearchResult::Found { .. })` for variant discrimination
3. `BookIterator` wraps a `Vec<Book>` — callers call `.next()` via the `Iterator` trait

## Design Patterns Demonstrated

- Trait-bounded generics: `Catalog<T: Searchable>`
- Default trait method implementation: `Searchable::relevance()`
- Derive macros: `#[derive(Debug, Clone, PartialEq)]` on `Genre` and `BookRef`
- Lifetime annotations: `borrow_title<'a>(book: &'a Book) -> &'a str`
- `impl Trait` return type: `available_titles(books: &[Book]) -> impl Iterator<Item = &str>`
- Type alias via re-export: `pub use crate::models::genre::Genre as BookGenre` in extensions
- Associated type: `type Item = Book` in `impl Iterator for BookIterator`
- Convenience re-exports at crate root: `lib.rs` re-exports the four core types

## Semantic Search Examples

```
semantic_search("Searchable trait implementation", project_id="rust-library")
semantic_search("generic catalog search items", project_id="rust-library")
semantic_search("iterator associated type Book", project_id="rust-library")
semantic_search("lifetime annotation borrow", project_id="rust-library")
semantic_search("derive macros struct PartialEq", project_id="rust-library")
```

Note: the semantic index may not be built for this fixture by default — fall back to `grep(pattern, path="tests/fixtures/rust-library/src")` or `symbols(path=...)`.
