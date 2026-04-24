# rust-library Architecture

Small single-crate library with no external dependencies. Four top-level modules:

- `models` — plain data types (`Book`, `Genre`)
- `traits` — behavioural abstractions (`Searchable` trait)
- `services` — generic container (`Catalog<T: Searchable>`)
- `extensions` — utilities, iterators, lifetime examples (`SearchResult`, `BookIterator`, `BookRef`, `borrow_title`, `available_titles`)

The domain models a library catalogue: books have genres and availability counts; catalogues hold searchable items; search results carry typed outcomes.

### Design choices relevant to LSP testing
- `Catalog<T>` uses a trait bound — exercises generic symbol resolution
- `BookIterator` implements `std::iter::Iterator` — exercises trait impl navigation
- `borrow_title` uses explicit lifetime parameters — exercises hover/inlay hints
- `SearchResult` enum carries data variants — exercises enum variant goto-definition
- `impl Searchable for Book` is in `traits/searchable.rs`, not `models/book.rs` — cross-file impl navigation
