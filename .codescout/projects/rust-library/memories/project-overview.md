# rust-library — Project Overview

## Purpose
A fixture project used as a test target for codescout's Rust code intelligence features (symbol navigation, semantic search, LSP integration). It models a small library catalog domain — books, genres, a generic catalog service — with deliberate use of diverse Rust language constructs so codescout tooling can be validated against them.

## Tech Stack
- **Language:** Rust (edition 2021)
- **Crate name:** `rust-library`
- **Version:** 0.1.0
- **Dependencies:** none (no external crates)
- **Build:** standard `cargo build` / `cargo test`

## Key Dependencies
None. The crate is intentionally dependency-free to keep it a clean fixture.

## Module Layout
```
src/
  lib.rs              — crate root; re-exports four top-level modules
  models/             — domain data types (Book, Genre)
  traits/             — Rust trait definitions (Searchable)
  services/           — business logic (Catalog<T>)
  extensions/         — advanced Rust patterns (SearchResult, BookIterator, BookRef, lifetime/iterator demos)
```

## Runtime Requirements
- Rust stable toolchain
- No runtime dependencies or external services
