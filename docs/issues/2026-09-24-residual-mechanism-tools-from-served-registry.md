---
id: b36761478b125b35
kind: bug
status: open
title: 'RESIDUAL: Derive MECHANISM_TOOLS in scripts/probe_guide_section_use.py from the served tool registry instead of a hand list'
tags:
- cluster/selector-narrower-than-its-population
closed: null
opened: 2026-09-24
owner: marius
related:
- docs/issues/archive/2026-09-03-probe-mechanism-filter-omits-the-renamed-doc-tool.md
severity: low
---

## Summary

Remaining work split out of `docs/issues/archive/2026-09-03-probe-mechanism-filter-omits-the-renamed-doc-tool.md`, whose `unverified:` caveat named it and was, until this file, the only surface reporting it. Routed here on 2026-09-24 so the open-bug query reaches it: the parent caveat now opens `TRACKED <this file's id>`, and `doctor`'s `terminal_status_with_caveat` reports the parent again if this file closes or moves without the work.

**The work:** Derive MECHANISM_TOOLS in scripts/probe_guide_section_use.py from the served tool registry instead of a hand list.

## Parent caveat, verbatim

`docs/issues/archive/2026-09-03-probe-mechanism-filter-omits-the-renamed-doc-tool.md` (status `fixed`):

> No regression test. `scripts/` has no test harness in this repo, so nothing gates `MECHANISM_TOOLS` against the next rename — the same defect can recur exactly as it did here, and the fix's evidence is an observed before/after rather than a guard. The union is also unbounded in principle: a THIRD name would go undetected the same way, and the durable remedy named in the Fix section (derive the list from the served registry) is NOT implemented. Separately, the documented `"doc"` substring greediness is recorded at the site but not enforced — a future `mcp__codescout__docs_*` tool would be silently counted as mechanism operation, and only a reader of the comment would know.

## Fix

Not started. The parent's § Fix and § Resume hold the design context; read them before acting, and re-check the caveat against HEAD first — it was written at the parent's closing and may have been overtaken since.

## References

- `docs/issues/archive/2026-09-03-probe-mechanism-filter-omits-the-renamed-doc-tool.md` — parent
