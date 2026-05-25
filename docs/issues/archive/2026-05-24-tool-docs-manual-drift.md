---
status: fixed
opened: 2026-05-24
closed: 2026-05-25
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

**Shipped in master `6ebf65ee` (2026-05-25).** Prerequisite commits on master:
- `0aaf3f2c` — log F-13 + F-14 release-pipeline frictions (incidental).
- `bb5c160a` — fix the lint's own bugs (silent-empty code-side, regex anchoring,
  test-file exclusion).
- `6ebf65ee` — doc-side rewrite + strict gate restoration.

Three-part fix:

**Part 1 — Lint correctness** (`.github/workflows/ci.yml`):

- Added `-h` to the recursive `grep`. The downstream filter `grep -E '^\s*"..."'`
  required leading whitespace, but `grep -r` prefixes each output line with
  `filename:` (matched) or `filename-` (context). Result: the code-side
  extraction had been silently producing an empty list for an unknown period.
  Discovered during the fix audit.
- Anchored the doc-side regex to `^#{1,2} \`[a-z_]+\`$` (line-exact match of
  H1 or H2 backticked tool name). The previous unanchored `## \`[a-z_]*\``
  matched H3/H4 sub-headings (e.g. `### \`replace\`` in edit-code.md) and
  inline backticks, creating six false-positive `+` entries in this bug's
  recorded diff.
- Added `--exclude='tests.rs'` (test-fixture files like `EchoTool` and
  `AlwaysTool` in `src/tools/core/tests.rs` were polluting code-side).
- Added `--exclude='probe.rs'` (internal `__probe_description_cap__` is not
  user-facing).
- H1 acceptance: single-tool pages (`call-graph.md`, `edit-code.md`,
  `get-guide.md`, `read-markdown.md`) already use `# \`tool_name\`` as title;
  forcing a redundant H2 would be documentation-for-the-lint.

**Part 2 — Doc-side cleanup** (`docs/manual/src/tools/**`):

- Removed 4 legacy `*_symbol` stubs in `symbol-navigation.md` (consolidated
  into `edit_code` in v0.11).
- Renamed `## \`edit_section\`` → `## \`edit_markdown\`` in
  `document-section-editing.md` (v0.11 tool rename).
- Normalized `read-markdown.md` H1 from `# \`read_markdown\` improvements`
  to `# \`read_markdown\``; preserved the "page history" framing.
- Rewrote parenthetical H2 forms to bare H2s + new alias sections:
  - `workflow-and-config.md`: `## \`workspace\``, added `## \`project_status\``
    and `## \`get_usage_stats\``.
  - `semantic-search.md`: `## \`index\``, added `## \`index_project\`` and
    `## \`index_status\`` alias sections.
  - `library-navigation.md`: `## \`library\``, added `## \`list_libraries\``
    and `## \`register_library\``.
  - `memory.md`: added `## \`read_memory\``, `## \`write_memory\``,
    `## \`list_memories\``, `## \`delete_memory\`` alias sections.
  - `ast.md`: added `## \`list_functions\`` and `## \`list_docs\`` back-compat
    notes, corrected the stale "removed in v1" prose.

**Part 3 — Strict gate restored** (`.github/workflows/ci.yml`):

- Removed the `if ! diff ...; then echo "::warning::"; fi` wrapper. Bare
  `diff -u` now fails the job on any drift, matching the original intent.

Final lint output: 33 code-side, 33 doc-side, diff exit 0.

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
