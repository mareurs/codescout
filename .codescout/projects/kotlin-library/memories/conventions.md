## Conventions

**Language:** Kotlin 2.1.0 on JVM. Build: Gradle Kotlin DSL (`build.gradle.kts`).

**Naming:**
- Classes/interfaces/objects: PascalCase (`Book`, `Catalog`, `SearchResult`, `BookRegistry`)
- Functions and properties: camelCase (`isAvailable`, `searchText`, `copiesAvailable`)
- Constants: camelCase at top level (`MAX_RESULTS`), not SCREAMING_SNAKE (deviates from Java convention)
- Enum entries: UPPER_SNAKE (`FICTION`, `NON_FICTION`)
- Packages: lowercase dotted (`library.models`, `library.services`)

**Immutability:**
- Data class properties are `val` by default
- Internal mutable state is private (`private val items = mutableListOf<T>()`)
- Public API exposes `List<T>` (read-only view), never `MutableList`

**Null safety:**
- Nullable returns typed explicitly (`Book?` for registry lookup)
- No `!!` usage in the fixture — absence is expressed as nullable return

**Error / precondition handling:**
- `require(condition) { message }` for preconditions (see `createNamedCatalog`)
- No exception throwing beyond `require`; errors modelled as sealed class variants (`SearchResult.Error`)

**Testing:**
- No test directory in this fixture — it is a navigation/parsing target, not a tested library

**Documentation:**
- All public members have KDoc (`/** ... */`)
- `createNamedCatalog` demonstrates multi-line KDoc with `@param` / `@return` tags
- Comments use `//` for inline code notes

**Kotlin-specific idioms present:**
- `@JvmInline value class` for zero-overhead wrappers (`ISBN`)
- `by lazy` for deferred property init (`LazyBook.formattedTitle`)
- `sealed class` for exhaustive result modeling (`SearchResult`)
- Top-level `object` for singleton services (`BookRegistry`)
- Extension functions on own and foreign types (`Book.toSearchText`, `Catalog<T>.searchAsync`)
- Scope functions (`let`, `copy`) for inline transformations
- Nested data class inside outer class (`Catalog.CatalogStats`)

**LSP note:**
The Kotlin LSP (`kotlin-language-server`) circuit-breaker is known to trip on this fixture
when another codescout instance targets the same project. If `symbols(include_body=true)`
fails with "circuit-breaker open", use `grep(pattern, path=...)` as fallback to read source.
See `docs/issues/2026-03-24-kotlin-lsp-concurrent-instances.md` in the code-explorer project.
