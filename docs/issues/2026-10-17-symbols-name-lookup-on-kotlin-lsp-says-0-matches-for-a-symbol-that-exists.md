---
kind: bug
status: open
title: 'BUG: symbols(name=...) on a Kotlin file answers "0 matches" for a symbol that exists, then finds it on the next identical call'
tags:
- cluster/unclassified
opened: 2026-10-17
owner: marius
related:
- docs/issues/2026-10-16-mcp-smoke-kotlin-targets-a-project-layout-that-no-longer-exists.md
severity: low
---

# BUG: symbols(name=...) on a Kotlin file answers "0 matches" for a symbol that exists, then finds it on the next identical call

## Summary

Against a fresh copy of `tests/fixtures/kotlin-library`, `symbols` with `name: "search"` and `path: ".../Catalog.kt"` answered `0 matches` once and found `Catalog/search` on the same call a few seconds later. The empty answer is a negative result that does not say it came from a server that had not settled. Found 2026-10-17 by the first live run of `tests/mcp-smoke-kotlin.sh`.

## Class fit

No existing class was checked beyond a read of the roster headings; the closest reading is a failure rendered as an answer, but the roster was not walked class by class, so the sanctioned `cluster/unclassified` is used rather than a forced fit. What the case shows: a not-ready dependency (the Kotlin language server) is reported as an authoritative empty result.

## Symptom (Effect)

Measured 2026-10-17 with the release build of this branch, one `mcp call` per request against one alias, a project copy made with `git archive HEAD tests/fixtures/kotlin-library`:

```
overview 10:41:17   -> Class 7-25 Catalog, members items, CatalogStats, add, search
name try 1 10:41:20 -> 0 matches
name try 2 10:41:26 -> Method 20-21 Catalog/search
name try 3..6       -> Method 20-21 Catalog/search
```

It did not happen on every start: a different fresh project whose first call was the name lookup found `Catalog/search` on the first try (10:39:26). In the second full script run (overview call first) the first name lookup after it failed, and the following one passed; in the first full run the same lookup failed too (its next lookup also failed, but an assertion there was unsatisfiable on its own, see the sibling issue on `include_body`). The cause was not established; "it is the first name lookup after another call" fits three observations and is not shown.

## Reproduction

```
S=$(mktemp -d); mkdir -p $S/p $S/tmp
git archive HEAD tests/fixtures/kotlin-library | tar -x --strip-components=3 -C $S/p
export TMPDIR=$S/tmp
mcp alias add kt-repro ./target/release/codescout start --project $S/p
mcp call symbols -p '{"path": "src/main/kotlin/library/services/Catalog.kt"}' kt-repro
mcp call symbols -p '{"name": "search", "path": "src/main/kotlin/library/services/Catalog.kt"}' kt-repro   # 0 matches (on the runs above)
sleep 3
mcp call symbols -p '{"name": "search", "path": "src/main/kotlin/library/services/Catalog.kt"}' kt-repro   # Catalog/search
mcp alias remove kt-repro
```

## Environment

This machine, `kotlin-lsp` at `/usr/bin/kotlin-lsp`, codescout release build of branch `worktree-agent-a2ef1acc5c69d1c42`.

## Root cause

Not investigated.

## Evidence

The timed output above; `docs/manual/src/troubleshooting.md:124-127` already tells the reader that a Kotlin server "may still be initializing" and to wait and retry, so the behaviour is known to users but the tool's answer does not say so.

## Hypotheses tried

None.

## Fix

Unknown. See `docs/adrs/2026-08-27-negative-results-name-their-scope.md` for what a negative result from this tool should state.

## Tests added

N/A: open. `tests/mcp-smoke-kotlin.sh` retries a `0 matches` reply up to five times (3s apart) and says so in a comment, which hides this from the smoke run on purpose.

## Workarounds

Repeat the call.

## Resume

1. Reproduce with the commands above and capture the mux log for the `0 matches` call. 2. Find whether `symbols` knew the server was not ready.

## References

- `tests/mcp-smoke-kotlin.sh` (`call`)
- `docs/manual/src/troubleshooting.md`
