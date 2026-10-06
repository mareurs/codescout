---
id: '7c8a5ba864bc8398'
kind: bug
status: fixed
title: 'RESIDUAL: Report the count of param keys the probe skips (accepts_any_json / unlabelled) so the sweep''s coverage is visible'
tags:
- cluster/guard-narrower-than-its-name
closed: 2026-10-06
opened: 2026-09-24
owner: marius
related:
- docs/issues/archive/2026-09-02-param-probe-reads-only-the-first-slash-token-so-later-actions-are-unswept.md
severity: low
---

## Summary

Remaining work split out of `docs/issues/archive/2026-09-02-param-probe-reads-only-the-first-slash-token-so-later-actions-are-unswept.md`, whose `unverified:` caveat named it and was, until this file, the only surface reporting it. Routed here on 2026-09-24 so the open-bug query reaches it: the parent caveat now opens `TRACKED <this file's id>`, and `doctor`'s `terminal_status_with_caveat` reports the parent again if this file closes or moves without the work.

**The work:** Report the count of param keys the probe skips (accepts_any_json / unlabelled) so the sweep's coverage is visible.

## Parent caveat, verbatim

`docs/issues/archive/2026-09-02-param-probe-reads-only-the-first-slash-token-so-later-actions-are-unswept.md` (status `fixed`):

> The prescribed second half (emit `checked N of M labelled pairs`) was judged obviated by the parser fix rather than implemented — reasoning in the Fix section. Residue: keys skipped for `accepts_any_json` or for carrying no `<action>:` label remain uncounted anywhere.

## Fix

The sweep now reports the keys it skips and each call site pins them; it does not probe them. `Sweep` in `src/tools/param_probe.rs` gained `skipped_any_json` (keys passed over because the call site lists them in `accepts_any_json`, nested dotted paths included, each reported once) and `unlabelled` (top-level keys that got no probe because they have no description or because no label token names a dispatched action). `assert_all_honored` takes a new per-call-site `unlabelled_pin` argument and asserts exact set equality with `unlabelled`, so a newly unlabelled key and a stale pinned entry are each red. It also reconciles `accepts_any_json` against the keys actually skipped for it, and prints the coverage counts in its failure messages. The three call sites pin what the sweep measured on 2026-10-05: `doc` (`src/librarian/tools/artifact.rs`) `[]`, `library` (`src/tools/library.rs`) `[]`, and `librarian` (`src/librarian/tools/librarian.rs`) 13 keys: `actor`, `confirm`, `export`, `new_root`, `old_root`, `op`, `prune_before_ms`, `root`, `row_id`, `since`, `tbl`, `until`, `write`. That list is recorded unguarded debt, not an approval.

## Tests added

All in the test module of `src/tools/param_probe.rs`, on a fixture schema with one skip of each kind:

- `skipped_keys_are_reported_by_kind`: a key with no description, a prose label and a label naming no dispatched action land in `unlabelled`; an `accepts_any_json` key lands in `skipped_any_json`; `action` and a key with one matching label token are not skips.
- `a_fully_labelled_schema_reports_no_skipped_keys`: the positive twin, so the two fields are not simply always non-empty.
- `a_nested_any_json_skip_is_reported_once`: a nested `event.payload` admission is reported once although the key is labelled for two actions.
- `assert_all_honored_accepts_a_matching_pin`: the entry point passes with a pin that matches the sweep.
- `assert_all_honored_rejects_a_newly_unlabelled_key`: a skipped key that is not pinned reds (`#[should_panic]` on the message).
- `assert_all_honored_rejects_a_stale_pin`: a pinned key the sweep now probes reds.
- `assert_all_honored_rejects_a_stale_admission`: an `accepts_any_json` entry naming a key the sweep never skipped reds.

The three call sites' pins are asserted by their existing sweep tests (`doc`, `library`, `librarian`), which now take the extra argument. No mutation run is recorded in the commit message.

## Fix provenance

- **SHA:** `40add6c4` (`experiments`)
- **patch-id:** `a2e96f72254883d1d273383f5bd83742b71fbe3a`

## Resume

Closed on 2026-10-06: the skipped keys are now reported and pinned, which is what this file asked for. Residual follow-ups, listed and not filed:

- The 13 pinned `librarian` keys are real unguarded keys. The whole `audit_log` action is missing from that site's `spec.actions`, so its 8 `audit_log:`-labelled keys (`actor`, `export`, `op`, `prune_before_ms`, `row_id`, `since`, `tbl`, `until`) are unswept. `write`, `root`, `confirm`, `old_root` and `new_root` carry prose labels (`legibility_scan (default true): ...`, `doctor fix=...`, `For fix=rehome:`) that name no action. Clearing an entry means relabelling the key to a bare `<action>:` or adding the action to the spec with a `required` arm, which also moves `floor`.
- The `librarian` site's `floor` is 28 (read 2026-09-09); the sweep was reported to measure 30 at this commit. The floor was not set at the current measurement, which narrows the margin that exists to catch a lost label. This count was not re-measured while closing this file.
- A partially mismatched label such as `alpha/typo` is not recorded: the key counts as matched as soon as one token names a dispatched action, so the stray token is ignored silently.
- The source comment at `src/librarian/tools/librarian.rs` says "Ten are labelled `audit_log:`"; the schema carries eight keys with that label.

## References

- `docs/issues/archive/2026-09-02-param-probe-reads-only-the-first-slash-token-so-later-actions-are-unswept.md` — parent
