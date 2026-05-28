# java-library Conventions

## Language & Style
- **Java 21** — uses modern features: records, sealed interfaces, pattern matching (`instanceof`)
- All source in package `library.*` (root group `library`)
- Public API uses Javadoc (`/** ... */`) on every type and method
- Package-private by default for non-public classes (e.g. `BookProcessor` is package-private)
- Public interfaces and record types are the primary API surface

## Naming
- Types: PascalCase (`Book`, `Catalog`, `SearchResult`, `BookProcessor`)
- Methods: camelCase (`searchText`, `isAvailable`, `createDefault`, `processAll`)
- Constants: SCREAMING_SNAKE_CASE (`MAX_RESULTS`)
- Packages: lowercase, dot-separated (`library.models`, `library.services`, etc.)
- Enum values: SCREAMING_SNAKE_CASE (`FICTION`, `NON_FICTION`)

## Type Modeling Conventions
- Immutable value types → Java records (Book, Found, NotFound, Error)
- Result/discriminated unions → sealed interfaces with record subtypes
- Service/logic containers → regular classes with generics when needed
- Enumerated domains → enum with helper methods
- Reusable abstractions → interfaces with default methods for optional extensions

## Annotation Convention
- Custom annotations defined with `@interface` in the same package that uses them
- Runtime retention (`@Retention(RetentionPolicy.RUNTIME)`) for annotations intended for tooling
- Annotation placed on the method, not the class

## Generics Convention
- Bounded type parameters use `T extends <Interface>` (not raw types)
- Wildcard `? extends` for read-only consumer parameters (`processAll`)
- Static factory methods avoid repetition: `Catalog.createDefault()` returns typed `Catalog<Book>`

## Testing Approach
- **No test sources present** in this fixture — it is a codescout test target, not a tested library
- Tests (if they existed) would live in `src/test/java/library/`
- Gradle `id 'java'` plugin supports JUnit; no test framework is configured

## Build Conventions
- Single `build.gradle` with no dependencies block (JDK only)
- `settings.gradle` sets `rootProject.name = 'java-library'`
- No Gradle wrapper committed — caller provides Gradle
- Source and target compatibility locked to Java 21
