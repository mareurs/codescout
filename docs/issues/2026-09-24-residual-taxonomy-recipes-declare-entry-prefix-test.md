---
id: '5820a75840dd2d52'
kind: bug
status: open
title: 'RESIDUAL: Write the doc-to-code test asserting every tracker with an append_entry recipe in docs/TAXONOMY.md declares entry_prefix'
tags:
- cluster/doc-contradicted-by-code
closed: null
opened: 2026-09-24
owner: marius
related:
- docs/issues/archive/2026-09-02-two-trackers-have-no-open-append-path.md
severity: low
---

## Summary

Remaining work split out of `docs/issues/archive/2026-09-02-two-trackers-have-no-open-append-path.md`, whose `unverified:` caveat named it and was, until this file, the only surface reporting it. Routed here on 2026-09-24 so the open-bug query reaches it: the parent caveat now opens `TRACKED <this file's id>`, and `doctor`'s `terminal_status_with_caveat` reports the parent again if this file closes or moves without the work.

**The work:** Write the doc-to-code test asserting every tracker with an append_entry recipe in docs/TAXONOMY.md declares entry_prefix.

## Parent caveat, verbatim

`docs/issues/archive/2026-09-02-two-trackers-have-no-open-append-path.md` (status `fixed`):

> The PRECONDITION is verified at the bytes; the ALLOCATION is not. Both files now declare entry_prefix (T / I) and a matching entry_high_water, read back from disk. But no append_entry was run against either, because a successful call allocates a real id and writes a real entry, and that is a content decision rather than a probe. So the claim 'append_entry now works here' rests on allocate_entry_id's frontmatter check being the only thing that was failing — read at augmentation.rs:971-983, not observed. The next person to append verifies it for free; if it still refuses, the cause is downstream of the declaration and this record is reopened rather than re-derived. No regression test either: the durable form is a doc-to-code join asserting that every tracker with an append_entry recipe in docs/TAXONOMY.md declares an entry_prefix, which is IC-11's mechanizable sub-shape and is not written.

## Fix

Not started. The parent's § Fix and § Resume hold the design context; read them before acting, and re-check the caveat against HEAD first — it was written at the parent's closing and may have been overtaken since.

## References

- `docs/issues/archive/2026-09-02-two-trackers-have-no-open-append-path.md` — parent
