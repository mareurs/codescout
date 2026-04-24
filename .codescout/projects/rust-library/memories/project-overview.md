# rust-library — Project Overview

## Purpose

A minimal Rust library fixture used by the codescout test suite to exercise
symbol navigation, LSP integration, and semantic search on idiomatic Rust code.
It models a simple book catalog domain (no external users; exists purely as a
test target).

## Tech Stack

- **Language:** Rust (edition 2021)
- **Dependencies:** none (stdlib only)
- **Manifest:** Cargo.toml — name `rust-library`, version `0.1.0`
- **Entry point:** `src/lib.rs`

## Module Layout

```
src/
  lib.rs                   — top-level module declarations + convenience re-exports
  models/
    genre.rs               — Genre enum (Fiction, NonFiction, Science, History, Biography)
    book.rs                — Book struct + impl (title, isbn, genre, copies_available)
  traits/
    searchable.rs          — Searchable trait + impl for Book
  services/
    catalog.rs             — Catalog<T: Searchable> generic service + CatalogStats
  extensions/
    results.rs             — SearchResult enum (Found, NotFound, Error) + BookIterator
    advanced.rs            — BookRef struct, lifetime fns, impl Trait, pub use re-export
```

## Key Re-exports (src/lib.rs)

`Book`, `Genre`, `Searchable`, `Catalog` are re-exported at crate root for
convenience.

## Runtime Requirements

None — pure library, no binary, no external services.

## Notes for codescout testing

- No test modules exist in the fixture itself; tests live in the parent
  codescout crate and target this fixture via workspace.
- The fixture deliberately exercises: structs, enums, traits, generics,
  lifetimes, `impl Trait`, derive macros, `matches!`, `Iterator` impl,
  associated types, and `pub use` re-exports.
