# python-library — Project Overview

## Purpose

A minimal Python library management system used as a **test fixture** for the codescout
project (code-explorer). It is NOT a standalone application. Its role is to provide a
realistic Python codebase for validating codescout's symbol navigation, semantic search,
LSP integration, and code-intelligence tools against Python source code.

## Tech Stack

- **Language:** Python 3.10+
- **Build system:** pyproject.toml (no build backend specified — pure fixture)
- **Dependencies:** stdlib only (abc, dataclasses, enum, typing)
- **Runtime requirement:** Python >= 3.10

## Package Structure

```
library/
  __init__.py          # Re-exports: Book, Genre, Searchable, Catalog
  models/
    book.py            # Book dataclass + MAX_RESULTS constant
    genre.py           # Genre enum (FICTION, NON_FICTION, SCIENCE, HISTORY, BIOGRAPHY)
  interfaces/
    searchable.py      # Searchable ABC + HasISBN Protocol
  services/
    catalog.py         # Catalog[T] generic class + create_default_catalog()
  extensions/
    advanced.py        # AudioBook, Playable mixin, BookList alias, search_books, rank_results
```

## Key Entities

- `Book` — dataclass with title, isbn, genre, copies_available; identity by isbn
- `Genre` — enum with human-readable labels
- `Catalog[T]` — generic container for Searchable items; supports add/search/stats
- `Searchable` — ABC requiring search_text(); provides default relevance()
- `HasISBN` — runtime-checkable Protocol for structural ISBN typing
- `AudioBook` — extends Book + Playable mixin; implements search_text()

## Design Intent (as fixture)

Designed to cover a wide range of Python language features for LSP/symbol testing:
ABCs, Protocols, generics, dataclasses, enums, multiple inheritance, mixins,
type aliases, *args/**kwargs, nested classes, nested functions, and properties.
