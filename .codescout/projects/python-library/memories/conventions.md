# python-library — Conventions

## Language & Style

- `from __future__ import annotations` in every module (deferred evaluation of type hints, Python 3.10+)
- Type annotations on all function signatures and class fields
- `@dataclass` for value objects (Book); plain classes for services and ABCs
- Enum over string literals for categorical values (Genre)

## Naming

- Classes: `PascalCase` (`Book`, `AudioBook`, `Catalog`, `Genre`)
- Functions and methods: `snake_case` (`search_text`, `is_available`, `rank_results`)
- Constants: `UPPER_SNAKE_CASE` (`MAX_RESULTS`, `FICTION`, `NON_FICTION`)
- Private attributes: single leading underscore (`_items`, `_name`, `_score`)
- Type variables: single uppercase letter (`T`)

## Module / Package Conventions

- Each subpackage has an `__init__.py` with a module docstring only (no re-exports except top-level)
- Top-level `library/__init__.py` re-exports the four main public symbols: `Book`, `Genre`, `Searchable`, `Catalog`
- `extensions/advanced.py` is explicitly a showcase of advanced Python constructs, not production code

## Interface Design

- Abstractions live in `interfaces/` (ABC and Protocol)
- `Catalog` is generic over `Searchable` — only types implementing `search_text()` are valid items
- `Book` itself is **not** `Searchable`; `AudioBook` is the concrete searchable book type
- `HasISBN` Protocol uses `@runtime_checkable` for `isinstance()` checks

## Testing Approach

- No test files exist inside this fixture
- The fixture is exercised by codescout's own integration/unit tests in the host project
- `pytest` is listed as a build command in project hints but no test suite is present

## Error Handling

- No explicit error handling in this fixture — it is a clean-path demonstration codebase
- Type safety is enforced at the annotation level; no runtime validation or exceptions defined
