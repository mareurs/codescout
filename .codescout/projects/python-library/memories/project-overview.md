# python-library — Project Overview

## Purpose

A small Python library management system serving as a **test fixture** for codescout's
Python symbol navigation, LSP, and semantic-search capabilities. It is intentionally
designed to exercise diverse Python language features: dataclasses, enums, ABCs,
Protocols, generics, mixins, multiple inheritance, type aliases, *args/**kwargs, and
nested functions/closures.

## Location

`tests/fixtures/python-library/` within the code-explorer workspace.

## Tech Stack

- **Language:** Python ≥ 3.10
- **Build/Manifest:** `pyproject.toml` (minimal; no external dependencies)
- **No test suite** — the fixture itself is exercised by codescout's own integration tests

## Package Structure

```
library/               # top-level package
  __init__.py          # re-exports: Book, Genre, Searchable, Catalog
  models/
    book.py            # Book dataclass + MAX_RESULTS constant
    genre.py           # Genre enum
  interfaces/
    searchable.py      # Searchable ABC + HasISBN Protocol
  services/
    catalog.py         # Catalog[T] generic + create_default_catalog()
  extensions/
    advanced.py        # Playable mixin, AudioBook, search_books, rank_results, BookList type alias
```

## Key Dependencies (stdlib only)

- `abc` — ABC, abstractmethod
- `dataclasses` — dataclass, field
- `enum` — Enum
- `typing` — Generic, TypeVar, Protocol, runtime_checkable, Any
