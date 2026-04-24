# rust-library — Test Fixture

A minimal Rust library crate used by the main code-explorer (codescout) project as a fixture for LSP and symbol-navigation tests. It mirrors a small library-management domain in idiomatic Rust, providing realistic symbols across traits, structs, enums, generics, lifetimes, and iterators.

## Crate info
- Name: `rust-library` (Cargo.toml: edition 2021, no external deps)
- Entry point: `src/lib.rs` — re-exports four modules: `models`, `traits`, `services`, `extensions`

## Module tree

```
src/
  lib.rs                        — 4 pub mod declarations
  models/
    book.rs                     — Book struct + impl
    genre.rs                    — Genre enum + impl
  traits/
    searchable.rs               — Searchable trait + impl for Book
  services/
    catalog.rs                  — Catalog<T> struct, CatalogStats, free fn create_default_catalog
  extensions/
    results.rs                  — SearchResult enum, BookIterator struct + Iterator impl
    advanced.rs                 — BookRef struct, borrow_title fn, available_titles fn
```

## Key types and functions

### models
- `Book` — struct with fields: `title: String`, `isbn: String`, `genre: Genre`, `copies_available: u32`; methods: `new`, `title`, `isbn`, `is_available`, `genre`
- `MAX_RESULTS: usize` — module-level constant (value 10)
- `Genre` — enum: `Fiction | NonFiction | Science | History | Biography`; method: `label(&self) -> &str`

### traits
- `Searchable` — trait with `search_text(&self) -> String` (required) and `relevance(&self) -> f64` (default 1.0)
- `impl Searchable for Book` — `search_text` returns title; `relevance` returns copies_available as f64

### services
- `Catalog<T: Searchable>` — generic struct (`items: Vec<T>`, `name: String`); methods: `new`, `add`, `search(&str) -> Vec<&T>`, `stats() -> CatalogStats`
- `CatalogStats` — struct: `total_items: usize`, `name: String`
- `create_default_catalog() -> Catalog<Book>` — free constructor function

### extensions
- `SearchResult` — enum: `Found(Book) | NotFound | Error(String)`; method: `is_match(&self) -> bool`
- `BookIterator` — struct: `books: Vec<Book>`, `index: usize`; `impl Iterator` with `Item = Book`
- `BookRef` — struct: `title: String`, `available: bool`
- `borrow_title<'a>(book: &'a Book) -> &'a str` — lifetime-annotated free fn
- `available_titles(books: &[Book]) -> impl Iterator<Item = &str>` — returns filtered iterator

## Purpose in the test suite
Used to exercise codescout's Rust LSP (rust-analyzer) integration: symbol listing, goto-definition, find-references, hover, and did_change cache invalidation. The variety of constructs (trait impls, generics, lifetimes, iterator impls, enums with data) gives broad coverage of LSP response shapes.
