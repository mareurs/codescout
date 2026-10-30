---
id: '78d98432e98a15e5'
kind: bug
status: open
title: 'RESIDUAL: Decide on and, if accepted, remediate the ~5,566 host_previous_stranded_rows so they re-emit under the corrected host id'
tags:
- cluster/authorship-unrecoverable-after-the-fact
closed: null
opened: 2026-09-24
owner: marius
related:
- docs/issues/archive/2026-09-04-a-transported-catalog-carries-its-host-identity.md
severity: low
---

## Summary

Remaining work split out of `docs/issues/archive/2026-09-04-a-transported-catalog-carries-its-host-identity.md`, whose `unverified:` caveat named it and was, until this file, the only surface reporting it. Routed here on 2026-09-24 so the open-bug query reaches it: the parent caveat now opens `TRACKED <this file's id>`, and `doctor`'s `terminal_status_with_caveat` reports the parent again if this file closes or moves without the work.

**The work:** Decide on and, if accepted, remediate the ~5,566 host_previous_stranded_rows so they re-emit under the corrected host id.

## Parent caveat, verbatim

`docs/issues/archive/2026-09-04-a-transported-catalog-carries-its-host-identity.md` (status `fixed`):

> The audit_open_gaps question is UNCHANGED and still not established: whether the several-hundred-entry gap list was caused by cross-host sequence interleaving was never proven, and gaps have other documented causes (prune markers, rolled-back transactions burning seq). The fix stops future interleaving; it does not explain the existing gaps and no measurement here attributes them. Also still unestablished: whether any OTHER host merged rows into this shard, which would make the mixing bidirectional. AND NEW, created by the fix: the ~5,566 rows already exported under ripper-65e654 stay counted as exported by the per-repo watermark and will never re-emit under the corrected id without a deliberate rollback -- doctor reports them as host_previous_stranded_rows, but nothing remediates them and no one has decided whether to.

## Fix

Not started. Re-checked 2026-10-30: `doctor` reports `host_previous_stranded_rows` only when the catalog carries a `host_previous` id (`src/librarian/tools/doctor.rs`, near line 1342). A `doctor` run on this machine returned no such field (0 matches in the report), so the count cannot be read here. The ~5,566 figure belongs to the machine that holds the transported catalog; whether it is still that number, and which machine that is, was not verified. The parent's § Fix and § Resume hold the design context; read them before acting, and re-check the caveat against HEAD first — it was written at the parent's closing and may have been overtaken since.

## References

- `docs/issues/archive/2026-09-04-a-transported-catalog-carries-its-host-identity.md` — parent
