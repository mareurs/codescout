---
status: open
opened: 2026-10-17
closed:
severity: low
owner: marius
related:
- docs/issues/2026-10-17-p50-emission-budget-fixture-never-archives-a-bug-file-so-the-largest-doc-move-delivery-is-unbudgeted.md
- docs/issues/2026-10-17-doc-list-stale-on-an-augmented-tracker-routes-to-tracker-conventions-and-gets-only-the-preamble.md
tags:
- cluster/selector-narrower-than-its-population
kind: bug
title: 'no test validates the frontmatter of docs/issues files; a glued `key: valuekey: value` line and two broken archive files went unnoticed'
---

# BUG: no test validates the frontmatter of docs/issues files; a glued `key: valuekey: value` line and two broken archive files went unnoticed

## Summary

Nothing in the repo parses the YAML frontmatter of `docs/issues/*.md` and fails when it is malformed. On
2026-10-17 an integration of eight fix branches carried two files whose frontmatter had two keys glued onto one
line (`closed: 2026-10-17owner: marius`) with a green gate, and a PyYAML scan afterwards also found two
pre-existing archived files whose blocks do not parse at all. A malformed block means a reader of the
frontmatter cannot read the file's `status`/`kind`, so `doc(action="find", kind="bug", ...)` can miss the record.

## Class fit

The cluster scan in `tests/issue_clusters.rs` is a selector narrower than the population its name implies: it
reads only open bug files and only their `cluster/` tag lines, so a `fixed` file, a non-tag key or an archived
file is never examined and a malformed line there returns a well-formed pass. It differs from the class's other
members in that the population is a directory of markdown records rather than tool output or transcripts.

## Symptom (Effect)

The glued line, in two merged files (the p50 emission-budget issue at line 10, and the doc.list_stale issue):

```
closed: 2026-10-17owner: marius
```

PyYAML (`yaml.safe_load` on the block between the `---` fences) on the pre-fix p50 blob:

```
mapping values are not allowed here
```

Two archived files that already failed before the integration (`yaml.safe_load` on every tracked file under
`docs/issues/`, 1161 files, 2 bad):

```
docs/issues/archive/2026-08-26-index-status-model-fields-dropped-but-still-documented.md
  while scanning for the next token
  found character '`' that cannot start any token
    in "<unicode string>", line 16, column 13:
      unverified: `embedding_count`, `db_path` and ...

docs/issues/archive/2026-08-28-root-is-https-or-loopback-has-no-test-coverage.md
  mapping values are not allowed here
    in "<unicode string>", line 15, column 340:
       ... te is now PERMANENT, not pending: a change must be made in both  ...
```

## Reproduction

At `integration/open-issues-2026-10-17`, base `4b173071` (`git rev-parse HEAD` before the repair commits was
`0b353283`):

```
git show 31f0041c^:docs/issues/2026-10-17-p50-emission-budget-fixture-never-archives-a-bug-file-so-the-largest-doc-move-delivery-is-unbudgeted.md > glued.md
python3 -I -c "import yaml,re; yaml.safe_load(re.match(r'---\n(.*?)\n---\n', open('glued.md').read(), re.S).group(1))"
```

Raises `mapping values are not allowed here`. The gate (`./scripts/gate.sh`) was green on that tree.

## Environment

Linux, PyYAML available to `python3`, branch `integration/open-issues-2026-10-17`.

## Root cause

measured 2026-10-17: grep over `tests/` and `src/` for readers of `docs/issues` frontmatter finds one test that
reads it, `tests/issue_clusters.rs` (`tracked_open_bug_files`, `frontmatter()`, `cluster_tags()`), and it
selects `status: open` bug files and reads only `cluster/` tag lines with a line scanner, not YAML. The glued
lines sat in `status: fixed` files, so they were never read. The production parser
(`src/tools/markdown/frontmatter.rs`; the librarian's `serde_yml` is an optional dependency) runs on reindex,
not in a test over the tracked files. What wrote `closed: <date>` without its trailing newline was not isolated:
not measured.

## Evidence

The two archived files were found by the scan in the Symptom section. Neither is covered by `issue_clusters`
(archived files are not open).

## Hypotheses tried

1. **Hypothesis:** a test already validates frontmatter. **Test:** grep `tests/` and `src/` for `docs/issues`
   readers. **Verdict:** rejected: only the cluster-tag line scan above exists. **Evidence link:** Root cause.

## Fix

A dependency-free integration test (no YAML crate) that walks tracked non-archive `docs/issues/*.md` and checks
each `---` block with a strict line grammar: every line is `key: value`, `- item`, blank, or an indented
continuation; no two keys glued on one line; the block carries `kind`, `status`, `title` and `opened`; and
`status: fixed` carries `closed`. Added on the integration branch together with the two archive repairs (quoting
the `unverified` value, YAML syntax only). Status stays `open` until that branch lands and its SHA and patch-id
can be recorded here.

## Tests added

`tests/issue_frontmatter.rs`, with a fixture of the pre-fix glued blob and one mutation per rule.

## Workarounds

Scan by hand with PyYAML over `docs/issues/**/*.md`.

## Resume

Once the integration lands: record the fix SHA and `git show <sha> | git patch-id --stable` here, set
`status: fixed` and `closed:`, and decide separately whether archived files should be validated too (the new
test walks non-archive files only, so a broken file moved into the archive later is not caught).

## References

- `tests/issue_clusters.rs`
- `src/tools/markdown/frontmatter.rs`
- `docs/issues/_TEMPLATE.md`
