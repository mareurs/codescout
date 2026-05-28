# python-library — Conventions

## Language Patterns

- **Python 3.10+** with `from __future__ import annotations` in files that use
  self-referential or forward-ref type hints (book.py, catalog.py, advanced.py)
- **Stdlib only** — no third-party dependencies; types come from abc, dataclasses,
  enum, typing

## Naming Conventions

- Classes: PascalCase (`Book`, `AudioBook`, `Catalog`, `Genre`)
- Constants: SCREAMING_SNAKE_CASE (`MAX_RESULTS`, `T`)
- Methods/functions: snake_case (`search_text`, `rank_results`, `create_default_catalog`)
- Private attributes: single underscore prefix (`_items`, `_name`)
- Internal helpers: underscore prefix + descriptive name (`_score` closure in rank_results)

## Type Annotation Style

- All public methods annotated with return types
- TypeVar bound: `T = TypeVar("T", bound=Searchable)` for generic constraints
- Type alias at module level: `BookList = list[Book]`
- Protocol for structural typing: `@runtime_checkable class HasISBN(Protocol)`
- No use of `Optional` / `Union` — fixture is intentionally simple

## Class Design Patterns

- Domain models use `@dataclass` (Book, AudioBook inherits it)
- Interfaces use `ABC` + `@abstractmethod` (Searchable)
- Structural interfaces use `Protocol` (HasISBN)
- Enums subclass `Enum` and may have instance methods (Genre.label)
- Mixins are plain classes with no required base (Playable)
- Generic services use `Generic[T]` with TypeVar bound (Catalog)

## Module Organization

- `models/` — pure data, no service dependencies
- `interfaces/` — ABCs and Protocols, no model imports
- `services/` — depend on interfaces only, not models directly
- `extensions/` — depends on both models and interfaces; houses edge-case language features
- `__init__.py` files are minimal: top-level re-exports the 4 main public symbols

## Testing Approach

- No test files within this fixture — it IS the test fixture for codescout
- Designed to exercise codescout's Python LSP/symbol/navigation tooling
- Each file and class is annotated with comments like `"""Extension: ...`" to
  document which Python language feature it demonstrates

## Docstring Style

- All public classes and methods have one-line docstrings
- Extension/edge-case markers in docstrings: `"""Extension: <feature description>"""`
- No multi-paragraph docstrings; keep minimal for fixture purposes
