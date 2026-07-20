---
kind: bug
status: fixed
title: 'BUG: strip_project_root omits the bare project root (no trailing slash) emitted by format_list_dir, leaking the absolute root in list_dir output'
tags:
- tree
- path-stripping
- server
closed: 2026-07-20
---

## Summary

`strip_project_root_from_result` (`src/server.rs:~1555`) strips the project root
from tool output only when it appears as `<root>/` (trailing slash required — it
matches at a value boundary followed by `/`). But `format_list_dir`
(`src/tools/tree.rs:~255`) emits the **bare** root without a trailing slash when
the root is the sole common prefix of the listing — e.g. `"<root> — N entries"`.
That bare form is not stripped, so the absolute project root leaks into
`list_dir` output.

## Why it's a bug

Path stripping exists so absolute machine paths don't leak into agent-visible
tool output (privacy + prompt-noise). The bare-root case defeats it for a common
shape (`list_dir` on a project root that has ≥1 visible top-level entry).

## Discovered

2026-07-18, during the catalog-hygiene branch (`experiments`). Latent until now:
the server test `call_tool_strips_project_root_from_output` used
`tests::make_server`, whose tempdir project root had **no visible top-level
files** (only a hidden `.codescout/` dir, which `list_dir` skips via
`.hidden(true)`), so `format_list_dir` returned `"(empty directory)"` and never
exercised the bare-root prefix. Isolating `make_server`'s catalog (temp-write
prevention work) briefly placed `librarian.db` at the visible root, which
surfaced the gap; the catalog was then relocated under `.codescout/` to keep the
branch scoped, leaving this pre-existing stripper gap untouched for a dedicated
fix.

## Fix directions

- Preferred: make `strip_project_root_from_result` also strip the bare root at a
  value boundary (end-of-value or followed by whitespace/` — `), reusing the
  existing boundary-safety the trailing-slash path already has (see
  `strip_prefix_only_at_value_boundary` / `strip_prefix_not_inside_longer_path`
  tests) so it doesn't over-strip a root that is a prefix of a longer path.
- Alternative: have `format_list_dir` keep the trailing slash on the root header.
- Add a regression test: `list_dir` on a root with exactly one visible top-level
  entry, assert the absolute root does not appear in the stripped output.

## Status log

- 2026-07-18 — opened; latent stripper gap surfaced (then side-stepped) by the
  catalog-hygiene make_server isolation. Not fixed here (out of scope — tree/
  server stripping, unrelated to catalog hygiene).
- 2026-07-19 — fixed at `e68f43ae` (branch `experiments`). Implemented the
  preferred direction: `strip_prefix_from_text` now also matches the bare root
  (no trailing slash), gated by an explicit right-boundary check so it can't
  over-strip a root that's a prefix of a longer, unrelated path (e.g.
  `<root>-backup/foo`). Regression test
  `call_tool_strips_bare_project_root_from_list_dir_output` writes a visible
  top-level file first (the existing strip test's tempdir has none, so it
  never reached this branch). Kept `open`->`fixed` in `docs/issues/`, not yet
  archived — waits for the fix to ship to `master`.
