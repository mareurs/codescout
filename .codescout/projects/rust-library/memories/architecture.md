# rust-library — Architecture

## Module Structure

Four top-level modules declared in `src/lib.rs`:

| Module | File(s) | Responsibility |
|---|---|---|
| `models` | `genre.rs`, `book.rs` | Core domain types |
| `traits` | `searchable.rs` | Behavioral abstractions (Rust trait) |
| `services` | `catalog.rs` | Business logic, generic over `Searchable` types |
| `extensions` | `results.rs`, `advanced.rs` | Richer types, iterators, lifetime examples |

## Key Abstractions

### `Book` (src/models/book.rs)
Struct with private fields: `title: String`, `isbn: String`, `genre: Genre`,
`copies_available: u32`. Public accessors: `title()`, `isbn()`, `genre()`,
`is_available()`. Constructor: `Book::new(title, isbn, genre)` sets
`copies_available = 1`.

### `Genre` (src/models/genre.rs)
Enum: `Fiction | NonFiction | Science | History | Biography`.
Derives `Debug, Clone, PartialEq`. Method `label() -> &str` via match.

### `Searchable` (src/traits/searchable.rs)
Trait with:
- Required: `search_text(&self) -> String`
- Default impl: `relevance(&self) -> f64` returns `0.0`

`Book` implements it: `search_text` returns `"title (isbn)"`,
`relevance` returns `1.0` if available else `0.5`.

### `Catalog<T: Searchable>` (src/services/catalog.rs)
Generic struct over any `Searchable` type. Methods:
- `new(name) -> Self`
- `add(&mut self, item: T)`
- `search(&self, query: &str) -> Vec<&T>` — filters by `search_text().contains(query)`
- `stats(&self) -> CatalogStats` — returns `total_items` + `name`

Free function `create_default_catalog() -> Catalog<Book>` for convenience.

### `SearchResult` (src/extensions/results.rs)
Rich enum with struct and tuple variants:
- `Found { book: Book, score: f64 }`
- `NotFound(String)` — query string
- `Error { message: String, code: u32 }`
Method `is_match()` uses `matches!` macro.

### Extensions (src/extensions/advanced.rs)
- `BookRef { title: String, available: bool }` — derives `Debug, Clone, PartialEq`
- `borrow_title<'a>(book: &'a Book) -> &'a str` — explicit lifetime annotation
- `available_titles(books: &[Book]) -> impl Iterator<Item = &str>` — `impl Trait` return
- `pub use crate::models::genre::Genre as BookGenre` — re-export alias

### `BookIterator` (src/extensions/results.rs)
Struct implementing `Iterator<Item = Book>` with manual `next()` and internal
`index: usize`. (Stub impl for symbol-discovery testing.)

## Data Flow

### Search path
1. Caller holds `Catalog<Book>`
2. Calls `catalog.search("query")`
3. `search` iterates `self.items`, calls `item.search_text()` (dispatches via `Searchable` vtable-free monomorphization)
4. Filters by `contains(query)`, collects `Vec<&Book>`

### Available-titles path (functional / `impl Trait`)
1. Caller passes `&[Book]` to `available_titles(books)`
2. Returns lazy `impl Iterator<Item = &str>` chaining `.filter(is_available).map(title)`
3. Caller drives the iterator; lifetimes tie `&str` slices to the input slice

## Design Patterns Used

- Trait-bound generics (`Catalog<T: Searchable>`) — monomorphized, zero overhead
- Default method implementations in traits (`relevance`)
- `impl Trait` in return position for opaque iterator types
- Explicit lifetime annotations to express borrowing relationships
- `matches!` macro for concise enum arm checking
- Derive macros for `Debug`, `Clone`, `PartialEq`
- `pub use` re-exports for ergonomic API surface

## Useful semantic_search queries for this project

- `semantic_search("trait implementation for searchable types", project_id="rust-library")`
- `semantic_search("generic catalog search with trait bound", project_id="rust-library")`
- `semantic_search("lifetime annotations and borrowing", project_id="rust-library")`
- `semantic_search("enum variants pattern matching", project_id="rust-library")`
- `semantic_search("iterator adapter filter map collect", project_id="rust-library")`
