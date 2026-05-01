# python-library — Project Overview

## Purpose

A minimal Python library management system used as a fixture for testing
codescout's Python code-intelligence features (symbol navigation, semantic
search, LSP). It is **not** a production application — it exists to exercise
Python-specific language constructs in a realistic domain.

## Tech Stack

- **Language:** Python 3.10+
- **Build/manifest:** `pyproject.toml` (PEP 517/518, no build backend specified)
- **Core stdlib used:** `dataclasses`, `enum`, `abc`, `typing` (Generic, TypeVar, Protocol)
- **No third-party runtime dependencies**
- **No test suite** in the fixture (tests live in the codescout host project)

## Package Layout

```
library/
  __init__.py          # Re-exports: Book, Genre, Searchable, Catalog
  models/
    book.py            # Book dataclass + MAX_RESULTS constant
    genre.py           # Genre enum
  interfaces/
    searchable.py      # Searchable ABC + HasISBN Protocol
  services/
    catalog.py         # Generic Catalog[T] service + create_default_catalog()
  extensions/
    advanced.py        # Advanced Python features: multiple inheritance, type
                       # aliases, *args/**kwargs, nested functions, closures
```

## Key Concepts

- `Book` — immutable-style `@dataclass`; identity by ISBN (`__eq__`, `__hash__`)
- `Genre` — `Enum` with a human-readable `.label()` method
- `Catalog[T: Searchable]` — generic container; searches via `item.search_text()`
- `AudioBook` — extends `Book` + `Playable` mixin (multiple inheritance / MRO demo)
- `MAX_RESULTS = 100` — module-level constant

## Runtime Requirements

- Python ≥ 3.10 (uses `from __future__ import annotations` throughout)
- Run with: `python -m library` or `pytest` (no tests present in fixture)
