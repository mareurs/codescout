# python-library — Architecture

## Module Structure

```
library/
  __init__.py         → public API: Book, Genre, Searchable, Catalog
  models/             → domain data types (no external deps)
  interfaces/         → ABCs and Protocols (ABC, Protocol from stdlib)
  services/           → business logic layer (depends on interfaces)
  extensions/         → edge-case language features (depends on models + interfaces)
```

## Key Abstractions

### `Searchable` (interfaces/searchable.py)
- Abstract base class (ABC) with one abstract method: `search_text() -> str`
- Provides default `relevance() -> float` (returns 0.0)
- Used as TypeVar bound in `Catalog[T]`: `T = TypeVar("T", bound=Searchable)`

### `HasISBN` (interfaces/searchable.py)
- `@runtime_checkable` Protocol — structural typing (isbn property)
- Decoupled from Searchable hierarchy; `isinstance()` checks work at runtime

### `Book` (models/book.py)
- `@dataclass` with fields: title, isbn, genre, copies_available (default=1)
- Identity and hashing by isbn (`__eq__`, `__hash__`)
- `is_available` property: copies_available > 0
- Does NOT extend `Searchable` — cannot be placed in a strictly typed `Catalog[T]`
  without subclassing. Only `AudioBook` implements `search_text()`.

### `Genre` (models/genre.py)
- Python `Enum` with values: FICTION, NON_FICTION, SCIENCE, HISTORY, BIOGRAPHY
- `label()` method converts snake_case value to Title Case for display

### `Catalog[T]` (services/catalog.py)
- `Generic[T]` where `T` is bound to `Searchable`
- Internal storage: `_items: list[T]`, `_name: str`
- `search(query)`: linear scan calling `item.search_text()` on each item
- `stats()`: returns nested `Catalog.Stats(total_items, name)`
- `create_default_catalog()`: module-level factory returning `Catalog("Main Library")`

### `AudioBook` (extensions/advanced.py)
- Multiple inheritance: `AudioBook(Book, Playable)` — MRO covers both
- Adds `narrator: str` field; `search_text()` returns title + narrator string
- Bridges `Book` (data model) and `Searchable` (interface) — can go into `Catalog`

### `Playable` (extensions/advanced.py)
- Simple mixin: `play() -> str` ("Playing..."), `duration_minutes() -> int` (0)
- No base class beyond object; mixed into AudioBook

## Data Flows

### Flow 1: Add and search catalog items
1. `create_default_catalog()` → `Catalog(name="Main Library")`
2. `catalog.add(audiobook)` → `_items.append(audiobook)`
3. `catalog.search("query")` → `[item for item in _items if "query" in item.search_text()]`
   - Each item's `search_text()` is called (AudioBook: returns title + narrator)

### Flow 2: Rank search results
1. `rank_results(books: BookList) -> BookList`
2. Internal `_score(book)` closure: 1.0 if available, 0.5 if not
3. Returns `sorted(books, key=_score, reverse=True)` — available books first
4. `search_books(*terms, **filters)` is a stub returning `[]`

## Design Patterns Demonstrated

- ABC + abstract method (Searchable)
- runtime_checkable Protocol (HasISBN) — structural typing
- Generic class with TypeVar bound (Catalog[T])
- @dataclass with custom __eq__/__hash__ (Book — isbn identity)
- Enum with methods (Genre.label)
- Multiple inheritance + MRO (AudioBook)
- Mixin class (Playable)
- Nested class (Catalog.Stats)
- Nested function / closure (_score inside rank_results)
- Type alias (BookList = list[Book])
- *args + **kwargs (search_books)
- from __future__ import annotations (postponed evaluation in book.py, catalog.py, advanced.py)

## Useful Search Queries

```python
semantic_search("abstract base class interface protocol", project_id="python-library")
semantic_search("generic type parameter TypeVar bound", project_id="python-library")
semantic_search("dataclass isbn identity hashing", project_id="python-library")
semantic_search("multiple inheritance mixin audiobook", project_id="python-library")
semantic_search("catalog search items query", project_id="python-library")
```
