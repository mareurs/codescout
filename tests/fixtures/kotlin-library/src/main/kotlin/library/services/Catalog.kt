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

/**
 * Multi-line KDoc with preamble text before params.
 *
 * This function exists to test whether kotlin-language-server includes
 * the full docstring (from the opening comment marker) in the symbol's `range.start`,
 * or only starts the range at the first @param line.
 *
 * @param name The catalog name to use
 * @param maxItems Maximum number of items allowed
 * @return A new Catalog instance
 */
fun createNamedCatalog(name: String, maxItems: Int = 100): Catalog<Book> {
    require(maxItems > 0) { "maxItems must be positive" }
    return Catalog(name)
}

/** Extension: suspend function (coroutine). */
suspend fun <T : Searchable> Catalog<T>.searchAsync(query: String): List<T> =
    search(query)

/** Extension: extension function on Book. */
fun Book.toSearchText(): String = "$title ($isbn)"
