# java-library — Project Overview

A minimal Java library fixture used as a test fixture for codescout's symbol
navigation and LSP tooling. It models a simple book catalog domain with modern
Java 21 language features: records, sealed interfaces, generics, annotations,
and anonymous classes.

## Purpose

This is NOT a runnable application — it is a test fixture that exercises
tree-sitter and LSP symbol extraction for Java. No tests, no main entry point.

## Tech Stack

- Language: Java 21
- Build: Gradle (plugin: 'java')
- Group: library, Version: 0.1.0
- Source compatibility: JavaVersion.VERSION_21

## Package Structure

```
library/
  models/      — Book (record), Genre (enum)
  interfaces/  — Searchable (interface)
  services/    — Catalog<T> (generic class + nested CatalogStats)
  extensions/  — @Indexed annotation, BookProcessor, SearchResult (sealed)
```

## Key Dependencies

None beyond the JDK. Pure Java, no external libraries.

## Runtime Requirements

No runtime — used as a compilation fixture only. Build with `./gradlew build`.
