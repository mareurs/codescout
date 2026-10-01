---
id: '80a62d2b87683852'
kind: bug
status: archived
title: 'RESIDUAL: Derive MECHANISM_TOOLS in scripts/probe_guide_section_use.py from the served tool registry instead of a hand list'
tags:
- cluster/selector-narrower-than-its-population
claimed_at: 2026-09-27
claimed_by: 48d1f0c8-9f60-43bb-a15e-17ec7995813a
closed: 2026-09-27
opened: 2026-09-24
owner: marius
related:
- docs/issues/archive/2026-09-03-probe-mechanism-filter-omits-the-renamed-doc-tool.md
severity: low
---

## Summary

Remaining work split out of `docs/issues/archive/2026-09-03-probe-mechanism-filter-omits-the-renamed-doc-tool.md`, whose `unverified:` caveat named it and was, until this file, the only surface reporting it. Routed here on 2026-09-24 so the open-bug query reaches it: the parent caveat now opens `TRACKED <this file's id>`, and `doctor`'s `terminal_status_with_caveat` reports the parent again if this file closes or moves without the work.

**The work:** Derive MECHANISM_TOOLS in scripts/probe_guide_section_use.py from the served tool registry instead of a hand list.

## Tests added

`tests/test_probe_mechanism_tools_registry.py` — three cases against `mechanism_tools_registry_problems()`, a pure function taking the live set **as data** so no case spawns a process. **Re-run by this ledger rather than taken on report: 3 passed, exit 0.**

- **existence**: a live set missing `doc` with no collisions → exactly one problem naming `doc`, replaying the parent bug's own failure (the 2026-09-02 `artifact` → `doc` collapse).
- **boundedness**: `doc` and `librarian` present *plus* `docs_export` → exactly one problem naming the unintended substring match.
- **control**: today's real 21-tool registry → **zero** problems. Without this row the other two would pass on a predicate that flags everything.

The ceiling is annotated **at the site** — in the function's own docstring and above the new constant — not left to a report: membership judgement *"has no registry surface to derive from"*. The constant is also deliberately not named `*_TOOLS`, to avoid opening a second block under the Rust static-scan gate; that reasoning is written where the next editor will read it.

## Parent caveat, verbatim

`docs/issues/archive/2026-09-03-probe-mechanism-filter-omits-the-renamed-doc-tool.md` (status `fixed`):

> No regression test. `scripts/` has no test harness in this repo, so nothing gates `MECHANISM_TOOLS` against the next rename — the same defect can recur exactly as it did here, and the fix's evidence is an observed before/after rather than a guard. The union is also unbounded in principle: a THIRD name would go undetected the same way, and the durable remedy named in the Fix section (derive the list from the served registry) is NOT implemented. Separately, the documented `"doc"` substring greediness is recorded at the site but not enforced — a future `mcp__codescout__docs_*` tool would be silently counted as mechanism operation, and only a reader of the comment would know.

## Fix

Not started. The parent's § Fix and § Resume hold the design context; read them before acting, and re-check the caveat against HEAD first — it was written at the parent's closing and may have been overtaken since.

**Done 2026-09-27, and the honest answer is a PARTIAL derivation with its ceiling named.**

**The registry is enumerable.** The MCP server answers a standard `tools/list` JSON-RPC call over stdio — the transport `scripts/probe_tool_surface.py`'s `fetch_tools(binary)` already uses, reused rather than re-implemented. Against `target/debug/codescout` today it returns **21 live tools**.

**Two of the three axes are now derived; the third cannot be, and that is a property of the type rather than of the effort.**

| axis | derivable? | now checked against the registry |
|---|---|---|
| **existence** — do the hand-claimed names still exist? | yes | yes |
| **boundedness** — does any *unintended* live tool satisfy the substring test? | yes | yes |
| **membership** — which names conceptually *belong* in `MECHANISM_TOOLS`? | **no** | n/a |

**Membership has no registry surface, verified at the type rather than taken on report.** `Tool` (`src/tools/core/types.rs:1067`) declares **19 methods** and not one is a category or kind: the closest are `is_write` (a boolean, and input-dependent) and `annotations` (MCP's `ToolAnnotations`, not a codescout classification). So the literal tuple stays hand-written — but it is now continuously checked against the thing it claims to describe, which is the whole of what the registry can answer.

**Contrast worth keeping, because the two outcomes look alike and are not.** This campaign hit the same shape at `ea152af988811fa1` and derivation was impossible there — two of three bounds were constants **local to their test functions**, one named `CEILING` with no scannable marker — so a hand list with an honest ceiling was the entire available answer. Here the population had a wire surface, so two axes moved from recited to derived and only the judgement stayed manual. *"Hand list plus annotation"* is the right outcome in one case and a failure to look in the other; the discriminator is whether the population has any enumerable form at all, and it must be checked rather than assumed.

**What HEAD had already overtaken, and what it had not.** `src/server.rs`'s `provenance_probes_reference_only_real_tool_names` (added `cc160413`) already mutation-tests *"drop `doc` from `MECHANISM_TOOLS`"* → killed, which closes the parent's *"a third name could go undetected"* worry for **static source drift**. It cannot catch a **live-registry rename**, which is the residual's actual complaint — so the remedy was genuinely still unimplemented, confirmed by reading the file before editing.

## Fix provenance

- **SHA:** `2d70c7c2` (experiments-only) — positional; dies on a rebase of `experiments`.
- **patch-id:** `2cd28af96008f7d9e11734a28009953922ff3f28` — content hash of the diff; survives rebase and cherry-pick. Derived through a file, never a pipe from `git show`.

## References

- `docs/issues/archive/2026-09-03-probe-mechanism-filter-omits-the-renamed-doc-tool.md` — parent
