# rust-library — Conventions

## Language Patterns

### Naming
- Structs and enums: `PascalCase` (`Book`, `Genre`, `Catalog`, `SearchResult`, `BookIterator`, `BookRef`, `CatalogStats`)
- Traits: `PascalCase` (`Searchable`)
- Functions and methods: `snake_case` (`search_text`, `is_available`, `borrow_title`, `available_titles`, `create_default_catalog`)
- Constants: `SCREAMING_SNAKE_CASE` (`MAX_RESULTS`)
- Modules: `snake_case` (`models`, `traits`, `services`, `extensions`)

### Module Organization
- Each module directory has a `mod.rs` that only re-exports child modules via `pub mod <name>;`
- Implementation lives in individual files (`book.rs`, `catalog.rs`, etc.), not in `mod.rs`
- `src/lib.rs` is the crate root — declares top-level modules, no inline logic

### Structs
- Fields are private by default; public accessors provided as methods (`title()`, `isbn()`, etc.)
- Constructor pattern: associated `new()` function rather than struct literal at call sites
- Owned fields (`String`) in structs; borrow-returning methods return `&str`

### Traits
- Traits use doc comments on each method
- Default implementations used for optional behavior (`relevance()` returns `1.0`)
- Trait impls placed in the trait's own file (`searchable.rs`) rather than in the implementor's file

### Error / Result Modeling
- No `Result`/`?` operator usage in this fixture; errors modeled as an enum variant (`SearchResult::Error(String)`)
- No `thiserror` / `anyhow` dependencies

### Generics
- Trait bounds written inline on `impl` blocks: `impl<T: Searchable> Catalog<T>`
- `impl Trait` used for iterator return types to avoid naming the concrete type

### Lifetimes
- Explicit lifetime annotations used only where inference is insufficient (`borrow_title<'a>`)

## Testing Approach
- No tests exist in this fixture (no `#[cfg(test)]` blocks, no `tests/` directory)
- The codebase exists purely as a navigation/analysis target for codescout tooling tests
- `BookIterator::next()` is intentionally stubbed and annotated as such

## Documentation Style
- All public items have `///` doc comments
- Comments are concise, single-sentence descriptions
- Annotated stubs note their purpose: `"In real code we'd use a different approach; this is for testing symbol discovery"`
