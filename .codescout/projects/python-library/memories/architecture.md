# python-library — Architecture

## Module Structure

```
library/interfaces/searchable.py   — Abstract base + Protocol
library/models/book.py             — Core domain model
library/models/genre.py            — Enum taxonomy
library/services/catalog.py        — Generic service layer
library/extensions/advanced.py     — Python feature showcase
library/__init__.py                — Public API surface
```

## Key Abstractions

### `Searchable` (ABC) — `library/interfaces/searchable.py`
- Abstract base with `@abstractmethod search_text() -> str`
- Default `relevance() -> float` returns 0.0 (override for custom ranking)
- Separate `HasISBN` is a `@runtime_checkable Protocol` for structural typing

### `Book` (dataclass) — `library/models/book.py`
- Fields: `title: str`, `isbn: str`, `genre: Genre`, `copies_available: int = 1`
- Identity by ISBN via `__eq__` and `__hash__`
- `is_available` property: `copies_available > 0`
- Does **not** extend `Searchable` directly — callers use `AudioBook` for typed catalog use

### `Genre` (Enum) — `library/models/genre.py`
- Values: FICTION, NON_FICTION, SCIENCE, HISTORY, BIOGRAPHY (string values)
- `.label()` method produces human-readable display string

### `Catalog[T: Searchable]` (Generic) — `library/services/catalog.py`
- `T = TypeVar("T", bound=Searchable)` — type-safe container
- `_items: list[T]` internal storage
- `search(query: str) -> list[T]` — linear scan via `item.search_text()`
- Nested `Stats` class holds `total_items` + `name`
- `create_default_catalog()` factory function

### `AudioBook(Book, Playable)` — `library/extensions/advanced.py`
- Multiple inheritance: `Book` (dataclass) + `Playable` mixin
- Implements `search_text()` → `"<title> (narrated by <narrator>)"`
- Satisfies `Searchable` interface; usable as `Catalog[AudioBook]`

## Data Flows

### Flow 1 — Add and search items in catalog
1. Construct `AudioBook(title=..., isbn=..., genre=Genre.FICTION, narrator=...)`
2. `catalog = Catalog(name="Main Library")`  or `create_default_catalog()`
3. `catalog.add(book)` → appends to `_items`
4. `catalog.search("query")` → iterates `_items`, calls `item.search_text()`,
   returns items where query is a substring
5. `catalog.stats()` → returns `Stats(total_items=len(_items), name=_name)`

### Flow 2 — Ranking available books
1. `books: BookList` collected (type alias `list[Book]`)
2. `rank_results(books)` in `extensions/advanced.py`
3. Inner `_score(book)` closure: returns 1.0 if `book.is_available` else 0.5
4. `sorted(books, key=_score, reverse=True)` — available books first

## Design Patterns Demonstrated

- ABC + abstract method (`Searchable`)
- `@runtime_checkable Protocol` (`HasISBN`)
- `@dataclass` with custom equality/hash
- `Generic[T]` service with `TypeVar` bound
- Multiple inheritance + MRO (`AudioBook`)
- Nested class (`Catalog.Stats`)
- Type aliases (`BookList = list[Book]`)
- `*args / **kwargs` signature (`search_books`)
- Nested function / closure (`rank_results / _score`)

## Useful `semantic_search` Queries for This Project

- `semantic_search("abstract interface searchable protocol", project_id="python-library")`
- `semantic_search("generic catalog type parameter search items", project_id="python-library")`
- `semantic_search("multiple inheritance mixin audiobook", project_id="python-library")`
- `semantic_search("dataclass book isbn genre availability", project_id="python-library")`
- `semantic_search("rank sort books by availability score", project_id="python-library")`
