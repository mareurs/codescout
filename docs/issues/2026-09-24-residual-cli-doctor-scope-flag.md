---
id: b161f5ed9b7bfbd9
kind: bug
status: open
title: 'RESIDUAL: Add the --scope flag to codescout doctor''s CLI now that the scanner selector is wired, and drop the declared omission'
tags:
- cluster/declared-not-wired
closed: null
opened: 2026-09-24
owner: marius
related:
- docs/issues/archive/2026-08-30-cli-doctor-exposes-no-fix-flag.md
- docs/issues/archive/2026-09-09-cli-doctor-passes-an-empty-args-map-so-no-fix-or-paging-is-reachable.md
severity: low
---

## Summary

Remaining work split out of `docs/issues/archive/2026-08-30-cli-doctor-exposes-no-fix-flag.md`, `docs/issues/archive/2026-09-09-cli-doctor-passes-an-empty-args-map-so-no-fix-or-paging-is-reachable.md`, whose `unverified:` caveat named it and was, until this file, the only surface reporting it. Routed here on 2026-09-24 so the open-bug query reaches it: the parent caveat now opens `TRACKED <this file's id>`, and `doctor`'s `terminal_status_with_caveat` reports the parent again if this file closes or moves without the work.

**The work:** Add the --scope flag to codescout doctor's CLI now that the scanner selector is wired, and drop the declared omission.

## Parent caveat, verbatim

`docs/issues/archive/2026-08-30-cli-doctor-exposes-no-fix-flag.md` (status `fixed`):

> `--scope` deliberately not exposed (declared omission) while 33e740960f758b6d is open. `to_tool_args` is doctor-local, so marshalling is still duplicated per subcommand — only the guard generalises. Tests are librarian-gated and absent from the lean lane.

`docs/issues/archive/2026-09-09-cli-doctor-passes-an-empty-args-map-so-no-fix-or-paging-is-reachable.md` (status `fixed`):

> Seven of the scanner's eight params are wired; `--scope` is deliberately omitted (declared in SCANNER_PARAMS_THE_CLI_OMITS) because its selector is still broken — 33e740960f758b6d. Tests are librarian-gated, so they do not run in the lean lane.


**Blocker since cleared.** Both parents omitted `--scope` only because its selector was
broken (`2026-09-09-doctor-accepts-a-scope-argument-and-never-reads-it`). That bug has since
been fixed and archived, which re-keyed its id — the quotes above are repointed to its
current id, `33e740960f758b6d` — so this residual is unblocked.

## Fix

Not started. The parent's § Fix and § Resume hold the design context; read them before acting, and re-check the caveat against HEAD first — it was written at the parent's closing and may have been overtaken since.

## References

- `docs/issues/archive/2026-08-30-cli-doctor-exposes-no-fix-flag.md` — parent
- `docs/issues/archive/2026-09-09-cli-doctor-passes-an-empty-args-map-so-no-fix-or-paging-is-reachable.md` — parent
