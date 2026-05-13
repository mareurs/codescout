# Multi-Ecosystem Library Auto-Registration


## Overview

When `workspace(action: activate)` runs, codescout now automatically detects and registers dependencies
from five ecosystems:

| Ecosystem | Manifest | Source Location |
|-----------|----------|-----------------|
| **Rust** | `Cargo.toml` | `~/.cargo/registry/src/` |
| **Node/TypeScript** | `package.json` | `node_modules/` |
| **Python** | `pyproject.toml` / `requirements.txt` | `.venv/lib/pythonX.Y/site-packages/` |
| **Go** | `go.mod` | `$GOMODCACHE` (via `go env`) |
| **Java/Kotlin** | `build.gradle.kts` / `build.gradle` / `pom.xml` | *(no local source)* |

## How It Works

1. **Discovery** — each ecosystem's manifest file is parsed to extract dependency names.
   Only production dependencies are included (dev/test dependencies are skipped).

2. **Source location** — for each dependency, codescout checks whether local source code
   exists (e.g., in `node_modules/`, the Cargo registry, or a Python venv).

3. **Registration** — dependencies are batch-registered with `DiscoveryMethod::ManifestScan`.
   The `source_available` flag indicates whether the source was found locally.

4. **Precedence** — `ManifestScan` never overwrites `Manual` or `LspFollowThrough`
   registrations. If you've manually registered a library, auto-scan won't touch it.

## Source Availability

Libraries without local source (common for JVM dependencies) are registered with
`source_available: false`. When an agent tries to use symbol tools or semantic search on
such a library, they receive a `RecoverableError` with a hint:

```
Library source code is not available locally for: jackson-databind
Hint: To browse library source, download it using the project's build tool
(e.g. ./gradlew dependencies, mvn dependency:sources), then call
register_library(name, "/path/to/source", language) and retry.
```

## Output

The `workspace(action: activate)` response includes auto-registered libraries:

```json
{
  "auto_registered_libs": [
    {"name": "express", "language": "javascript", "source_available": true},
    {"name": "guava", "language": "java", "source_available": false}
  ]
}
```

The compact output shows: `activated · myproject · auto-registered 5 libs (2 without source)`

`list_libraries` now includes `source_available` for each entry.

## Python Name Normalization

Python package names are normalized per PEP 503: lowercased, with runs of `-`, `_`, `.`
collapsed to a single `_`. This matches how pip stores packages in `site-packages/`.

## Go Module Cache Encoding

Go module paths with uppercase letters are encoded per Go conventions: `Azure` → `!azure`.
This ensures correct lookups in `$GOMODCACHE`.
