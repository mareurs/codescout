# rust-library — Architecture

## Module Structure

| Module | File(s) | Role |
|---|---|---|
| `models` | `book.rs`, `genre.rs` | Domain data: `Book` struct, `Genre` enum |
| `traits` | `searchable.rs` | `Searchable` trait definition + `Book` impl |
| `services` | `catalog.rs` | Generic `Catalog<T: Searchable>`, `CatalogStats`, `create_default_catalog()` |
| `extensions` | `results.rs`, `advanced.rs` | Extension patterns: `SearchResult` enum, `BookIterator`, `BookRef`, lifetime fns |

## Key Abstractions

### `Book` (models/book.rs)
Struct with fields: `title: String`, `isbn: String`, `genre: Genre`, `copies_available: u32`.
Methods: `new()`, `title()`, `isbn()`, `is_available()`, `genre()`.
Constant `MAX_RESULTS: usize = 10`.

### `Genre` (models/genre.rs)
Enum with variants: `Fiction`, `NonFiction`, `Science`, `History`, `Biography`.
`label()` method returns a human-readable `&str` via exhaustive match.

### `Searchable` (traits/searchable.rs)
Trait with:
- `search_text(&self) -> String` — required
- `relevance(&self) -> f64` — provided default (returns `1.0`)
`Book` implements it: `search_text` returns `"<title> (<isbn>)"`.

### `Catalog<T: Searchable>` (services/catalog.rs)
Generic struct holding `Vec<T>` and a `name: String`.
Methods: `new(name)`, `add(&mut self, item)`, `search(&self, query) -> Vec<&T>` (substring match via `search_text()`), `stats() -> CatalogStats`.
Free function `create_default_catalog()` returns `Catalog<Book>` named `"Main Library"`.

### `SearchResult` (extensions/results.rs)
Enum variants: `Found(Book)`, `NotFound`, `Error(String)`.
`is_match()` method returns true only for `Found`.

### `BookIterator` (extensions/results.rs)
Struct wrapping `Vec<Book>` with an `index: usize`.
Implements `Iterator<Item = Book>` — `next()` is a stub (always returns `None`; annotated as a test fixture).

### `BookRef` / lifetime functions (extensions/advanced.rs)
`BookRef { title: String, available: bool }` — owned snapshot.
`borrow_title<'a>(book: &'a Book) -> &'a str` — demonstrates explicit lifetime annotation.
`available_titles(books: &[Book]) -> impl Iterator<Item = &str>` — demonstrates `impl Trait` return.

## Data Flows

### Search flow (representative)
1. Caller builds `Book` via `Book::new(title, isbn, genre)`
2. Adds to `Catalog<Book>` via `catalog.add(book)`
3. Calls `catalog.search("rust")` → iterates items, calls `item.search_text()` (delegates to `Book`'s `Searchable` impl), collects refs where substring matches
4. Returns `Vec<&Book>`

### Stats flow (secondary)
1. Caller calls `catalog.stats()`
2. `Catalog::stats()` constructs `CatalogStats { total_items: self.items.len(), name: self.name.clone() }`
3. Returns value type (no heap allocation beyond clone of name)

## Design Patterns Demonstrated
- Generic structs with trait bounds (`Catalog<T: Searchable>`)
- Trait with provided default method (`Searchable::relevance`)
- Enum-based result type (`SearchResult`)
- Custom `Iterator` implementation (`BookIterator`)
- Explicit lifetime annotations (`borrow_title<'a>`)
- `impl Trait` return type (`available_titles`)
- Free factory function (`create_default_catalog`)

## Useful `semantic_search` Queries
- `semantic_search("catalog search books query", project_id="rust-library")`
- `semantic_search("iterator implementation and lifetime references", project_id="rust-library")`
- `semantic_search("error handling and result types", project_id="rust-library")`
- `semantic_search("generic type parameters and trait bounds", project_id="rust-library")`
- `semantic_search("book availability and copies", project_id="rust-library")`
