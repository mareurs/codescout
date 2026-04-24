# python-library — Architecture

## Module Structure

### `library/models/`
- **`genre.py`** — `Genre(Enum)`: five values (FICTION, NON_FICTION, SCIENCE, HISTORY,
  BIOGRAPHY) plus a `label()` method returning human-readable strings.
- **`book.py`** — `Book` `@dataclass`: fields `title: str`, `isbn: str`, `genre: Genre`,
  `copies_available: int = 1`. Identity is ISBN-based (`__eq__`/`__hash__`).
  `is_available` is a `@property`. `MAX_RESULTS: int = 100` module-level constant.

### `library/interfaces/`
- **`searchable.py`** — Two interface types:
  - `Searchable(ABC)`: abstract `search_text() -> str` + default `relevance() -> float`.
  - `HasISBN(Protocol)` with `@runtime_checkable`: structural typing duck-type check for
    any object that has an `isbn` property.

### `library/services/`
- **`catalog.py`** — `Catalog(Generic[T])` where `T` is bound to `Searchable`.
  Holds `_items: list[T]`, supports `add(item)`, `search(query)` (substring match on
  `item.search_text()`), and `stats() -> Catalog.Stats`. `Catalog.Stats` is a nested
  class tracking `total_items` and `name`. `create_default_catalog()` is a free function
  returning `Catalog(name="Main Library")`.

### `library/extensions/`
- **`advanced.py`** — Exercises advanced Python features for parser/LSP stress-testing:
  - `BookList = list[Book]` — type alias
  - `Playable` — mixin class with `play()` and `duration_minutes()`
  - `AudioBook(Book, Playable)` — multiple inheritance; overrides `search_text()`
  - `search_books(*terms: str, **filters: Any) -> BookList` — variadic signature (stub)
  - `rank_results(books: BookList) -> BookList` — sorts by availability using nested
    closure `_score`

### `library/__init__.py`
Re-exports `Book`, `Genre`, `Searchable`, `Catalog` as the public API.

## Key Abstractions

| Abstraction | Kind | Role |
|---|---|---|
| `Searchable` | ABC | Contract: any catalog item must implement `search_text()` |
| `HasISBN` | Protocol | Structural type: duck-typed isbn check |
| `Book` | dataclass | Core domain entity; identity by ISBN |
| `Genre` | Enum | Categorical attribute of Book |
| `Catalog[T]` | Generic class | Container + search over any `Searchable` type |

## Data Flows

### Flow 1: Searching the catalog
1. `create_default_catalog()` → returns `Catalog(name="Main Library")`
2. `catalog.add(book)` → appends `Book` to `_items`
3. `catalog.search("python")` → list comprehension calls `item.search_text()` on each
   stored item; returns items where query string appears in the search text
4. `catalog.stats()` → returns `Catalog.Stats(total_items=len(_items), name=_name)`

### Flow 2: Ranking results
1. `rank_results(books)` called with a `BookList`
2. Nested closure `_score(book)` returns `1.0` if `book.is_available` else `0.5`
3. `sorted(books, key=_score, reverse=True)` — available books float to top

## Design Patterns Demonstrated

- ABC + abstractmethod (nominal interface)
- `@runtime_checkable` Protocol (structural typing)
- `Generic[T]` with `TypeVar` bound
- `@dataclass` with custom `__eq__`/`__hash__` (ISBN-based identity)
- Nested class (`Catalog.Stats`)
- Mixin inheritance (`Playable` + `Book` → `AudioBook`)
- Nested function / closure (`rank_results` / `_score`)
- Type alias (`BookList`)
- Module-level constant with docstring (`MAX_RESULTS`)

## Useful Search Queries

- `semantic_search("generic catalog searchable items", project_id="python-library")`
- `semantic_search("book availability copies", project_id="python-library")`
- `semantic_search("abstract interface search text", project_id="python-library")`
- `semantic_search("multiple inheritance mixin audiobook", project_id="python-library")`
- `semantic_search("rank sort results by score", project_id="python-library")`
