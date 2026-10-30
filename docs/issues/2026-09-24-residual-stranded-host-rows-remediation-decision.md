---
id: '78d98432e98a15e5'
kind: bug
status: wontfix
title: 'RESIDUAL: Decide on and, if accepted, remediate the ~5,566 host_previous_stranded_rows so they re-emit under the corrected host id'
tags:
- cluster/authorship-unrecoverable-after-the-fact
closed: 2026-10-30
opened: 2026-09-24
owner: marius
related:
- docs/issues/archive/2026-09-04-a-transported-catalog-carries-its-host-identity.md
severity: low
unverified: 'Not read: the stranded-row count (it lives in the laptop''s catalog), the 99.2% session-attribution figure (taken from the parent, 2026-09-06, not re-measured), and how a merged audit read treats duplicated rows. The line counts above are wc -l of this checkout''s shards on 2026-10-30.'
---

## Summary

Remaining work split out of `docs/issues/archive/2026-09-04-a-transported-catalog-carries-its-host-identity.md`, whose `unverified:` caveat named it and was, until this file, the only surface reporting it. Routed here on 2026-09-24 so the open-bug query reaches it: the parent caveat now opens `TRACKED <this file's id>`, and `doctor`'s `terminal_status_with_caveat` reports the parent again if this file closes or moves without the work.

**The work:** Decide on and, if accepted, remediate the ~5,566 host_previous_stranded_rows so they re-emit under the corrected host id.

## Parent caveat, verbatim

`docs/issues/archive/2026-09-04-a-transported-catalog-carries-its-host-identity.md` (status `fixed`):

> The audit_open_gaps question is UNCHANGED and still not established: whether the several-hundred-entry gap list was caused by cross-host sequence interleaving was never proven, and gaps have other documented causes (prune markers, rolled-back transactions burning seq). The fix stops future interleaving; it does not explain the existing gaps and no measurement here attributes them. Also still unestablished: whether any OTHER host merged rows into this shard, which would make the mixing bidirectional. AND NEW, created by the fix: the ~5,566 rows already exported under ripper-65e654 stay counted as exported by the per-repo watermark and will never re-emit under the corrected id without a deliberate rollback -- doctor reports them as host_previous_stranded_rows, but nothing remediates them and no one has decided whether to.

## Fix

Not started. Re-checked 2026-10-30: `doctor` reports `host_previous_stranded_rows` only when the catalog carries a `host_previous` id (`src/librarian/tools/doctor.rs`, near line 1342). A `doctor` run on this machine returned no such field (0 matches in the report), so the count cannot be read here. The ~5,566 figure belongs to the machine that holds the transported catalog; whether it is still that number, and which machine that is, was not verified. The parent's § Fix and § Resume hold the design context; read them before acting, and re-check the caveat against HEAD first — it was written at the parent's closing and may have been overtaken since.

## Decision

DECIDED BY THE USER ON 2026-10-30: option A, accept the loss. No remediation. Options offered: A accept and close as `wontfix`; B roll the laptop's watermark back and re-export under `archlinux-d9b5c3`; C keep it parked.

**Why A.** Machine attribution of the laptop's old rows is gone for good. Session attribution survives: the parent measured on 2026-09-06 that 99.2% of 27,297 rows carried a session actor (not re-measured). The severity is low. B is a manual operator step on another machine and re-emits everything after the rollback point; I did not check how a merged audit read treats rows that a re-export would duplicate.

**Checked on this machine on 2026-10-30 (the workstation).**

- `~/.local/share/librarian/catalog.db` holds `audit_host_id = ripper-65e654` and no `audit_host_id_previous`. `doctor` therefore cannot report `host_previous_stranded_rows` here, and my run contained no such field.
- Line counts in `.codescout/audit/`: `archlinux-d9b5c3-202609` 84, `ripper-1848b7-202609` 5,073, `ripper-65e654-202609` 69,737, `ripper-65e654-202610` 17,175. The parent measured the `ripper-65e654-202609` shard at 27,297 rows on 2026-09-06, so the mixed rows are a smaller share now.
- The stranded count itself lives in the laptop's catalog and was not read.

**Reopen if** someone needs machine attribution of the old rows, or if a merged audit read is found to misattribute them.

## References

- `docs/issues/archive/2026-09-04-a-transported-catalog-carries-its-host-identity.md` — parent
