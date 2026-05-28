# rust-library — Project Overview

## Purpose

A small, deliberately-structured Rust library that serves as a test fixture for the codescout code intelligence engine. It is NOT a production library. Its primary role is to provide a representative set of Rust language constructs — structs, enums, traits, generics, lifetimes, derive macros, impl Trait, Iterator impls — so that codescout's symbol discovery, semantic search, and retrieval pipelines can be exercised against realistic Rust code.

## Tech Stack

- Language: Rust (edition 2021)
- No runtime dependencies (empty `[dependencies]` in Cargo.toml)
- Crate type: library (`lib.rs` entry point)

## Domain

A minimal library catalog domain: books, genres, a searchable catalog, and search result types. The domain is simple and stable — it exists only to anchor recognizable language patterns.

## Key Constructs Covered

- `Book` — core domain struct with private fields, public accessor methods
- `Genre` — enum with `#[derive(Debug, Clone, PartialEq)]` and a `label()` method
- `Searchable` — trait with one required method (`search_text`) and one defaulted method (`relevance`)
- `Catalog<T: Searchable>` — generic struct with trait-bounded type parameter
- `SearchResult` — enum with struct variants, tuple variants, and unit-like variants
- `BookIterator` — struct implementing the `Iterator` trait (associated type pattern)
- `BookRef` — derive-macro struct in extensions
- `borrow_title` — function with lifetime annotation
- `available_titles` — function with `impl Trait` return type

## Public API Surface (re-exported from lib.rs)

```
rust_library::Book
rust_library::Genre
rust_library::Searchable
rust_library::Catalog
```

## Runtime Requirements

None — no external services, databases, or environment variables needed to build.
