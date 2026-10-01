---
id: 98d001d81c241245
kind: bug
status: fixed
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

`codescout doctor --scope <project|repo|umbrella|all>` now exists. It is validated by clap at the command line (a typo is refused there and names what is accepted) and sent to the scanner only when set, because the scanner owns its default and a value sent here would assert a scope the caller never chose. `SCANNER_PARAMS_THE_CLI_OMITS` is kept, empty, as the mechanism for the next omission.

Re-checked against HEAD first, as this file asked: the blocker (the scanner never read its own `scope`) was fixed and archived, so the omission's stated reason was false while every test stayed green, because `every_declared_omission_names_a_real_param_and_gives_a_reason` checks that a reason EXISTS, never that it is still TRUE.

RED first: emptying the omission list made `every_scanner_param_is_reachable_from_the_cli_or_named_as_omitted` fail, naming exactly `scope`. 5 mutations, 5 killed (emission dropped, emitted value hardcoded, unset flag defaulted, `value_parser` removed, `all` missing from the accepted list). The flag is tested through a real clap parse, not a hand-built struct.

## References

- `docs/issues/archive/2026-08-30-cli-doctor-exposes-no-fix-flag.md` — parent
- `docs/issues/archive/2026-09-09-cli-doctor-passes-an-empty-args-map-so-no-fix-or-paging-is-reachable.md` — parent


## Fix provenance

- **SHA:** `49d08af335a1caad21c2c7d2e9b067bf65033724` (`experiments`)
- **patch-id:** `6665082ae5cbe264bdeafe4a85186a697947415c`
