---
id: fb8ad49da8d9c32d
kind: bug
status: fixed
title: 'BUG: the code index recognises ten file extensions, so C and C++ files are never indexed and index(verify) still reports complete'
tags:
- cluster/selector-narrower-than-its-population
closed: 2026-11-07
opened: 2026-11-07
severity: high
unverified: 'The optional step ''index(verify) lists languages declared by the project but not admitted by lang_for_ext'' was NOT done, so verify can still say complete for a language the walk ignores. The file''s ''Related observation'' (nested .codescout store when activating a sub-project by path) was not investigated and should be filed separately if confirmed. .h maps to "c" although a header cannot say C or C++ (harmless today: the plain-text chunker treats both alike). After upgrading, an existing C/C++ index reports the new files missing in verify until the next sync. The gate (./scripts/gate.sh --check) does not cover the three heavy Python suites, server-stack, librarian code in the lean lane, or the local ONNX weight tests.'
---

# BUG: the code index recognises ten file extensions, so C and C++ files are never indexed and index(verify) still reports complete

## Summary

`semantic_search` is useless on a C/C++ repository: the index holds no `.c`, `.cpp`, `.h` or `.hpp` chunk, and `index(action="verify")` reports `complete` because it compares the store against the indexer's own file walk, which already excludes those files. Seen on the Kodi tree (thousands of `.cpp`/`.h` files under `xbmc/`).

## Class fit

`cluster/selector-narrower-than-its-population`: the indexer's extension selector (`lang_for_ext`) sees fewer files than a project's declared `languages` list (`c`, `cpp`, ...) claims, and `verify` reuses the same selector as its denominator, so the gap reads as zero missing files instead of as absent coverage.

## Symptom (Effect)

Workspace: Kodi (https://github.com/xbmc/xbmc), five projects in `.codescout/workspace.toml` (`kodi` `.`, `xbmc` `xbmc`, `addons` `cmake/addons`, `libUPnP` `lib/libUPnP`, `packaging` `tools/android/packaging`).

- Root project `kodi`: `index(action="status")` = 156 files / 3675 chunks. `index(action="verify")` says `expected_files` 158. The indexed files are Python, JS and shell (for example `addons/webinterface.default/js/kodi-webinterface.js`, `tools/Linux/packaging/package-kodi-launchpad.sh`).
- Project `xbmc` (built with `workspace(action="activate", path="xbmc")` then `index(action="build", force=true)`): `file_count` 8, `chunk_count` 51. `index(action="verify")` verdict `complete`, `expected_files` 8. All 8 files are Python/shell tools under `xbmc/addons/kodi-dev-kit/tools`.
- `semantic_search(project_id="xbmc", query="application startup and service initialisation")` returns only those tools. A root query `UPnP device discovery SSDP` returns unrelated shell and JS chunks.

## Reproduction

Any checkout whose source is C/C++ (observed on Kodi; server build git_sha `a10cb7703`, embeddings model CodeRankEmbed via remote-http):

1. Activate the workspace.
2. `index(action="build", force=true)`
3. `index(action="verify")`: reports `complete`, `expected_files` is the count of non-C/C++ files only.
4. `semantic_search(query="application startup and service initialisation")`: no C/C++ chunk is returned.

## Environment

Linux, codescout MCP over stdio, server build git_sha `a10cb7703`, embeddings model CodeRankEmbed via remote-http. Project: Kodi checkout, master. `/usr/bin/clangd` is installed.

## Root cause

Verified by reading the source, 2026-11-07 (not measured at runtime by changing the table):

- `src/embed/mod.rs:34-48`, `lang_for_ext`, is documented as the single source of truth for which files get embedded. Its arms are `rs`, `py`, `ts`/`tsx`, `js`/`jsx`/`mjs`, `go`, `java`, `kt`, `md`/`mdx`, `sh`/`bash`, `toml`; every other extension returns `None` (`_ => return None`, line 46). There is no `c`, `h`, `cc`, `cpp`, `cxx`, `hpp` or `cs` arm.
- `src/retrieval/sync.rs:526`, in `indexable_files` (492-540), does `let Some(lang) = crate::embed::lang_for_ext(ext) else { continue; }`, so a `.cpp` file is dropped from the walk. `src/embed/preflight.rs:148` applies the same predicate.
- `src/retrieval/sync.rs:594-646`, `verify_index_coverage`, builds `expected` from `indexable_files`, so excluded files are neither "missing" nor "orphan" and the verdict is `complete`. Its doc comment states this is deliberate ("two are extensions outside `languages`").
- The AST layer does know C/C++ (`src/ast/mod.rs:76`, `"cpp" | "cc" | "cxx" => Some("cpp")`), and `symbols` works on `xbmc/platform/xbmc.cpp` (`XBMC_Run L22-81`), so the gap is specific to the embedding walk's extension table.

The `languages` list in `.codescout/project.toml` is not consulted by this walk, which is why editing it changed nothing (see Hypotheses tried). Whether omitting C/C++ from `lang_for_ext` was intended (for example to bound index size) is unknown; nothing read states it.

Correction 2026-11-07, after the fix: the bullet above that says 'The AST layer does know C/C++' (`src/ast/mod.rs:76`) is true of `detect_language` (it maps the names), but C and C++ have no tree-sitter grammar (`without_ast` list in `src/ast/mod.rs`), so the chunker has no C/C++ splitter and uses plain text. The mechanism above describes the code before the fix.

## Evidence

Numbers in Symptom were read from live `index(status)`, `index(verify)` and `semantic_search` output on 2026-11-07 after `/mcp` restart.

Verification of the fix, 2026-11-07: `./scripts/gate.sh --check` on `fix/open-issues-batch-20261107-r2` at its tip `af3e2a9ec`: FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0, PYTHON_LIGHT=0 (lean nextest 6227 tests; default lane green). The gate does NOT cover: the three heavy Python suites, server-stack, librarian code in the lean lane, the local ONNX weight tests.

## Hypotheses tried

1. **Language config excludes C/C++.** Test: root `.codescout/project.toml` lists `bash,c,cpp,csharp,css,html,java,javascript,markdown,python`, no `ignored_paths`, `force_include = []`, empty `[lsp]`. A `project.toml` for `xbmc` listing `bash,c,cpp,html,markdown,python` gave the same 8 files after a forced rebuild; without that file `workspace(activate xbmc)` reports languages `[]` and the build also gave 8 files. Verdict: rejected.
2. **No C++ language server.** Test: `/usr/bin/clangd` exists and `symbols(path="xbmc/platform/xbmc.cpp")` returns `XBMC_Run L22-81`. Verdict: rejected.
3. **Stale server.** Test: earlier builds were refused with "binary deleted from disk"; after `/mcp` restart the build ran and gave the numbers above. Verdict: rejected.
4. **Extension table lacks C/C++.** Verdict: confirmed by reading (see Root cause); not tested by editing the table.

## Fix

Fixed on branch `fix/open-issues-batch-20261107-r2`. All of the commits below are now on `experiments` at `143c1941f`, the same SHAs because the branch was fast-forwarded in.

Commits:

- `4bc3dfefebc94488a2427b071d22c1de8d151541` (branch fix/open-issues-batch-20261107-r2) / patch-id `4c7c62bd7568a3745b8508f947d5099eaee58f9d` - the fix.
- `562a2abc1e9dbc2ea6c5de9dc38691154709b26a` (branch fix/open-issues-batch-20261107-r2) / patch-id `054d4340915d7c29c9cc591faa968e4f35115cee` - doc comment: only tag names match `ast::detect_language`, which does not know header extensions.

Where the change lives:

- `src/embed/mod.rs`, `lang_for_ext`, gains `"c" | "h" => "c"` and `"cc" | "cpp" | "cxx" | "hpp" | "hh" | "hxx" => "cpp"`.
- The walk (`src/retrieval/sync.rs`, `indexable_files`) and `verify_index_coverage` take their extensions from `lang_for_ext`, so they follow without a change of their own.
- `src/embed/preflight.rs` uses the same predicate; its behaviour is covered by a test (see Tests added).
- The chunker falls back to plain text for C and C++, because they have no tree-sitter grammar (`without_ast` list in `src/ast/mod.rs`). No grammar was added.

Plan deviation: the candidate fix also said to make `index(verify)` name extensions or languages the project declares but the walk never admits. That step was NOT done (see the `unverified` caveat).

Limits:

- `.h` maps to `c` although a header cannot say C or C++. Harmless today: the plain-text chunker treats both alike.
- After upgrading, an existing C/C++ index reports the new files as missing in `verify` until the next sync.

## Tests added

Each was red before the fix, seen failing for the stated reasons:

- `embed::tests::lang_for_ext_indexes_c_and_cpp_sources_and_headers`
- `retrieval::sync::tests::indexable_files_walks_c_and_cpp_sources_and_headers`
- `embed::preflight::tests::cpp_preflight_counts_sources_and_headers_toward_size_limit`

Each fix was reviewed adversarially by an Opus reviewer who ran the new test red against the reverted production change (or by mutants) and approved.

## Workarounds

Use `grep` and `symbols(path=...)` for C/C++ code; `semantic_search` only covers the Python, JS, shell, markdown and toml files.

## Related observation (separate defect, not part of the above)

Activating a sub-project by path (`workspace(action="activate", path="xbmc")`) or pinning a call with `workspace=<sub-project path>` creates a nested `<subproject>/.codescout/` store (`usage.db`, `private-memories`, `index-state.json`) and writes `.codescout/private-memories/` into `<subproject>/.gitignore` (a new file, or an edit of a tracked one). Memories written through such a pinned call land in the nested store, not in the central `.codescout/projects/<id>/memories`. Seen on Kodi on 2026-11-07. Not investigated in source. Related earlier records: `docs/issues/2026-10-18-a-workspace-pin-naming-a-path-that-does-not-exist-creates-a-project-there.md` and `docs/issues/2026-10-28-a-nested-git-repo-cannot-join-a-workspace-as-a-project-and-pinned-onboarding-mixes-in-the-home-workspace.md`. File as its own bug if confirmed distinct.

## Fix provenance

- **SHA:** `4bc3dfefebc94488a2427b071d22c1de8d151541` (`experiments`)
- **patch-id:** `4c7c62bd7568a3745b8508f947d5099eaee58f9d`

## Resume

Landed: the fix is on `experiments` (fast-forward to `143c1941f`), `./scripts/gate.sh --check` was green there, and this file was archived. Optional follow-up: have `index(verify)` list languages declared by the project but not admitted by `lang_for_ext`. If the nested `.codescout` store under 'Related observation' is confirmed, file it as its own bug.

## References

- `src/embed/mod.rs:34-48`, `src/retrieval/sync.rs:492-540`, `src/retrieval/sync.rs:594-646`, `src/embed/preflight.rs:148`, `src/ast/mod.rs:76`
- `docs/issues/archive/2026-06-02-preflight-sync-walker-divergence.md` (origin of the single-source-of-truth rule)
