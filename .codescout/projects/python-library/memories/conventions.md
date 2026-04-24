# python-library — Conventions

## Language Patterns

- **Python 3.10+** features used throughout; `from __future__ import annotations` in all
  non-trivial files to enable forward references without quotes.
- **Dataclasses** preferred over plain classes for value objects (`Book`).
- **Enums** for categorical domain values (`Genre`).
- **ABCs** (abstract base classes) for nominal interfaces (`Searchable`).
- **Protocols** with `@runtime_checkable` for structural/duck-type interfaces (`HasISBN`).
- **Generics** via `TypeVar` + `Generic[T]` with `bound=` constraint (`Catalog[T]`).
- **Type aliases** as module-level variables: `BookList = list[Book]`.

## Naming Conventions

- Classes: `PascalCase` — `Book`, `Genre`, `Catalog`, `Searchable`, `AudioBook`
- Functions/methods: `snake_case` — `search_text`, `rank_results`, `create_default_catalog`
- Private attributes: single underscore prefix — `_items`, `_name`, `_score` (closure)
- Constants: `UPPER_SNAKE_CASE` — `MAX_RESULTS`, enum values `FICTION`, `NON_FICTION`
- TypeVars: single uppercase letter — `T`

## Module Organization

- `models/` — pure data types (no service dependencies)
- `interfaces/` — abstract contracts, no concrete logic
- `services/` — business logic; depends on models + interfaces
- `extensions/` — advanced/edge-case features; depends on models + interfaces
- `__init__.py` at package root re-exports the four core public symbols

## Testing

No test files exist in this fixture. It is consumed by codescout's own test suite
(in the parent `tests/` directory) for symbol navigation, LSP, and semantic search
regression tests. The fixture demonstrates Python parser edge cases, not application behavior.

## Import Style

- Absolute imports throughout: `from library.models.book import Book`
- No relative imports used
- Minimal stdlib imports; no third-party dependencies

## Documentation

- Module-level docstrings in every `__init__.py`
- Class-level docstrings on all public classes
- Method docstrings on public/abstract methods
- Module-level constant docstring via string literal after assignment (`MAX_RESULTS`)
- Comments in `extensions/advanced.py` note which language feature each construct exercises
