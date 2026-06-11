---
kind: tracker
status: active
title: "Session Log — symbols single-match focus + docs"
owners: []
tags:
  - symbols
  - tool-ux
  - reconnaissance
---

# Session Log — symbols single-match focus + docs

> Work stream: make `symbols` search useful when it resolves to one symbol
> (show the code) and make `include_docs` work in search mode. Sibling work
> this session: `get_guide` per-session dedup (shipped — see
> `docs/issues/2026-06-11-get-guide-no-session-dedup.md`).

---

## Index

| ID | Date | Severity | Category | Status | Title |
|----|------|---------:|----------|--------|-------|
| F-1 | 2026-06-11 | med | architectural | mitigated | `workspace=` pin honored only by read_file, not the symbols family |

## Wins Index

| ID | Date | Impact | Pattern | Counterfactual | Status |
|----|------|-------:|---------|----------------|--------|
| W-1 | 2026-06-11 | med | scout the live render path, not just the field-builder | green tests on a dead renderer path | validated |

---

## W-1 — Scouted the live render path before claiming the symbols-focus feature worked

**Observed:** 2026-06-11, implementing `focus_single_symbol` + `attach_docstrings`
post-passes in `src/tools/symbol/symbols.rs` and the matching render edits in
`src/tools/symbol/display.rs::format_search_symbols`. User interrupted the build
and invoked `/reconnaissance` before I claimed success.

**Pattern:** Before declaring a tool-output change (new response field) done,
scout the tool's `output_form()` AND the `call()` exit structure — not just the
helper that builds the field. Confirm the field actually reaches the rendered
surface the user sees.

**Counterfactual:** The new fields (`docs`, `children`, `members_hint`) were
added by post-passes and rendered by `format_search_symbols`. Both edits rest on
two unverified assumptions: (a) search output renders via `format_search_symbols`
(i.e. `output_form() == Text`), and (b) the search path always reaches the tail
post-pass. The planned unit tests assert on the helpers' JSON mutation directly —
so if `output_form()` were `Json` (renderer dead) OR an early `return Ok(...)`
bypassed the tail (post-pass never runs for some branch), the tests would still
pass green while the feature was invisible/absent in real output. The scout
confirmed `output_form() == Text` (`symbols.rs:624`) and that `call()` has a
single early return — the overview dispatch at `symbols.rs:149` — so the search
path (LSP `workspace/symbol`, AST, glob) all funnel through the tail. Both edits
are live. Cost avoided: a green-tests-but-dead-feature ship, catchable only by
manual live inspection — or never.

**Confirming data points:**
1. This session — `output_form()=Text` + single search-path exit verified before
   the build; renderer edits proven on the live path.
2. `get_guide` bug earlier this session
   (`docs/issues/2026-06-11-get-guide-no-session-dedup.md`) — feature passed 46
   unit tests but was dead in the live CLI entry point until `references()`
   surfaced the missing call sites. Same failure family: tests on the unit, not
   the live wiring.

**Impact:** med — would have shipped a dead renderer path; unit tests on the
field-builder would not catch it.

**Promote-when:** A second instance where scouting `output_form()` / the
call-path exit prevents a tests-green/feature-dead ship. At 2 datapoints, promote
to CLAUDE.md: "Before claiming a tool-output change works, scout `output_form()`
and the call-path exit — unit tests on the field-builder don't prove the field
renders."

**Status:** validated

---

## F-1 — `workspace=` pin honored by read_file but silently ignored across the symbols family

**Observed:** 2026-06-11, fixing side-finding #2 (overview ignored a `workspace=` pin).

**When:** Scouting why a pinned `symbols(path=…, workspace=B)` resolved against the
active project A instead of B.

**Expected:** Per CLAUDE.md "Concurrent multi-workspace", every pinnable tool honors
a per-request `workspace=` pin; `symbols` advertises the param.

**Got:** Only `read_file` was wired for it (Phase 3, guard test
`read_file_honors_workspace_override_pin`). The symbols family resolves path args
via `resolve_read_path` / `resolve_glob`, which use the *active* project — so
`list_overview`, `symbol_at`, `call_graph`, `references`, and the `symbols`
search-glob path (`symbols.rs:253`) all silently fell back to the default
workspace. `list_overview` surfaced it as "path not found, relative to <default>".

**Probable cause:** Phase 3 pinning wired the override into `read_file`'s inlined
resolution but not into the shared `resolve_read_path`/`resolve_glob` helpers, so
every other read tool calling them inherited the gap ("distance from change").

**Workaround:** Fixed overview (commit `9fa4d482`) via override-aware
`resolve_read_path_for` / `resolve_glob_for`; siblings still pending.

**Severity:** med — a pinned read silently returns the wrong project's data (or
errors), the exact last-writer-wins hazard the per-request pin was meant to remove;
contained because the common single-project path is unaffected.

**Status:** mitigated — overview fixed + tested; `symbol_at` / `call_graph` /
`references` / search-glob still call the non-override wrappers.

**Fix idea / Pointer:** swap those four call sites to the `_for` twins (one-liner
each) + a per-tool pin guard test. See
`docs/issues/2026-06-11-symbols-search-include-docs-and-focus.md` Resume.

## Template for new entries

<!-- Insert new F-N / W-N entries above this line via:
     edit_markdown(action="insert_before",
                   heading="## Template for new entries",
                   content="## F-N — title\n...")
     Also update the matching Index / Wins Index table row at the top. -->
