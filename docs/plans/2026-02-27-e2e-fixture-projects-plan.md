# E2E Test Fixture Projects — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Create 5 controlled fixture projects (Rust, Python, TypeScript, Kotlin, Java) with a data-driven TOML test harness that validates code-explorer's tools against known codebases with deterministic assertions.

**Architecture:** Each fixture project implements a "Library" domain (Book, Genre, Searchable, Catalog) using its language's idioms, plus language-specific extensions (sealed classes, traits, decorators, etc.). A Rust test harness reads `core-expectations.toml` and `<lang>-extensions.toml` to run assertions. LSP servers start once per language and are reused across test cases. All gated behind `e2e-*` cargo features.

**Tech Stack:** Rust (test harness with tokio + serde + toml), existing code-explorer Tool trait, LspManager, Agent. Fixture projects: Cargo, pyproject.toml, package.json, build.gradle.kts, build.gradle.

**Design doc:** `docs/plans/2026-02-27-e2e-fixture-projects-design.md`

---

## Task 1: Add E2E feature flags to Cargo.toml

**Files:**
- Modify: `Cargo.toml`

**Step 1: Add e2e feature flags**

Add to the `[features]` section:

```toml
# E2E tests — require real LSP servers installed
e2e = ["e2e-rust", "e2e-python", "e2e-typescript", "e2e-kotlin", "e2e-java"]
e2e-rust = []       # needs: rust-analyzer
e2e-python = []     # needs: pyright-langserver
e2e-typescript = [] # needs: typescript-language-server
e2e-kotlin = []     # needs: kotlin-lsp
e2e-java = []       # needs: jdtls
```

**Step 2: Verify it compiles**

Run: `cargo check --features e2e`
Expected: compiles cleanly (features are empty for now)

**Step 3: Commit**

```bash
git add Cargo.toml
git commit -m "feat(e2e): add feature flags for per-language E2E tests"
```

---

## Task 2: Create the Rust fixture project

**Files:**
- Create: `tests/fixtures/rust-library/Cargo.toml`
- Create: `tests/fixtures/rust-library/src/lib.rs`
- Create: `tests/fixtures/rust-library/src/models/mod.rs`
- Create: `tests/fixtures/rust-library/src/models/book.rs`
- Create: `tests/fixtures/rust-library/src/models/genre.rs`
- Create: `tests/fixtures/rust-library/src/traits/mod.rs`
- Create: `tests/fixtures/rust-library/src/traits/searchable.rs`
- Create: `tests/fixtures/rust-library/src/services/mod.rs`
- Create: `tests/fixtures/rust-library/src/services/catalog.rs`
- Create: `tests/fixtures/rust-library/src/extensions/mod.rs`
- Create: `tests/fixtures/rust-library/src/extensions/results.rs`
- Create: `tests/fixtures/rust-library/src/extensions/advanced.rs`

This is the most detailed fixture since code-explorer is itself a Rust project. The fixture must compile with `cargo check`.

**Step 1: Create Cargo.toml**

```toml
[package]
name = "rust-library"
version = "0.1.0"
edition = "2021"

[dependencies]
```

**Step 2: Create src/lib.rs with module declarations**

```rust
pub mod models;
pub mod traits;
pub mod services;
pub mod extensions;

// Core: re-export for convenience
pub use models::book::Book;
pub use models::genre::Genre;
pub use traits::searchable::Searchable;
pub use services::catalog::Catalog;
```

**Step 3: Create src/models/mod.rs**

```rust
pub mod book;
pub mod genre;
```

**Step 4: Create src/models/book.rs**

Core features: struct, methods (impl block), constants.

```rust
/// A book in the library catalog.
pub struct Book {
    title: String,
    isbn: String,
    genre: super::genre::Genre,
    copies_available: u32,
}

/// Maximum number of search results to return.
pub const MAX_RESULTS: usize = 100;

impl Book {
    /// Create a new book.
    pub fn new(title: String, isbn: String, genre: super::genre::Genre) -> Self {
        Self {
            title,
            isbn,
            genre,
            copies_available: 1,
        }
    }

    /// Get the book title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Get the ISBN.
    pub fn isbn(&self) -> &str {
        &self.isbn
    }

    /// Check if the book is available for borrowing.
    pub fn is_available(&self) -> bool {
        self.copies_available > 0
    }

    /// Get the genre.
    pub fn genre(&self) -> &super::genre::Genre {
        &self.genre
    }
}
```

**Step 5: Create src/models/genre.rs**

Core features: enum with multiple variants.

```rust
/// Genre categories for library books.
#[derive(Debug, Clone, PartialEq)]
pub enum Genre {
    Fiction,
    NonFiction,
    Science,
    History,
    Biography,
}

impl Genre {
    /// Human-readable label for display.
    pub fn label(&self) -> &str {
        match self {
            Genre::Fiction => "Fiction",
            Genre::NonFiction => "Non-Fiction",
            Genre::Science => "Science",
            Genre::History => "History",
            Genre::Biography => "Biography",
        }
    }
}
```

**Step 6: Create src/traits/mod.rs**

```rust
pub mod searchable;
```

**Step 7: Create src/traits/searchable.rs**

Core features: trait, default method. Extension: blanket impl.

```rust
use std::fmt;

/// Interface for anything that can be searched in the catalog.
pub trait Searchable {
    /// Return a search-friendly text representation.
    fn search_text(&self) -> String;

    /// Default relevance score — override for custom ranking.
    fn relevance(&self) -> f64 {
        0.0
    }
}

/// Extension: blanket impl — anything that implements Display is Searchable.
impl<T: fmt::Display> Searchable for T {
    fn search_text(&self) -> String {
        self.to_string()
    }
}
```

NOTE: The blanket impl above would conflict with explicit impls on Book. So we make Book NOT implement Display, and instead implement Searchable directly. Adjust:

Actually, to avoid coherence issues, let's keep the blanket impl separate and NOT also impl Searchable for Book directly. The blanket impl IS the implementation for Book if Book implements Display. Let's restructure to make this work:

```rust
/// Interface for anything that can be searched in the catalog.
pub trait Searchable {
    /// Return a search-friendly text representation.
    fn search_text(&self) -> String;

    /// Default relevance score — override for custom ranking.
    fn relevance(&self) -> f64 {
        0.0
    }
}

// Explicit impl for Book (core: interface implementation)
impl Searchable for crate::models::book::Book {
    fn search_text(&self) -> String {
        format!("{} ({})", self.title(), self.isbn())
    }

    fn relevance(&self) -> f64 {
        if self.is_available() { 1.0 } else { 0.5 }
    }
}
```

**Step 8: Create src/services/mod.rs**

```rust
pub mod catalog;
```

**Step 9: Create src/services/catalog.rs**

Core features: generics, nested type, free functions.

```rust
use crate::traits::searchable::Searchable;

/// A catalog that holds searchable items.
pub struct Catalog<T: Searchable> {
    items: Vec<T>,
    name: String,
}

/// Nested type: statistics about the catalog.
pub struct CatalogStats {
    pub total_items: usize,
    pub name: String,
}

impl<T: Searchable> Catalog<T> {
    /// Create a new empty catalog.
    pub fn new(name: String) -> Self {
        Self {
            items: Vec::new(),
            name,
        }
    }

    /// Add an item to the catalog.
    pub fn add(&mut self, item: T) {
        self.items.push(item);
    }

    /// Search for items matching a query.
    pub fn search(&self, query: &str) -> Vec<&T> {
        self.items
            .iter()
            .filter(|item| item.search_text().contains(query))
            .collect()
    }

    /// Get catalog statistics.
    pub fn stats(&self) -> CatalogStats {
        CatalogStats {
            total_items: self.items.len(),
            name: self.name.clone(),
        }
    }
}

/// Free function: create a default catalog for books.
pub fn create_default_catalog() -> Catalog<crate::models::book::Book> {
    Catalog::new("Main Library".to_string())
}
```

**Step 10: Create src/extensions/mod.rs**

```rust
pub mod results;
pub mod advanced;
```

**Step 11: Create src/extensions/results.rs**

Extension: enum with struct/tuple variants, associated types via Iterator impl.

```rust
use crate::models::book::Book;

/// Search result with structured variants (Rust extension: struct + tuple variants).
pub enum SearchResult {
    /// Found a match with a relevance score.
    Found { book: Book, score: f64 },
    /// No results for the query.
    NotFound(String),
    /// Search error with message.
    Error { message: String, code: u32 },
}

impl SearchResult {
    /// Check if this result contains a match.
    pub fn is_match(&self) -> bool {
        matches!(self, SearchResult::Found { .. })
    }
}

/// Extension: Iterator with associated type.
pub struct BookIterator {
    books: Vec<Book>,
    index: usize,
}

impl Iterator for BookIterator {
    type Item = Book;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.books.len() {
            self.index += 1;
            // In real code we'd use a different approach; this is for testing symbol discovery
            None
        } else {
            None
        }
    }
}
```

**Step 12: Create src/extensions/advanced.rs**

Extension: lifetimes, impl Trait return, re-exports, derive macros.

```rust
use crate::models::book::Book;
use crate::traits::searchable::Searchable;

/// Extension: derive macros generate code (Debug, Clone).
#[derive(Debug, Clone, PartialEq)]
pub struct BookRef {
    pub title: String,
    pub available: bool,
}

/// Extension: lifetime annotations.
pub fn borrow_title<'a>(book: &'a Book) -> &'a str {
    book.title()
}

/// Extension: impl Trait return type.
pub fn available_titles(books: &[Book]) -> impl Iterator<Item = &str> {
    books.iter().filter(|b| b.is_available()).map(|b| b.title())
}

/// Extension: re-export (pub use).
pub use crate::models::genre::Genre as BookGenre;
```

**Step 13: Verify the fixture compiles**

Run: `cd tests/fixtures/rust-library && cargo check`
Expected: compiles cleanly

**Step 14: Commit**

```bash
git add tests/fixtures/rust-library/
git commit -m "feat(e2e): add Rust fixture project with core + extension features"
```

---

## Task 3: Create the Python fixture project

**Files:**
- Create: `tests/fixtures/python-library/pyproject.toml`
- Create: `tests/fixtures/python-library/library/__init__.py`
- Create: `tests/fixtures/python-library/library/models/__init__.py`
- Create: `tests/fixtures/python-library/library/models/book.py`
- Create: `tests/fixtures/python-library/library/models/genre.py`
- Create: `tests/fixtures/python-library/library/interfaces/__init__.py`
- Create: `tests/fixtures/python-library/library/interfaces/searchable.py`
- Create: `tests/fixtures/python-library/library/services/__init__.py`
- Create: `tests/fixtures/python-library/library/services/catalog.py`
- Create: `tests/fixtures/python-library/library/extensions/__init__.py`
- Create: `tests/fixtures/python-library/library/extensions/advanced.py`

**Step 1: Create pyproject.toml**

```toml
[project]
name = "library"
version = "0.1.0"
requires-python = ">=3.10"
```

**Step 2: Create library/__init__.py**

```python
"""Library management system."""
from library.models.book import Book
from library.models.genre import Genre
from library.interfaces.searchable import Searchable
from library.services.catalog import Catalog
```

**Step 3: Create library/models/__init__.py**

```python
"""Data models."""
```

**Step 4: Create library/models/book.py**

Core: class, methods, constants. Extension: @dataclass, @property.

```python
from __future__ import annotations
from dataclasses import dataclass, field
from library.models.genre import Genre


MAX_RESULTS: int = 100
"""Maximum number of search results to return."""


@dataclass
class Book:
    """A book in the library catalog."""

    title: str
    isbn: str
    genre: Genre
    copies_available: int = 1

    @property
    def is_available(self) -> bool:
        """Check if the book is available for borrowing."""
        return self.copies_available > 0

    def __repr__(self) -> str:
        return f"Book(title={self.title!r}, isbn={self.isbn!r})"

    def __eq__(self, other: object) -> bool:
        if not isinstance(other, Book):
            return NotImplemented
        return self.isbn == other.isbn

    def __hash__(self) -> int:
        return hash(self.isbn)
```

**Step 5: Create library/models/genre.py**

Core: Enum.

```python
from enum import Enum


class Genre(Enum):
    """Genre categories for library books."""

    FICTION = "fiction"
    NON_FICTION = "non_fiction"
    SCIENCE = "science"
    HISTORY = "history"
    BIOGRAPHY = "biography"

    def label(self) -> str:
        """Human-readable label for display."""
        return self.value.replace("_", " ").title()
```

**Step 6: Create library/interfaces/__init__.py**

```python
"""Interfaces and protocols."""
```

**Step 7: Create library/interfaces/searchable.py**

Core: ABC. Extension: Protocol.

```python
from abc import ABC, abstractmethod
from typing import Protocol, runtime_checkable


class Searchable(ABC):
    """Interface for anything that can be searched in the catalog."""

    @abstractmethod
    def search_text(self) -> str:
        """Return a search-friendly text representation."""
        ...

    def relevance(self) -> float:
        """Default relevance score — override for custom ranking."""
        return 0.0


@runtime_checkable
class HasISBN(Protocol):
    """Extension: structural typing via Protocol."""

    @property
    def isbn(self) -> str: ...
```

**Step 8: Create library/services/__init__.py**

```python
"""Service layer."""
```

**Step 9: Create library/services/catalog.py**

Core: generics, nested class, free functions.

```python
from __future__ import annotations
from typing import Generic, TypeVar
from library.interfaces.searchable import Searchable

T = TypeVar("T", bound=Searchable)


class Catalog(Generic[T]):
    """A catalog that holds searchable items."""

    class Stats:
        """Nested class: statistics about the catalog."""

        def __init__(self, total_items: int, name: str) -> None:
            self.total_items = total_items
            self.name = name

    def __init__(self, name: str) -> None:
        self._items: list[T] = []
        self._name = name

    def add(self, item: T) -> None:
        """Add an item to the catalog."""
        self._items.append(item)

    def search(self, query: str) -> list[T]:
        """Search for items matching a query."""
        return [item for item in self._items if query in item.search_text()]

    def stats(self) -> Stats:
        """Get catalog statistics."""
        return self.Stats(total_items=len(self._items), name=self._name)


def create_default_catalog() -> Catalog:
    """Free function: create a default catalog for books."""
    return Catalog(name="Main Library")
```

**Step 10: Create library/extensions/__init__.py**

```python
"""Language-specific extensions for testing edge cases."""
```

**Step 11: Create library/extensions/advanced.py**

Extensions: multiple inheritance, nested functions, type aliases, *args/**kwargs.

```python
from __future__ import annotations
from typing import Any
from library.models.book import Book
from library.interfaces.searchable import Searchable


# Extension: type alias
BookList = list[Book]


class Playable:
    """Mixin for items that can be played (audiobooks)."""

    def play(self) -> str:
        return "Playing..."

    def duration_minutes(self) -> int:
        return 0


class AudioBook(Book, Playable):
    """Extension: multiple inheritance with MRO."""

    narrator: str = ""

    def search_text(self) -> str:
        return f"{self.title} (narrated by {self.narrator})"


def search_books(*terms: str, **filters: Any) -> BookList:
    """Extension: *args and **kwargs in signature."""
    return []


def rank_results(books: BookList) -> BookList:
    """Extension: uses type alias in signature."""

    def _score(book: Book) -> float:
        """Extension: nested function / closure."""
        return 1.0 if book.is_available else 0.5

    return sorted(books, key=_score, reverse=True)
```

**Step 12: Verify pyright is happy**

Run: `cd tests/fixtures/python-library && python -c "import library"`
Expected: imports without error (basic sanity check)

**Step 13: Commit**

```bash
git add tests/fixtures/python-library/
git commit -m "feat(e2e): add Python fixture project with core + extension features"
```

---

## Task 4: Create the TypeScript fixture project

**Files:**
- Create: `tests/fixtures/typescript-library/package.json`
- Create: `tests/fixtures/typescript-library/tsconfig.json`
- Create: `tests/fixtures/typescript-library/src/index.ts`
- Create: `tests/fixtures/typescript-library/src/models/book.ts`
- Create: `tests/fixtures/typescript-library/src/models/genre.ts`
- Create: `tests/fixtures/typescript-library/src/interfaces/searchable.ts`
- Create: `tests/fixtures/typescript-library/src/interfaces/types.ts`
- Create: `tests/fixtures/typescript-library/src/services/catalog.ts`
- Create: `tests/fixtures/typescript-library/src/extensions/advanced.ts`

**Step 1: Create package.json**

```json
{
  "name": "typescript-library",
  "version": "0.1.0",
  "private": true,
  "main": "src/index.ts"
}
```

**Step 2: Create tsconfig.json**

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "module": "commonjs",
    "lib": ["ES2022"],
    "strict": true,
    "esModuleInterop": true,
    "outDir": "dist",
    "rootDir": "src",
    "experimentalDecorators": true,
    "emitDecoratorMetadata": true
  },
  "include": ["src"]
}
```

**Step 3: Create src/index.ts**

```typescript
export { Book, MAX_RESULTS } from './models/book';
export { Genre } from './models/genre';
export { Searchable } from './interfaces/searchable';
export { Catalog, createDefaultCatalog } from './services/catalog';
```

**Step 4: Create src/models/book.ts**

Core: class, methods, constants.

```typescript
import { Genre } from './genre';

/** Maximum number of search results to return. */
export const MAX_RESULTS = 100;

/** A book in the library catalog. */
export class Book {
    constructor(
        private _title: string,
        private _isbn: string,
        private _genre: Genre,
        private _copiesAvailable: number = 1
    ) {}

    /** Get the book title. */
    title(): string {
        return this._title;
    }

    /** Get the ISBN. */
    isbn(): string {
        return this._isbn;
    }

    /** Check if the book is available for borrowing. */
    isAvailable(): boolean {
        return this._copiesAvailable > 0;
    }

    /** Get the genre. */
    genre(): Genre {
        return this._genre;
    }
}
```

**Step 5: Create src/models/genre.ts**

Core: enum.

```typescript
/** Genre categories for library books. */
export enum Genre {
    Fiction = 'fiction',
    NonFiction = 'non_fiction',
    Science = 'science',
    History = 'history',
    Biography = 'biography',
}

/** Human-readable label for a genre. */
export function genreLabel(genre: Genre): string {
    return genre.replace('_', ' ');
}
```

**Step 6: Create src/interfaces/searchable.ts**

Core: interface.

```typescript
/** Interface for anything that can be searched in the catalog. */
export interface Searchable {
    /** Return a search-friendly text representation. */
    searchText(): string;

    /** Optional relevance score — default is 0. */
    relevance?(): number;
}
```

**Step 7: Create src/interfaces/types.ts**

Extension: union/intersection, mapped, conditional types, type guards.

```typescript
import { Book } from '../models/book';

/** Extension: union type. */
export type SearchResult = FoundResult | NotFoundResult | ErrorResult;

export interface FoundResult {
    kind: 'found';
    book: Book;
    score: number;
}

export interface NotFoundResult {
    kind: 'not_found';
    query: string;
}

export interface ErrorResult {
    kind: 'error';
    message: string;
    code: number;
}

/** Extension: type guard function. */
export function isFound(result: SearchResult): result is FoundResult {
    return result.kind === 'found';
}

/** Extension: mapped type. */
export type ReadonlyBook = Readonly<Pick<Book, 'title' | 'isbn'>>;

/** Extension: conditional type. */
export type IsAvailable<T> = T extends { isAvailable(): boolean } ? true : false;

/** Extension: index signature. */
export interface BookIndex {
    [isbn: string]: Book;
}
```

**Step 8: Create src/services/catalog.ts**

Core: generics, nested class equivalent, free functions.

```typescript
import { Searchable } from '../interfaces/searchable';

/** Statistics about the catalog. */
export class CatalogStats {
    constructor(
        public totalItems: number,
        public name: string
    ) {}
}

/** A catalog that holds searchable items. */
export class Catalog<T extends Searchable> {
    private items: T[] = [];

    constructor(private name: string) {}

    /** Add an item to the catalog. */
    add(item: T): void {
        this.items.push(item);
    }

    /** Search for items matching a query. */
    search(query: string): T[] {
        return this.items.filter(item => item.searchText().includes(query));
    }

    /** Get catalog statistics. */
    stats(): CatalogStats {
        return new CatalogStats(this.items.length, this.name);
    }
}

/** Free function: create a default catalog. */
export function createDefaultCatalog(): Catalog<any> {
    return new Catalog('Main Library');
}
```

**Step 9: Create src/extensions/advanced.ts**

Extension: overloaded signatures, decorators, namespace merging, default export.

```typescript
import { Book } from '../models/book';

/** Extension: function overload signatures. */
export function findBook(isbn: string): Book | undefined;
export function findBook(title: string, author: string): Book[];
export function findBook(first: string, second?: string): Book | Book[] | undefined {
    return undefined;
}

/** Extension: decorator (experimental). */
function logged(target: any, propertyKey: string, descriptor: PropertyDescriptor) {
    return descriptor;
}

export class BookService {
    @logged
    process(book: Book): void {
        // decorated method
    }
}

/** Extension: namespace merging (declaration merging). */
export interface BookMetadata {
    title: string;
    pages: number;
}

export namespace BookMetadata {
    export function create(title: string, pages: number): BookMetadata {
        return { title, pages };
    }
}

/** Extension: default export. */
export default class DefaultCatalog {
    readonly name = 'default';
}
```

**Step 10: Commit**

```bash
git add tests/fixtures/typescript-library/
git commit -m "feat(e2e): add TypeScript fixture project with core + extension features"
```

---

## Task 5: Create the Kotlin fixture project

**Files:**
- Create: `tests/fixtures/kotlin-library/build.gradle.kts`
- Create: `tests/fixtures/kotlin-library/settings.gradle.kts`
- Create: `tests/fixtures/kotlin-library/src/main/kotlin/library/models/Book.kt`
- Create: `tests/fixtures/kotlin-library/src/main/kotlin/library/models/Genre.kt`
- Create: `tests/fixtures/kotlin-library/src/main/kotlin/library/interfaces/Searchable.kt`
- Create: `tests/fixtures/kotlin-library/src/main/kotlin/library/services/Catalog.kt`
- Create: `tests/fixtures/kotlin-library/src/main/kotlin/library/extensions/Results.kt`
- Create: `tests/fixtures/kotlin-library/src/main/kotlin/library/extensions/Advanced.kt`

**Step 1: Create build.gradle.kts**

```kotlin
plugins {
    kotlin("jvm") version "2.1.0"
}

group = "library"
version = "0.1.0"

repositories {
    mavenCentral()
}

dependencies {
    implementation(kotlin("stdlib"))
}
```

**Step 2: Create settings.gradle.kts**

```kotlin
rootProject.name = "kotlin-library"
```

**Step 3: Create src/main/kotlin/library/models/Book.kt**

Core: data class, methods, constants. Extension: companion object.

```kotlin
package library.models

/** Maximum number of search results to return. */
const val MAX_RESULTS: Int = 100

/** A book in the library catalog. */
data class Book(
    val title: String,
    val isbn: String,
    val genre: Genre,
    val copiesAvailable: Int = 1
) {
    /** Check if the book is available for borrowing. */
    fun isAvailable(): Boolean = copiesAvailable > 0

    /** Extension: companion object with factory methods. */
    companion object {
        fun create(title: String, isbn: String): Book =
            Book(title, isbn, Genre.FICTION)

        fun fromJson(json: String): Book =
            Book("Parsed", "000-0", Genre.FICTION)
    }
}
```

**Step 4: Create src/main/kotlin/library/models/Genre.kt**

Core: enum class.

```kotlin
package library.models

/** Genre categories for library books. */
enum class Genre {
    FICTION,
    NON_FICTION,
    SCIENCE,
    HISTORY,
    BIOGRAPHY;

    /** Human-readable label for display. */
    fun label(): String = name.replace("_", " ").lowercase()
        .replaceFirstChar { it.uppercase() }
}
```

**Step 5: Create src/main/kotlin/library/interfaces/Searchable.kt**

Core: interface.

```kotlin
package library.interfaces

/** Interface for anything that can be searched in the catalog. */
interface Searchable {
    /** Return a search-friendly text representation. */
    fun searchText(): String

    /** Default relevance score — override for custom ranking. */
    fun relevance(): Double = 0.0
}
```

**Step 6: Create src/main/kotlin/library/services/Catalog.kt**

Core: generics, nested class, free functions. Extension: suspend, extension functions.

```kotlin
package library.services

import library.interfaces.Searchable
import library.models.Book

/** A catalog that holds searchable items. */
class Catalog<T : Searchable>(private val name: String) {

    private val items = mutableListOf<T>()

    /** Nested class: statistics about the catalog. */
    data class CatalogStats(val totalItems: Int, val name: String)

    /** Add an item to the catalog. */
    fun add(item: T) {
        items.add(item)
    }

    /** Search for items matching a query. */
    fun search(query: String): List<T> =
        items.filter { it.searchText().contains(query) }

    /** Get catalog statistics. */
    fun stats(): CatalogStats = CatalogStats(items.size, name)
}

/** Free function: create a default catalog for books. */
fun createDefaultCatalog(): Catalog<Book> = Catalog("Main Library")

/** Extension: suspend function (coroutine). */
suspend fun <T : Searchable> Catalog<T>.searchAsync(query: String): List<T> =
    search(query)

/** Extension: extension function on Book. */
fun Book.toSearchText(): String = "$title ($isbn)"
```

**Step 7: Create src/main/kotlin/library/extensions/Results.kt**

Extension: sealed class hierarchy, object declarations.

```kotlin
package library.extensions

import library.models.Book

/** Extension: sealed class with data class, object, and class subclasses. */
sealed class SearchResult {
    /** Found a match with a relevance score. */
    data class Found(val book: Book, val score: Double) : SearchResult()

    /** No results for the query. */
    object NotFound : SearchResult()

    /** Search error with message. */
    data class Error(val message: String, val code: Int) : SearchResult()

    /** Check if this result contains a match. */
    fun isMatch(): Boolean = this is Found
}

/** Extension: object declaration (singleton). */
object BookRegistry {
    private val books = mutableMapOf<String, Book>()

    fun register(book: Book) {
        books[book.isbn] = book
    }

    fun lookup(isbn: String): Book? = books[isbn]
}
```

**Step 8: Create src/main/kotlin/library/extensions/Advanced.kt**

Extension: delegated properties, inline class, scope functions.

```kotlin
package library.extensions

import library.models.Book

/** Extension: inline/value class. */
@JvmInline
value class ISBN(val value: String)

/** Extension: delegated property. */
class LazyBook(title: String) {
    val formattedTitle: String by lazy {
        title.uppercase()
    }
}

/** Extension: scope functions with receiver. */
fun createBookWithDefaults(): Book =
    Book(
        title = "Default",
        isbn = "000-0",
        genre = library.models.Genre.FICTION
    ).let { book ->
        // Using scope function
        book.copy(copiesAvailable = 5)
    }
```

**Step 9: Verify the fixture compiles**

Run: `cd tests/fixtures/kotlin-library && ./gradlew check` (if gradlew exists) or just verify structure is correct.

Note: The Kotlin fixture doesn't need to compile in CI if kotlin-lsp isn't installed. The LSP server will read the source files regardless.

**Step 10: Commit**

```bash
git add tests/fixtures/kotlin-library/
git commit -m "feat(e2e): add Kotlin fixture project with core + extension features"
```

---

## Task 6: Create the Java fixture project

**Files:**
- Create: `tests/fixtures/java-library/build.gradle`
- Create: `tests/fixtures/java-library/settings.gradle`
- Create: `tests/fixtures/java-library/src/main/java/library/models/Book.java`
- Create: `tests/fixtures/java-library/src/main/java/library/models/Genre.java`
- Create: `tests/fixtures/java-library/src/main/java/library/interfaces/Searchable.java`
- Create: `tests/fixtures/java-library/src/main/java/library/services/Catalog.java`
- Create: `tests/fixtures/java-library/src/main/java/library/extensions/Results.java`
- Create: `tests/fixtures/java-library/src/main/java/library/extensions/Advanced.java`

**Step 1: Create build.gradle**

```groovy
plugins {
    id 'java'
}

group = 'library'
version = '0.1.0'

java {
    sourceCompatibility = JavaVersion.VERSION_21
    targetCompatibility = JavaVersion.VERSION_21
}
```

**Step 2: Create settings.gradle**

```groovy
rootProject.name = 'java-library'
```

**Step 3: Create src/main/java/library/models/Book.java**

Core: record, methods, constants.

```java
package library.models;

/** A book in the library catalog. */
public record Book(
    String title,
    String isbn,
    Genre genre,
    int copiesAvailable
) {
    /** Maximum number of search results to return. */
    public static final int MAX_RESULTS = 100;

    /** Compact constructor with default copies. */
    public Book(String title, String isbn, Genre genre) {
        this(title, isbn, genre, 1);
    }

    /** Check if the book is available for borrowing. */
    public boolean isAvailable() {
        return copiesAvailable > 0;
    }
}
```

**Step 4: Create src/main/java/library/models/Genre.java**

Core: enum.

```java
package library.models;

/** Genre categories for library books. */
public enum Genre {
    FICTION,
    NON_FICTION,
    SCIENCE,
    HISTORY,
    BIOGRAPHY;

    /** Human-readable label for display. */
    public String label() {
        return name().replace("_", " ").substring(0, 1)
            + name().replace("_", " ").substring(1).toLowerCase();
    }
}
```

**Step 5: Create src/main/java/library/interfaces/Searchable.java**

Core: interface. Extension: default method.

```java
package library.interfaces;

/** Interface for anything that can be searched in the catalog. */
public interface Searchable {
    /** Return a search-friendly text representation. */
    String searchText();

    /** Extension: default method — override for custom ranking. */
    default double relevance() {
        return 0.0;
    }
}
```

**Step 6: Create src/main/java/library/services/Catalog.java**

Core: generics, static nested class, static methods.

```java
package library.services;

import library.interfaces.Searchable;
import library.models.Book;
import library.models.Genre;

import java.util.ArrayList;
import java.util.List;

/** A catalog that holds searchable items. */
public class Catalog<T extends Searchable> {

    private final List<T> items = new ArrayList<>();
    private final String name;

    /** Static nested class: statistics about the catalog. */
    public static class CatalogStats {
        public final int totalItems;
        public final String name;

        public CatalogStats(int totalItems, String name) {
            this.totalItems = totalItems;
            this.name = name;
        }
    }

    public Catalog(String name) {
        this.name = name;
    }

    /** Add an item to the catalog. */
    public void add(T item) {
        items.add(item);
    }

    /** Search for items matching a query. */
    public List<T> search(String query) {
        return items.stream()
            .filter(item -> item.searchText().contains(query))
            .toList();
    }

    /** Get catalog statistics. */
    public CatalogStats stats() {
        return new CatalogStats(items.size(), name);
    }

    /** Static factory: create a default catalog. */
    public static Catalog<Book> createDefault() {
        return new Catalog<>("Main Library");
    }
}
```

**Step 7: Create src/main/java/library/extensions/Results.java**

Extension: sealed interface, pattern matching, records.

```java
package library.extensions;

import library.models.Book;

/** Extension: sealed interface hierarchy. */
public sealed interface SearchResult permits SearchResult.Found, SearchResult.NotFound, SearchResult.Error {

    /** Found a match with a relevance score. */
    record Found(Book book, double score) implements SearchResult {}

    /** No results for the query. */
    record NotFound(String query) implements SearchResult {}

    /** Search error with message. */
    record Error(String message, int code) implements SearchResult {}

    /** Check if this result contains a match — extension: pattern matching. */
    default boolean isMatch() {
        return this instanceof Found;
    }
}
```

**Step 8: Create src/main/java/library/extensions/Advanced.java**

Extension: annotations, anonymous classes, generics with wildcards.

```java
package library.extensions;

import library.interfaces.Searchable;
import library.models.Book;

import java.lang.annotation.Retention;
import java.lang.annotation.RetentionPolicy;
import java.util.List;

/** Extension: custom annotation. */
@Retention(RetentionPolicy.RUNTIME)
public @interface Indexed {
    String value() default "";
}

/** Extension: class with annotations, anonymous class, and wildcards. */
class BookProcessor {

    @Indexed("isbn")
    public void process(Book book) {
        // annotated method
    }

    /** Extension: anonymous class implementing an interface. */
    public Searchable createAnonymousSearchable() {
        return new Searchable() {
            @Override
            public String searchText() {
                return "anonymous";
            }
        };
    }

    /** Extension: generics with wildcards. */
    public void processAll(List<? extends Searchable> items) {
        for (Searchable item : items) {
            item.searchText();
        }
    }

    /** Extension: static inner class vs non-static. */
    static class BatchResult {
        int processed;
        int failed;
    }

    class ProcessingContext {
        String currentBook;
    }
}
```

NOTE: The `@interface Indexed` and `class BookProcessor` need to be in separate files since Java requires public types to be in files matching their name, and `@interface` is a type. Move `Indexed` annotation into the same file but make it non-public, or restructure. The above structure works because `Indexed` is public but `BookProcessor` is package-private.

Actually, Java allows only one public top-level class per file. Fix: make `Indexed` the public type (it's declared as `public @interface`) and `BookProcessor` package-private. This is fine for our testing purposes.

**Step 9: Commit**

```bash
git add tests/fixtures/java-library/
git commit -m "feat(e2e): add Java fixture project with core + extension features"
```

---

## Task 7: Create the expectations TOML files

**Files:**
- Create: `tests/fixtures/core-expectations.toml`
- Create: `tests/fixtures/rust-extensions.toml`
- Create: `tests/fixtures/python-extensions.toml`
- Create: `tests/fixtures/typescript-extensions.toml`
- Create: `tests/fixtures/kotlin-extensions.toml`
- Create: `tests/fixtures/java-extensions.toml`

**Step 1: Create core-expectations.toml**

```toml
# Core expectations: every language must pass all of these.
# Each section is a test case run by the harness.

[class_with_methods]
description = "A class/struct with methods is discoverable via get_symbols_overview"
tool = "get_symbols_overview"

  [class_with_methods.rust]
  path = "src/models/book.rs"
  contains_symbols = ["Book", "new", "title", "isbn", "is_available", "genre"]

  [class_with_methods.python]
  path = "library/models/book.py"
  contains_symbols = ["Book", "is_available"]

  [class_with_methods.typescript]
  path = "src/models/book.ts"
  contains_symbols = ["Book", "title", "isbn", "isAvailable", "genre"]

  [class_with_methods.kotlin]
  path = "src/main/kotlin/library/models/Book.kt"
  contains_symbols = ["Book", "isAvailable"]

  [class_with_methods.java]
  path = "src/main/java/library/models/Book.java"
  contains_symbols = ["Book", "isAvailable"]


[find_method_body]
description = "find_symbol with include_body returns method source"
tool = "find_symbol"

  [find_method_body.rust]
  file = "src/models/book.rs"
  symbol = "is_available"
  body_contains = ["copies_available", "> 0"]

  [find_method_body.python]
  file = "library/models/book.py"
  symbol = "is_available"
  body_contains = ["copies_available", "> 0"]

  [find_method_body.typescript]
  file = "src/models/book.ts"
  symbol = "isAvailable"
  body_contains = ["_copiesAvailable", "> 0"]

  [find_method_body.kotlin]
  file = "src/main/kotlin/library/models/Book.kt"
  symbol = "isAvailable"
  body_contains = ["copiesAvailable", "> 0"]

  [find_method_body.java]
  file = "src/main/java/library/models/Book.java"
  symbol = "isAvailable"
  body_contains = ["copiesAvailable", "> 0"]


[enum_variants]
description = "Enum variants are discoverable as children"
tool = "get_symbols_overview"

  [enum_variants.rust]
  path = "src/models/genre.rs"
  contains_symbols = ["Genre", "Fiction", "NonFiction", "Science", "History", "Biography"]

  [enum_variants.python]
  path = "library/models/genre.py"
  contains_symbols = ["Genre", "FICTION", "NON_FICTION", "SCIENCE", "HISTORY", "BIOGRAPHY"]

  [enum_variants.typescript]
  path = "src/models/genre.ts"
  contains_symbols = ["Genre", "Fiction", "NonFiction", "Science", "History", "Biography"]

  [enum_variants.kotlin]
  path = "src/main/kotlin/library/models/Genre.kt"
  contains_symbols = ["Genre", "FICTION", "NON_FICTION", "SCIENCE", "HISTORY", "BIOGRAPHY"]

  [enum_variants.java]
  path = "src/main/java/library/models/Genre.java"
  contains_symbols = ["Genre", "FICTION", "NON_FICTION", "SCIENCE", "HISTORY", "BIOGRAPHY"]


[interface_definition]
description = "Interface/trait is discoverable with its methods"
tool = "get_symbols_overview"

  [interface_definition.rust]
  path = "src/traits/searchable.rs"
  contains_symbols = ["Searchable", "search_text", "relevance"]

  [interface_definition.python]
  path = "library/interfaces/searchable.py"
  contains_symbols = ["Searchable", "search_text", "relevance"]

  [interface_definition.typescript]
  path = "src/interfaces/searchable.ts"
  contains_symbols = ["Searchable", "searchText"]

  [interface_definition.kotlin]
  path = "src/main/kotlin/library/interfaces/Searchable.kt"
  contains_symbols = ["Searchable", "searchText", "relevance"]

  [interface_definition.java]
  path = "src/main/java/library/interfaces/Searchable.java"
  contains_symbols = ["Searchable", "searchText", "relevance"]


[generic_class]
description = "Generic/parameterized class is discoverable"
tool = "get_symbols_overview"

  [generic_class.rust]
  path = "src/services/catalog.rs"
  contains_symbols = ["Catalog", "new", "add", "search", "stats"]

  [generic_class.python]
  path = "library/services/catalog.py"
  contains_symbols = ["Catalog", "add", "search", "stats"]

  [generic_class.typescript]
  path = "src/services/catalog.ts"
  contains_symbols = ["Catalog", "add", "search", "stats"]

  [generic_class.kotlin]
  path = "src/main/kotlin/library/services/Catalog.kt"
  contains_symbols = ["Catalog", "add", "search", "stats"]

  [generic_class.java]
  path = "src/main/java/library/services/Catalog.java"
  contains_symbols = ["Catalog", "add", "search", "stats"]


[nested_type]
description = "Nested type (inner class/struct) is discoverable"
tool = "get_symbols_overview"

  [nested_type.rust]
  path = "src/services/catalog.rs"
  contains_symbols = ["CatalogStats"]

  [nested_type.python]
  path = "library/services/catalog.py"
  contains_symbols = ["Stats"]

  [nested_type.typescript]
  path = "src/services/catalog.ts"
  contains_symbols = ["CatalogStats"]

  [nested_type.kotlin]
  path = "src/main/kotlin/library/services/Catalog.kt"
  contains_symbols = ["CatalogStats"]

  [nested_type.java]
  path = "src/main/java/library/services/Catalog.java"
  contains_symbols = ["CatalogStats"]


[free_function]
description = "Top-level / free functions are discoverable"
tool = "get_symbols_overview"

  [free_function.rust]
  path = "src/services/catalog.rs"
  contains_symbols = ["create_default_catalog"]

  [free_function.python]
  path = "library/services/catalog.py"
  contains_symbols = ["create_default_catalog"]

  [free_function.typescript]
  path = "src/services/catalog.ts"
  contains_symbols = ["createDefaultCatalog"]

  [free_function.kotlin]
  path = "src/main/kotlin/library/services/Catalog.kt"
  contains_symbols = ["createDefaultCatalog"]

  [free_function.java]
  path = "src/main/java/library/services/Catalog.java"
  contains_symbols = ["createDefault"]


[constants]
description = "Constants/statics are discoverable"
tool = "get_symbols_overview"

  [constants.rust]
  path = "src/models/book.rs"
  contains_symbols = ["MAX_RESULTS"]

  [constants.python]
  path = "library/models/book.py"
  contains_symbols = ["MAX_RESULTS"]

  [constants.typescript]
  path = "src/models/book.ts"
  contains_symbols = ["MAX_RESULTS"]

  [constants.kotlin]
  path = "src/main/kotlin/library/models/Book.kt"
  contains_symbols = ["MAX_RESULTS"]

  [constants.java]
  path = "src/main/java/library/models/Book.java"
  contains_symbols = ["MAX_RESULTS"]


[list_functions_signatures]
description = "list_functions returns function signatures"
tool = "list_functions"

  [list_functions_signatures.rust]
  path = "src/services/catalog.rs"
  contains_functions = ["new", "add", "search", "stats", "create_default_catalog"]

  [list_functions_signatures.python]
  path = "library/services/catalog.py"
  contains_functions = ["add", "search", "stats", "create_default_catalog"]

  [list_functions_signatures.typescript]
  path = "src/services/catalog.ts"
  contains_functions = ["add", "search", "stats", "createDefaultCatalog"]

  [list_functions_signatures.kotlin]
  path = "src/main/kotlin/library/services/Catalog.kt"
  contains_functions = ["add", "search", "stats", "createDefaultCatalog"]

  [list_functions_signatures.java]
  path = "src/main/java/library/services/Catalog.java"
  contains_functions = ["add", "search", "stats", "createDefault"]


[search_pattern]
description = "search_for_pattern finds text across files"
tool = "search_for_pattern"

  [search_pattern.rust]
  pattern = "Searchable"
  expected_files = ["searchable.rs", "catalog.rs"]

  [search_pattern.python]
  pattern = "Searchable"
  expected_files = ["searchable.py", "catalog.py"]

  [search_pattern.typescript]
  pattern = "Searchable"
  expected_files = ["searchable.ts", "catalog.ts"]

  [search_pattern.kotlin]
  pattern = "Searchable"
  expected_files = ["Searchable.kt", "Catalog.kt"]

  [search_pattern.java]
  pattern = "Searchable"
  expected_files = ["Searchable.java", "Catalog.java"]
```

**Step 2: Create rust-extensions.toml**

```toml
[enum_struct_variants]
description = "Enum with struct and tuple variants"
tool = "get_symbols_overview"
path = "src/extensions/results.rs"
contains_symbols = ["SearchResult", "Found", "NotFound", "Error"]

[trait_default_method_body]
description = "Trait default method has readable body"
tool = "find_symbol"
file = "src/traits/searchable.rs"
symbol = "relevance"
body_contains = ["0.0"]

[derive_macro_struct]
description = "Struct with derive macros is discoverable"
tool = "get_symbols_overview"
path = "src/extensions/advanced.rs"
contains_symbols = ["BookRef"]

[lifetime_function]
description = "Function with lifetime annotations is discoverable"
tool = "find_symbol"
file = "src/extensions/advanced.rs"
symbol = "borrow_title"
body_contains = ["book.title()"]

[impl_trait_return]
description = "Function with impl Trait return is discoverable"
tool = "get_symbols_overview"
path = "src/extensions/advanced.rs"
contains_symbols = ["available_titles"]

[reexport]
description = "Re-exported type alias is discoverable"
tool = "get_symbols_overview"
path = "src/extensions/advanced.rs"
contains_symbols = ["BookGenre"]

[iterator_associated_type]
description = "Iterator impl with associated type"
tool = "get_symbols_overview"
path = "src/extensions/results.rs"
contains_symbols = ["BookIterator"]
```

**Step 3: Create python-extensions.toml**

```toml
[dataclass_discovery]
description = "Dataclass is discoverable as a class"
tool = "get_symbols_overview"
path = "library/models/book.py"
contains_symbols = ["Book"]

[property_decorator]
description = "@property is discoverable as a method/attribute"
tool = "get_symbols_overview"
path = "library/models/book.py"
contains_symbols = ["is_available"]

[dunder_methods]
description = "Dunder methods are discoverable"
tool = "get_symbols_overview"
path = "library/models/book.py"
contains_symbols = ["__repr__", "__eq__", "__hash__"]

[protocol_class]
description = "Protocol class is discoverable"
tool = "get_symbols_overview"
path = "library/interfaces/searchable.py"
contains_symbols = ["HasISBN"]

[multiple_inheritance]
description = "Class with multiple inheritance is discoverable"
tool = "get_symbols_overview"
path = "library/extensions/advanced.py"
contains_symbols = ["AudioBook", "Playable"]

[nested_function]
description = "Nested function inside a function"
tool = "find_symbol"
file = "library/extensions/advanced.py"
symbol = "rank_results"
body_contains = ["_score"]

[type_alias]
description = "Type alias is discoverable"
tool = "get_symbols_overview"
path = "library/extensions/advanced.py"
contains_symbols = ["BookList"]

[args_kwargs]
description = "Function with *args and **kwargs"
tool = "get_symbols_overview"
path = "library/extensions/advanced.py"
contains_symbols = ["search_books"]
```

**Step 4: Create typescript-extensions.toml**

```toml
[union_type]
description = "Union type alias is discoverable"
tool = "get_symbols_overview"
path = "src/interfaces/types.ts"
contains_symbols = ["SearchResult", "FoundResult", "NotFoundResult", "ErrorResult"]

[type_guard]
description = "Type guard function is discoverable"
tool = "get_symbols_overview"
path = "src/interfaces/types.ts"
contains_symbols = ["isFound"]

[mapped_type]
description = "Mapped type alias is discoverable"
tool = "get_symbols_overview"
path = "src/interfaces/types.ts"
contains_symbols = ["ReadonlyBook"]

[overloaded_function]
description = "Overloaded function is discoverable"
tool = "get_symbols_overview"
path = "src/extensions/advanced.ts"
contains_symbols = ["findBook"]

[decorator_class]
description = "Decorated class is discoverable"
tool = "get_symbols_overview"
path = "src/extensions/advanced.ts"
contains_symbols = ["BookService", "process"]

[namespace_merging]
description = "Merged namespace/interface is discoverable"
tool = "get_symbols_overview"
path = "src/extensions/advanced.ts"
contains_symbols = ["BookMetadata", "create"]

[default_export]
description = "Default export class is discoverable"
tool = "get_symbols_overview"
path = "src/extensions/advanced.ts"
contains_symbols = ["DefaultCatalog"]

[index_signature]
description = "Interface with index signature is discoverable"
tool = "get_symbols_overview"
path = "src/interfaces/types.ts"
contains_symbols = ["BookIndex"]
```

**Step 5: Create kotlin-extensions.toml**

```toml
[sealed_class_hierarchy]
description = "Sealed class with all subclasses discoverable"
tool = "get_symbols_overview"
path = "src/main/kotlin/library/extensions/Results.kt"
contains_symbols = ["SearchResult", "Found", "NotFound", "Error"]

[companion_object]
description = "Companion object with factory methods"
tool = "find_symbol"
file = "src/main/kotlin/library/models/Book.kt"
symbol = "Companion"
contains_symbols = ["create", "fromJson"]

[extension_function]
description = "Extension function is discoverable"
tool = "get_symbols_overview"
path = "src/main/kotlin/library/services/Catalog.kt"
contains_symbols = ["toSearchText"]

[suspend_function]
description = "Suspend function is discoverable"
tool = "get_symbols_overview"
path = "src/main/kotlin/library/services/Catalog.kt"
contains_symbols = ["searchAsync"]

[object_declaration]
description = "Object declaration (singleton) is discoverable"
tool = "get_symbols_overview"
path = "src/main/kotlin/library/extensions/Results.kt"
contains_symbols = ["BookRegistry", "register", "lookup"]

[inline_value_class]
description = "Inline/value class is discoverable"
tool = "get_symbols_overview"
path = "src/main/kotlin/library/extensions/Advanced.kt"
contains_symbols = ["ISBN"]

[delegated_property]
description = "Class with delegated property"
tool = "get_symbols_overview"
path = "src/main/kotlin/library/extensions/Advanced.kt"
contains_symbols = ["LazyBook", "formattedTitle"]

[data_class_methods]
description = "Data class is discoverable with its methods"
tool = "find_symbol"
file = "src/main/kotlin/library/models/Book.kt"
symbol = "Book"
body_contains = ["title", "isbn", "genre"]
```

**Step 6: Create java-extensions.toml**

```toml
[sealed_interface]
description = "Sealed interface with permitted subclasses"
tool = "get_symbols_overview"
path = "src/main/java/library/extensions/Results.java"
contains_symbols = ["SearchResult", "Found", "NotFound", "Error"]

[record_type]
description = "Record type is discoverable"
tool = "get_symbols_overview"
path = "src/main/java/library/models/Book.java"
contains_symbols = ["Book"]

[default_interface_method]
description = "Default method in interface has body"
tool = "find_symbol"
file = "src/main/java/library/interfaces/Searchable.java"
symbol = "relevance"
body_contains = ["0.0"]

[annotation_definition]
description = "Custom annotation is discoverable"
tool = "get_symbols_overview"
path = "src/main/java/library/extensions/Advanced.java"
contains_symbols = ["Indexed"]

[anonymous_class_method]
description = "Method that returns anonymous class"
tool = "find_symbol"
file = "src/main/java/library/extensions/Advanced.java"
symbol = "createAnonymousSearchable"
body_contains = ["new Searchable"]

[wildcard_generics]
description = "Method with wildcard generics"
tool = "get_symbols_overview"
path = "src/main/java/library/extensions/Advanced.java"
contains_symbols = ["processAll"]

[static_nested_class]
description = "Static nested class is discoverable"
tool = "get_symbols_overview"
path = "src/main/java/library/extensions/Advanced.java"
contains_symbols = ["BatchResult", "ProcessingContext"]

[enum_method]
description = "Enum with methods"
tool = "find_symbol"
file = "src/main/java/library/models/Genre.java"
symbol = "label"
body_contains = ["name()"]
```

**Step 7: Commit**

```bash
git add tests/fixtures/*.toml
git commit -m "feat(e2e): add core and language-specific expectation TOML files"
```

---

## Task 8: Create the test harness — expectation types and TOML parsing

**Files:**
- Create: `tests/e2e/mod.rs`
- Create: `tests/e2e/expectations.rs`

**Step 1: Create tests/e2e/mod.rs**

```rust
#[cfg(any(
    feature = "e2e-rust",
    feature = "e2e-python",
    feature = "e2e-typescript",
    feature = "e2e-kotlin",
    feature = "e2e-java",
))]
mod expectations;

#[cfg(any(
    feature = "e2e-rust",
    feature = "e2e-python",
    feature = "e2e-typescript",
    feature = "e2e-kotlin",
    feature = "e2e-java",
))]
mod harness;

#[cfg(feature = "e2e-rust")]
mod test_rust;

#[cfg(feature = "e2e-python")]
mod test_python;

#[cfg(feature = "e2e-typescript")]
mod test_typescript;

#[cfg(feature = "e2e-kotlin")]
mod test_kotlin;

#[cfg(feature = "e2e-java")]
mod test_java;
```

**Step 2: Create tests/e2e/expectations.rs**

Serde structs for parsing the TOML files:

```rust
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

/// A single test expectation for one language.
#[derive(Debug, Clone, Deserialize)]
pub struct LangExpectation {
    /// File or directory path relative to the fixture root.
    #[serde(alias = "file")]
    pub path: Option<String>,
    /// For find_symbol: the symbol name to search for.
    pub symbol: Option<String>,
    /// Expected symbol names in the output.
    pub contains_symbols: Option<Vec<String>>,
    /// Expected substrings in the symbol body (requires include_body=true).
    pub body_contains: Option<Vec<String>>,
    /// Expected function names (for list_functions).
    pub contains_functions: Option<Vec<String>>,
    /// Regex pattern (for search_for_pattern).
    pub pattern: Option<String>,
    /// Expected file names in search results.
    pub expected_files: Option<Vec<String>>,
    /// Expected references (for find_referencing_symbols).
    pub expected_refs_contain: Option<Vec<String>>,
}

/// A test case section from the TOML.
/// Contains a description, tool, and per-language expectations.
#[derive(Debug, Clone, Deserialize)]
pub struct TestCase {
    pub description: String,
    pub tool: String,
    // Per-language entries
    #[serde(flatten)]
    pub languages: HashMap<String, LangExpectation>,
    // For extension files that have path/symbol at the top level (single-language)
    #[serde(alias = "file")]
    pub path: Option<String>,
    pub symbol: Option<String>,
    pub contains_symbols: Option<Vec<String>>,
    pub body_contains: Option<Vec<String>>,
    pub contains_functions: Option<Vec<String>>,
    pub pattern: Option<String>,
    pub expected_files: Option<Vec<String>>,
}

/// Load expectations from a TOML file, filtered for a specific language.
/// For core-expectations.toml, extracts the language-specific sub-tables.
/// For <lang>-extensions.toml, each section IS the expectation (no language sub-tables).
pub fn load_expectations(
    toml_path: &Path,
    language: &str,
) -> Vec<(String, LangExpectation, String)> {
    let content = std::fs::read_to_string(toml_path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {e}", toml_path.display()));
    let raw: HashMap<String, toml::Value> = toml::from_str(&content)
        .unwrap_or_else(|e| panic!("Failed to parse {}: {e}", toml_path.display()));

    let mut expectations = Vec::new();

    for (test_name, value) in &raw {
        let table = value.as_table().expect("Each section should be a table");
        let description = table
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let tool = table
            .get("tool")
            .and_then(|v| v.as_str())
            .unwrap_or("get_symbols_overview")
            .to_string();

        // Check if this section has a language-specific sub-table
        if let Some(lang_value) = table.get(language) {
            // Core expectations: language sub-table
            if let Ok(lang_exp) =
                lang_value.clone().try_into::<LangExpectation>()
            {
                expectations.push((test_name.clone(), lang_exp, tool.clone()));
            }
        } else if table.get("path").is_some() || table.get("file").is_some() {
            // Extension expectations: top-level path/symbol (single-language)
            if let Ok(lang_exp) = value.clone().try_into::<LangExpectation>() {
                expectations.push((test_name.clone(), lang_exp, tool.clone()));
            }
        }
    }

    expectations
}
```

**Step 3: Verify it compiles**

Run: `cargo check --features e2e-rust`
Expected: may not compile yet (missing harness.rs and test files), but the types should be valid

**Step 4: Commit**

```bash
git add tests/e2e/
git commit -m "feat(e2e): add expectation types and TOML parsing"
```

---

## Task 9: Create the test harness — fixture context and assertion runner

**Files:**
- Create: `tests/e2e/harness.rs`

**Step 1: Create tests/e2e/harness.rs**

```rust
use crate::e2e::expectations::{load_expectations, LangExpectation};
use code_explorer::agent::Agent;
use code_explorer::lsp::manager::LspManager;
use code_explorer::tools::{Tool, ToolContext};
use code_explorer::tools::symbol::{FindSymbol, GetSymbolsOverview, FindReferencingSymbols};
use code_explorer::tools::ast::ListFunctions;
use code_explorer::tools::file::SearchForPattern;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use tokio::sync::Mutex;

/// Root of the test fixtures directory.
fn fixtures_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
}

/// Get the fixture project directory for a language.
fn fixture_dir(language: &str) -> PathBuf {
    fixtures_root().join(format!("{language}-library"))
}

/// Cached fixture contexts — one per language, started lazily.
static CONTEXTS: OnceLock<Mutex<std::collections::HashMap<String, Arc<ToolContext>>>> =
    OnceLock::new();

/// Get or create a ToolContext with a real LSP for the given language.
pub async fn fixture_context(language: &str) -> Arc<ToolContext> {
    let map_mutex = CONTEXTS.get_or_init(|| Mutex::new(std::collections::HashMap::new()));
    let mut map = map_mutex.lock().await;

    if let Some(ctx) = map.get(language) {
        return ctx.clone();
    }

    let dir = fixture_dir(language);
    assert!(dir.exists(), "Fixture directory not found: {}", dir.display());

    let agent = Agent::new(Some(dir.clone()))
        .await
        .unwrap_or_else(|e| panic!("Failed to create Agent for {language}: {e}"));

    let lsp = Arc::new(LspManager::new());
    let ctx = Arc::new(ToolContext { agent, lsp });
    map.insert(language.to_string(), ctx.clone());
    ctx
}

/// Run all expectations from a TOML file for a specific language.
pub async fn run_expectations(language: &str, toml_filename: &str) {
    let toml_path = fixtures_root().join(toml_filename);
    let expectations = load_expectations(&toml_path, language);

    if expectations.is_empty() {
        panic!(
            "No expectations found for language '{language}' in {toml_filename}. \
             Check that the TOML has sections with [{language}] sub-tables or top-level path/file."
        );
    }

    let ctx = fixture_context(language).await;
    let mut pass = 0;
    let mut failures = Vec::new();

    for (name, expectation, tool) in &expectations {
        match run_single(ctx.as_ref(), expectation, tool).await {
            Ok(()) => {
                pass += 1;
                eprintln!("  PASS  {name}");
            }
            Err(e) => {
                eprintln!("  FAIL  {name}: {e}");
                failures.push((name.clone(), e));
            }
        }
    }

    let total = pass + failures.len();
    eprintln!("\n{pass}/{total} passed for {language} ({toml_filename})");

    if !failures.is_empty() {
        panic!(
            "{} of {total} expectations failed for {language}:\n{}",
            failures.len(),
            failures
                .iter()
                .map(|(n, e)| format!("  - {n}: {e}"))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
}

/// Run a single expectation and return Ok(()) or an error message.
async fn run_single(
    ctx: &ToolContext,
    exp: &LangExpectation,
    tool: &str,
) -> Result<(), String> {
    match tool {
        "get_symbols_overview" => run_symbols_overview(ctx, exp).await,
        "find_symbol" => run_find_symbol(ctx, exp).await,
        "find_referencing_symbols" => run_find_references(ctx, exp).await,
        "list_functions" => run_list_functions(ctx, exp).await,
        "search_for_pattern" => run_search_pattern(ctx, exp).await,
        other => Err(format!("Unknown tool: {other}")),
    }
}

async fn run_symbols_overview(ctx: &ToolContext, exp: &LangExpectation) -> Result<(), String> {
    let path = exp.path.as_deref().ok_or("Missing 'path'")?;
    let result = GetSymbolsOverview
        .call(json!({ "relative_path": path }), ctx)
        .await
        .map_err(|e| format!("Tool error: {e}"))?;

    if let Some(expected) = &exp.contains_symbols {
        assert_contains_symbols(&result, expected)?;
    }
    Ok(())
}

async fn run_find_symbol(ctx: &ToolContext, exp: &LangExpectation) -> Result<(), String> {
    let symbol = exp.symbol.as_deref().ok_or("Missing 'symbol'")?;
    let mut params = json!({ "pattern": symbol });

    if let Some(path) = &exp.path {
        params["relative_path"] = json!(path);
    }

    if exp.body_contains.is_some() {
        params["include_body"] = json!(true);
    }

    let result = FindSymbol
        .call(params, ctx)
        .await
        .map_err(|e| format!("Tool error: {e}"))?;

    // Check children/contains_symbols
    if let Some(expected) = &exp.contains_symbols {
        assert_contains_symbols(&result, expected)?;
    }

    // Check body content
    if let Some(expected_body) = &exp.body_contains {
        let result_str = serde_json::to_string(&result).unwrap_or_default();
        for needle in expected_body {
            if !result_str.contains(needle) {
                return Err(format!(
                    "find_symbol(\"{symbol}\") body missing \"{needle}\""
                ));
            }
        }
    }

    Ok(())
}

async fn run_find_references(ctx: &ToolContext, exp: &LangExpectation) -> Result<(), String> {
    let symbol = exp.symbol.as_deref().ok_or("Missing 'symbol'")?;
    let file = exp.path.as_deref().ok_or("Missing 'path'/'file'")?;

    let result = FindReferencingSymbols
        .call(json!({ "name_path": symbol, "relative_path": file }), ctx)
        .await
        .map_err(|e| format!("Tool error: {e}"))?;

    if let Some(expected) = &exp.expected_refs_contain {
        let result_str = serde_json::to_string(&result).unwrap_or_default();
        for needle in expected {
            if !result_str.contains(needle) {
                return Err(format!(
                    "find_referencing_symbols(\"{symbol}\") missing reference to \"{needle}\""
                ));
            }
        }
    }

    Ok(())
}

async fn run_list_functions(ctx: &ToolContext, exp: &LangExpectation) -> Result<(), String> {
    let path = exp.path.as_deref().ok_or("Missing 'path'")?;
    let result = ListFunctions
        .call(json!({ "path": path }), ctx)
        .await
        .map_err(|e| format!("Tool error: {e}"))?;

    if let Some(expected) = &exp.contains_functions {
        let result_str = serde_json::to_string(&result).unwrap_or_default();
        for needle in expected {
            if !result_str.contains(needle) {
                return Err(format!(
                    "list_functions(\"{path}\") missing \"{needle}\""
                ));
            }
        }
    }

    Ok(())
}

async fn run_search_pattern(ctx: &ToolContext, exp: &LangExpectation) -> Result<(), String> {
    let pattern = exp.pattern.as_deref().ok_or("Missing 'pattern'")?;
    let result = SearchForPattern
        .call(json!({ "pattern": pattern }), ctx)
        .await
        .map_err(|e| format!("Tool error: {e}"))?;

    if let Some(expected_files) = &exp.expected_files {
        let result_str = serde_json::to_string(&result).unwrap_or_default();
        for needle in expected_files {
            if !result_str.contains(needle) {
                return Err(format!(
                    "search_for_pattern(\"{pattern}\") missing file \"{needle}\""
                ));
            }
        }
    }

    Ok(())
}

/// Check that expected symbol names appear somewhere in the JSON result.
fn assert_contains_symbols(result: &Value, expected: &[String]) -> Result<(), String> {
    let result_str = serde_json::to_string(result).unwrap_or_default();
    let mut missing = Vec::new();
    for name in expected {
        if !result_str.contains(name.as_str()) {
            missing.push(name.as_str());
        }
    }
    if missing.is_empty() {
        Ok(())
    } else {
        Err(format!("Missing symbols: {:?}", missing))
    }
}
```

**Step 2: Verify it compiles**

Run: `cargo check --features e2e-rust`
Expected: may need minor import adjustments — the exact public paths for `ListFunctions`, `SearchForPattern` need to match the crate's module structure. Adjust imports as needed.

**Step 3: Commit**

```bash
git add tests/e2e/harness.rs
git commit -m "feat(e2e): add test harness with fixture context and assertion runner"
```

---

## Task 10: Create per-language test files

**Files:**
- Create: `tests/e2e/test_rust.rs`
- Create: `tests/e2e/test_python.rs`
- Create: `tests/e2e/test_typescript.rs`
- Create: `tests/e2e/test_kotlin.rs`
- Create: `tests/e2e/test_java.rs`

**Step 1: Create tests/e2e/test_rust.rs**

```rust
use crate::e2e::harness::run_expectations;

#[tokio::test]
async fn core_rust() {
    run_expectations("rust", "core-expectations.toml").await;
}

#[tokio::test]
async fn rust_extensions() {
    run_expectations("rust", "rust-extensions.toml").await;
}
```

**Step 2: Create tests/e2e/test_python.rs**

```rust
use crate::e2e::harness::run_expectations;

#[tokio::test]
async fn core_python() {
    run_expectations("python", "core-expectations.toml").await;
}

#[tokio::test]
async fn python_extensions() {
    run_expectations("python", "python-extensions.toml").await;
}
```

**Step 3: Create tests/e2e/test_typescript.rs**

```rust
use crate::e2e::harness::run_expectations;

#[tokio::test]
async fn core_typescript() {
    run_expectations("typescript", "core-expectations.toml").await;
}

#[tokio::test]
async fn typescript_extensions() {
    run_expectations("typescript", "typescript-extensions.toml").await;
}
```

**Step 4: Create tests/e2e/test_kotlin.rs**

```rust
use crate::e2e::harness::run_expectations;

#[tokio::test]
async fn core_kotlin() {
    run_expectations("kotlin", "core-expectations.toml").await;
}

#[tokio::test]
async fn kotlin_extensions() {
    run_expectations("kotlin", "kotlin-extensions.toml").await;
}
```

**Step 5: Create tests/e2e/test_java.rs**

```rust
use crate::e2e::harness::run_expectations;

#[tokio::test]
async fn core_java() {
    run_expectations("java", "core-expectations.toml").await;
}

#[tokio::test]
async fn java_extensions() {
    run_expectations("java", "java-extensions.toml").await;
}
```

**Step 6: Wire e2e module into test harness**

The e2e module needs to be visible from a test entry point. Since `tests/integration.rs` already exists, create a new top-level test file:

Create: `tests/e2e_tests.rs`

```rust
mod e2e;
```

**Step 7: Verify compilation**

Run: `cargo check --features e2e-rust`
Expected: compiles (though tests won't pass until LSP is started)

**Step 8: Commit**

```bash
git add tests/e2e/ tests/e2e_tests.rs
git commit -m "feat(e2e): add per-language test files and wire e2e module"
```

---

## Task 11: Run and fix — Rust E2E tests

This is the integration step where we run the Rust E2E tests, fix import paths, adjust expectations to match actual LSP output, and iterate.

**Step 1: Run Rust E2E core tests**

Run: `cargo test --features e2e-rust core_rust -- --nocapture`
Expected: some failures — the assertion strings in expectations may not match exact LSP output.

**Step 2: Debug and fix mismatches**

For each failure:
1. Read the error message (shows actual vs expected)
2. Adjust TOML expectations OR fixture code to match
3. Re-run

Common issues to expect:
- Symbol names may include type parameters (e.g., `Catalog<T>` vs `Catalog`)
- Method names may appear as `impl Book/title` vs just `title`
- Constants may or may not be reported by tree-sitter vs LSP
- Enum variants may be nested differently

**Step 3: Run Rust E2E extension tests**

Run: `cargo test --features e2e-rust rust_extensions -- --nocapture`

**Step 4: Fix all failures and verify green**

Run: `cargo test --features e2e-rust -- --nocapture`
Expected: all PASS

**Step 5: Also verify existing tests still pass**

Run: `cargo test`
Expected: all ~419+ tests pass (no regression)

**Step 6: Commit**

```bash
git add -A
git commit -m "fix(e2e): adjust Rust fixture expectations to match actual LSP output"
```

---

## Task 12: Run and fix — Python, TypeScript, Kotlin, Java E2E tests

Repeat the same process as Task 11 for each language. Each language should be done separately so failures in one don't block others.

**Step 1: Python**

Run: `cargo test --features e2e-python -- --nocapture`
Fix mismatches, commit.

**Step 2: TypeScript**

Run: `cargo test --features e2e-typescript -- --nocapture`
Fix mismatches, commit.

**Step 3: Kotlin**

Run: `cargo test --features e2e-kotlin -- --nocapture`
Fix mismatches, commit. NOTE: kotlin-lsp takes ~30-60s to start, be patient.

**Step 4: Java**

Run: `cargo test --features e2e-java -- --nocapture`
Fix mismatches, commit. NOTE: jdtls also has slow startup.

**Step 5: Run all E2E tests together**

Run: `cargo test --features e2e -- --nocapture`
Expected: all pass

**Step 6: Final commit**

```bash
git add -A
git commit -m "fix(e2e): tune all language fixture expectations to match LSP output"
```

---

## Task 13: Verify full test suite and clean up

**Step 1: Run all tests (unit + integration + e2e)**

Run: `cargo test --features e2e`
Expected: all pass

**Step 2: Run clippy**

Run: `cargo clippy --features e2e -- -D warnings`
Expected: clean

**Step 3: Run fmt**

Run: `cargo fmt`
Expected: no changes (already formatted)

**Step 4: Verify existing tests aren't broken**

Run: `cargo test` (without e2e features)
Expected: all ~419+ tests pass

**Step 5: Final commit**

```bash
git add -A
git commit -m "chore(e2e): clean up and verify full test suite"
```
