---
status: open
opened: 2026-05-24
closed:
severity: medium
owner: marius
related: []
tags: [docs, manual, ci, drift]
kind: bug
---

# BUG: docs/manual/src/tools/ has drifted ~20 tools from src/tools/**/*.rs

## Summary

The `tool-docs-sync` CI job (in `.github/workflows/ci.yml`) lints that
every tool registered under `src/tools/**` has matching documentation in
`docs/manual/src/tools/`. The manual hasn't been refreshed in months;
~20 tools are added/renamed/removed/consolidated. The job has been
downgraded to informational (prints `::warning::`, exits 0) until the
docs refresh ships.

## Symptom (Effect)

`tool-docs-sync` job in CI emits the following `diff -u` between
`/tmp/code-tools.txt` and `/tmp/doc-tools.txt`:

```
--- /tmp/code-tools.txt
+++ /tmp/doc-tools.txt
+at                       # docs only — superseded by edit_code(insert position=after)?
-call_graph               # code only — needs doc
-delete_memory            # code only — needs doc
-edit_code                # code only — needs doc
+edit_section             # docs only — renamed → edit_markdown?
-edit_markdown            # code only — needs doc
-get_guide                # code only — needs doc
-get_usage_stats          # code only — needs doc
-index                    # code only — needs doc
-index_project            # code only — needs doc
-index_status             # code only — needs doc
+insert                   # docs only — likely consolidated into edit_code
+insert_code              # docs only — likely consolidated into edit_code
-library                  # code only — needs doc
-list_docs                # code only — needs doc
-list_functions           # code only — needs doc
-list_libraries           # code only — needs doc
-list_memories            # code only — needs doc
+memory_staleness         # docs only — superseded?
-read_markdown            # code only — needs doc
-read_memory              # code only — needs doc
-register_library         # code only — needs doc
+remove                   # docs only — likely → edit_code(action=remove)
+remove_symbol            # docs only — likely → edit_code(action=remove)
+rename                   # docs only — likely → edit_code(action=rename)
+rename_symbol            # docs only — likely → edit_code(action=rename)
+replace                  # docs only — likely → edit_code(action=replace)
+replace_symbol           # docs only — likely → edit_code(action=replace)
+scope                    # docs only — superseded?
-write_memory             # code only — needs doc
```

(`-` lines exist in code only — need docs. `+` lines exist in docs only
— stale doc files for renamed/consolidated/removed tools.)

## Reproduction

```bash
git rev-parse HEAD
# (any commit on experiments after 2026-05-24)

# Run the lint locally:
grep -rA1 'fn name(&self)' src/tools/ --include='*.rs' --exclude-dir=tests \
  | grep -E '^\s*"[a-z_]+"' \
  | sed 's/.*"\(.*\)".*/\1/' \
  | sort -u > /tmp/code-tools.txt
grep -roh '## `[a-z_]*`' docs/manual/src/tools/ \
  | sed 's/## `\(.*\)`/\1/' | sort -u > /tmp/doc-tools.txt
diff -u /tmp/code-tools.txt /tmp/doc-tools.txt
```

## Environment

- OS: Linux 7.0.9-zen1-1-zen (and CI ubuntu-latest)
- Branch: experiments
- Affects: GitHub Actions `tool-docs-sync` job — failing before downgrade,
  warning after.

## Root cause

Code-side moved without doc updates. Likely chronology:
- `*_symbol` tools (`remove_symbol`, `rename_symbol`, `replace_symbol`,
  `insert_code`) consolidated into a single `edit_code` tool with an
  `action` argument. Doc files renamed/removed without updating the manual.
- `edit_section` renamed to `edit_markdown`.
- New tools added (`call_graph`, `get_guide`, `get_usage_stats`,
  `index*`, `library`, `list_*`, `delete_memory`, `read_memory`,
  `write_memory`, `register_library`, `read_markdown`) without
  corresponding doc additions.
- `at`, `insert`, `remove`, `rename`, `replace`, `scope`,
  `memory_staleness` are old tool names still referenced in the manual
  but no longer registered.

## Evidence

CI run https://github.com/mareurs/codescout/actions/runs/26355210379 —
`tool-docs-sync` job failed with the diff above before the gate was
downgraded.

## Hypotheses tried

N/A — the diff is mechanical; root cause is documented above.

## Fix

Two-phase:

1. **Reconcile renames/consolidations** — for each `+` line in the diff
   above, decide: did the tool get renamed (rename the doc file) or
   consolidated into another tool (delete the doc file, ensure the
   destination tool's doc covers the consolidated behavior)?
2. **Write missing docs** — for each `-` line, write a section in the
   appropriate `docs/manual/src/tools/*.md` file following the existing
   tool-doc shape.

Once both phases ship and the diff is clean, restore the strict gate in
`.github/workflows/ci.yml::tool-docs-sync` (revert the `informational`
comment block and the `if !diff ...` wrapper back to the previous
`diff ... || exit 1` shape).

## Tests added

The `tool-docs-sync` job itself is the regression test — flipping back
to strict mode (`exit 1` on diff) verifies the docs match the code.

## Workarounds

`tool-docs-sync` job is informational; it doesn't fail CI. Drift is
visible in CI logs as a `::warning::` annotation. Other CI jobs
unaffected.

## Resume

1. Inventory: list every file under `docs/manual/src/tools/` and the
   tools each file currently covers.
2. Map renames/consolidations from the diff to actual git history (use
   `git log --follow src/tools/edit_code*.rs` and similar to confirm
   what consolidated into what).
3. Write/rewrite/delete doc sections accordingly.
4. Verify clean diff locally, then restore strict gate.

## References

- `.github/workflows/ci.yml` — the `tool-docs-sync` job.
- `docs/manual/src/tools/` — the manual subtree.
- F-11 in `docs/trackers/bug-fix-session-log.md` — sibling pre-existing
  CI rot (mold linker).
- H-5 in `docs/trackers/codescout-usage-hookify.md` — sister
  `audit-doc-refs` informational gate, same pattern.
