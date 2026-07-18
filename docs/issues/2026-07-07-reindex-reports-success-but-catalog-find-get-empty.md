---
id: null
kind: bug
status: mitigated
title: null
owners: []
tags:
- librarian
- catalog
- reindex
- find
- windows
- data-loss-risk
- prune_missing
- git-ignore-interaction
topic: null
time_scope: null
closed: null
---

# BUG: `reindex` repeatedly reports success but `artifact(find)`/`artifact(get)` return empty/null for the whole project afterward

## Summary
In an active MCP session against the `Mercury BOM` project, `librarian(action="reindex")`
consistently reported `updated: 11` (matching the project's 11 `docs/trackers/*.md` files) across
three separate calls (plain, `force=true`, and `force=true` + explicit `repo`/`scope=repo`), but
`artifact(action="find", kind="tracker")` returned `count: 0` every time afterward, and
`artifact(action="get", id=<a tracker id returned by a working `find` earlier in the same
session>)` returned `null`. The tracker markdown files on disk were unaffected throughout — direct
`read_markdown`/`edit_markdown` calls against the same paths kept working normally.

## Update 2026-07-07 (later) — `doctor(fix="prune_missing")` escalation, confirmed destructive
Acting on this issue's own Resume step 1, `doctor(fix="prune_missing", root="//?/C:/Users/<user>/work/proj X")`
was run against the confirmed-dead pre-rename root (E7). It reported
`{"pruned": {"artifact_rows": 37, "commit_rows": 99}}` — but a `find(kind="tracker",
scope="project")` immediately afterward **still returned `count: 0`** for the active `Mercury BOM`
project, exactly as before the prune. A subsequent `reindex(force=true, scope="project")`
reported `{"updated": 11, ...}` (the original bug's exact signature) and **did not repopulate**
`find` either — still `count: 0`. See E8/E9.

This means the 37 pruned `artifact_rows` were very likely **not** genuinely-dead `proj X` debris —
they were this project's own live, current tracker rows, merely mis-rooted under the stale `proj X`
path string (consistent with hypothesis 4: the project's `git_root`/registered-root mapping still
points at the pre-rename path even though the working tree itself is `Mercury BOM`). `doctor`'s
`prune_missing` fix has no way to distinguish "genuinely orphaned, folder is gone" from "this
repo's own rows, incorrectly rooted after a rename" — both look identical (root doesn't exist on
disk) from `doctor`'s point of view, but only the first case is safe to delete. **The tool offers
no non-destructive alternative** (e.g. a `reroot`/`migrate` fix that rewrites `abs_path`/`git_root`
in place instead of deleting the row) for the second case, which is exactly the case this bug
report is about.

**Net effect on this session:** zero data loss at the source-of-truth level — every tracker
markdown file on disk is confirmed byte-for-byte intact (verified via direct filesystem listing,
all 11 files present with today's edit timestamps) — but the catalog's index of this project is now
*more* empty than before (37 fewer rows, including whatever link/entry-collection state lived only
in those DB rows rather than in the markdown body), and there is currently no known way to
repopulate it short of a full `create`-per-artifact re-onboarding, which would mint new ids and
likely break any existing `link`/`graph` edges between trackers.

## Symptom (Effect)
Early in the session, `artifact(action="find", kind="tracker")` returned 19-21 real tracker
items for the project (ids like `7dc4b76a0c852674` for `docs/trackers/bom-requirements.md`), and
`artifact(action="create", ...)` successfully created a new tracker
(`docs/trackers/data-quirks.md`, returned id `ff7a06bc76ccc271`), which was then successfully read
back and updated once via `artifact(action="update", id="ff7a06bc76ccc271", ...)`.

Later in the same session (after several more `edit_markdown` calls against *other* trackers, and
after one `librarian(action="reindex")` call that returned normally), the same operations began
failing:

```
artifact(action="update", id="ff7a06bc76ccc271", patch={...})
→ "unknown id `ff7a06bc76ccc271`"

artifact(action="find", filter={"rel_path": {"contains": "data-quirks"}})
→ {"count": 0, "items": []}

artifact(action="find", kind="tracker")               # no filter at all
→ {"count": 0, "items": []}

artifact(action="get", id="7dc4b76a0c852674")          # bom-requirements.md, known-good earlier
→ null
```

Three subsequent reindex attempts, each followed immediately by `find(kind="tracker")`, all
reproduced the same `count: 0`:

```jsonc
// Attempt 1 — plain reindex
librarian(action="reindex")
→ "collection_exists"                                   // (this call's own summary string)

// Attempt 2 — force reindex
librarian(action="reindex", force=true)
→ {"added":0,"updated":11,"removed":0,"unchanged":0,"orphans_removed":0,
   "unknown_count":7,"unknown_sample":["366f4ce5e9714aa8","a56101735484969b",
   "f550fd76e7af07e1","3928ace0da5ed8b7","adc74c100ab087fb","338a6b185095cdb8",
   "0bb05b5360ec51df"],"backfill_error_count":0,"backfill_errors":[],
   "unknown_sample_note":"complete","scope":"project",
   "targets":["\\\\?\\C:\\Users\\MAILINCA.BRN.002\\work\\Mercury BOM"]}

// Attempt 3 — force + explicit repo + scope=repo
librarian(action="reindex", force=true,
          repo="c:\\Users\\MAILINCA.BRN.002\\work\\Mercury BOM", scope="repo")
→ {"added":0,"updated":11,"removed":0,"unchanged":0,"orphans_removed":0,
   "unknown_count":7,"unknown_sample":[...same 7 ids as attempt 2...],
   "backfill_error_count":0,"backfill_errors":[],"unknown_sample_note":"complete",
   "scope":"repo","targets":["\\\\?\\C:\\Users\\MAILINCA.BRN.002\\work\\Mercury BOM"]}
```

`updated: 11` and the same 7 `unknown_sample` ids are byte-identical across attempts 2 and 3
despite `force=true` and a narrower explicit scope on attempt 3 — the reindex is either not
actually re-deriving find-queryable rows, or is writing them somewhere `find`'s query path
doesn't look.

`librarian(action="doctor")` (default scope, read-only) run immediately after was **not**
scoped to the active project — its 1,067 violations (1,013 `ads_colon_in_abs_path` + 54
`missing_file`) were entirely under an unrelated registered project
(`//?/C:/Users/MAILINCA.BRN.002/work/claude/codescout/...`), with zero violations reported for
`Mercury BOM`. So either `doctor` is not project-scoped by default (surprising, given
`reindex`/`find` clearly were), or the Mercury BOM rows are gone from the catalog in a way that
doesn't register as a `doctor`-detectable violation at all (e.g. absent rather than
present-but-broken).

Meanwhile, `edit_markdown(action="edit", path="docs/trackers/bom-requirements.md", heading=...)`
and `read_markdown(path="docs/trackers/open-questions.md", heading=...)` continued to succeed
throughout — including *between* the failing `find`/`get` calls above — so the bug is isolated to
the catalog's query/lookup layer, not file I/O, not the `docs/trackers/` write-guard (which also
kept firing correctly: attempting a direct `read_markdown`/`edit_markdown` on
`docs/trackers/data-quirks.md` — the one `find` couldn't locate — still returned the expected
`"is a librarian-managed artifact"` refusal, meaning *something* in the toolchain still knows that
path is librarian-owned even though the catalog `find`/`get` can't produce a row for it).

## Reproduction
Not yet reproducible as a minimal standalone repro — this was observed incidentally during a long
multi-hour session, not isolated. Best lead:

1. Long-running MCP session against a Windows project registered as
   `\\?\C:\Users\<user>\work\Mercury BOM` (note the `\\?\` extended-length-path prefix — every
   `abs_path`/`project_root` returned by `workspace(status)`/`workspace(activate)` in this session
   used this prefix form, alternating with a forward-slash `//?/C:/...` form in different tool
   responses within the *same* session — see Hypotheses tried #1).
2. Interleave `artifact(create)`, `artifact(update)`, `edit_markdown`, and at least one
   `librarian(action="reindex")` call across ~15-20 tool calls touching ~6 different tracker
   files in `docs/trackers/`.
3. At some point after the first mid-session `reindex`, `artifact(find, kind="tracker")` starts
   returning `count: 0` and stays that way for the rest of the session, surviving further
   `reindex` calls (plain, `force=true`, and `force=true` with explicit `repo`/`scope=repo`).

## Environment
- codescout MCP server, commit `0a1d87368b8f91c24dcde5ad161c7e685805b870`, branch `experiments`.
- Host: Windows (PowerShell 7 terminal observed in the session); target project path
  `C:\Users\MAILINCA.BRN.002\work\Mercury BOM`, a *different* registered project than the
  codescout repo itself (multi-root workspace).
- Project language mix: markdown, javascript, css, python (per `workspace(activate)` response).
- Session was long-running (many tens of tool calls over what the transcript implies was a
  multi-hour, multi-day-spanning conversation) before the failure was first noticed.

## Root cause
**Likely confirmed (2026-07-07, later same day, after a codescout service restart):**
this project's directory was renamed at some point from `work/proj X` to `work/Mercury BOM`
(same repo — confirmed via two stray log files at `work/proj_x_pytest_{pass,fail}.log` whose
captured pytest banners read `rootdir: C:\Users\<user>\work\proj X`, and via this session's own
test output emitting `C:\Users\<user>\work\proj X\.venv\Lib\site-packages\dash\...` in a
deprecation-warning path — the venv's installed-package metadata still carries the pre-rename
path). After the restart, `artifact(find)` (now scoped to the **home directory**, `scope="repo"`,
rather than the active project — see Evidence E5) surfaced a catalog row for
`work/proj X/docs/trackers/bom-requirements.md` (id `091f13098d244dbf`) whose `created_at`/
`updated_at` **predate** every edit made earlier in this same session, and whose body cannot be
read (`os error 3`, path doesn't exist — `work/proj X` is gone from disk). This is the same
orphaned-catalog-row failure mode as
[[2026-06-13-catalog-orphans-survive-repo-rename]] (no rename migration; catalog identity is
path-derived), but with a **new wrinkle**: rather than the old-path rows simply lingering
alongside working new-path rows, the *new*-path (`work/Mercury BOM`) rows this session created
and repeatedly confirmed working (see E1/E3) appear to have been superseded/reverted by the
stale pre-rename snapshot after the restart — `artifact(find/get)` scoped tightly to the active
`Mercury BOM` project returns nothing at all post-restart (re-tested, still `count: 0`), while
the home-directory-scoped query surfaces only the dead `proj X` copy. This suggests the restart
reloaded the catalog from a backup/snapshot **older than this session's mid-session reindex**,
rather than the live on-disk DB state — i.e. a persistence/restart-recovery issue layered on top
of the known rename-orphan gap, not purely the rename gap by itself.

**Reassurance, not part of the codescout defect:** the underlying tracker *markdown files* on
disk at `work/Mercury BOM/docs/trackers/*.md` were completely unaffected throughout — re-verified
after the restart via direct `grep`/`read_markdown` that every edit from this session (new
requirement rows, disposition/crosswalk notes, etc.) is still present. Only the librarian's
catalog/search layer regressed; no user content was lost.

## Evidence
### E1 — `find` before the break (session start, first `librarian` call of the session)
Returned 19+ real items including:
```json
{"id":"7dc4b76a0c852674","kind":"tracker","status":"active",
 "title":"BOM requirements — JC workbook (BOM-0001..0027)",
 "abs_path":"//?/C:/Users/MAILINCA.BRN.002/work/Mercury BOM/docs/trackers/bom-requirements.md",
 "updated_at":1783417801606}
```
Note the `abs_path` form here: `//?/C:/...` (forward slashes, doubled leading slash).

### E2 — `workspace(activate)` response later in the same session
```json
{"status":"ok","project":"Mercury BOM",
 "project_root":"\\\\?\\C:\\Users\\MAILINCA.BRN.002\\work\\Mercury BOM", ...}
```
Note the form here: `\\?\C:\...` (backslashes). Both forms appeared for what should be the same
path across different tool responses in the same session (`find`'s `abs_path` field vs.
`workspace`'s `project_root` field) — raised as Hypothesis 1 below, not confirmed as causal.

### E3 — post-break `get` on a pre-break id
```
artifact(action="get", id="7dc4b76a0c852674") → null
```
Same id as E1, same session, no intervening `delete`/`move` call against that artifact from this
session's tool-call history.

### E4 — `doctor` scope
```
librarian(action="doctor") → {"summary":{"total":1067,
  "by_check":{"ads_colon_in_abs_path":1013,"missing_file":54}}}
```
Sampled violations (first 40 of 6413 lines in the buffered result) were **all** under
`//?/C:/Users/MAILINCA.BRN.002/work/claude/codescout/...` — a different registered project than
the one being worked in (`Mercury BOM`) when `doctor` was invoked. Not manually confirmed whether
*any* of the 1,067 violations belong to Mercury BOM (the full 6413-line buffer was not exhaustively
scanned), but the first 40 — and the fact that `find`/`get` return nothing at all for Mercury BOM
rather than "present but flagged" — suggest Mercury BOM's rows may be absent from the catalog
entirely rather than present-but-drifted, which `doctor`'s checks (path-form + missing-on-disk)
would not catch.

### E5 — first `find` call after the user reported "codescout is up" (mid-session restart)
```json
artifact(action="find", kind="tracker", limit=30)
→ {"count": 30, "items": [
    {"id":"2c64d4356c1699fd", ..., "abs_path":"work/proj X/docs/trackers/reconnaissance-patterns.md", "updated_at":1782226465243},
    {"id":"091f13098d244dbf", ..., "abs_path":"work/proj X/docs/trackers/bom-requirements.md", "updated_at":1782226465169},
    ...
  ], "scope": {"applied":"repo", "abs_path":"\\\\?\\C:\\Users\\MAILINCA.BRN.002", ...}}
```
Every id in this result is **different** from the ids seen in E1/E3 for what should be the same
files (e.g. `bom-requirements.md` was `7dc4b76a0c852674` in E1, now `091f13098d244dbf`) — the
catalog was not just re-scoped, its rows were re-minted with new identities. `scope.abs_path`
resolved to the bare user home directory, not the active `Mercury BOM` project, without any
`workspace(activate)` call having been made yet in this post-restart turn — also surfaced two
duplicate/`kind:"unknown"` rows for `docs/open-questions.md` under two *other* sibling projects
(`work/Mercury MRP Automation/` and `work/MRP/`), suggesting broader post-restart scope/dedup
issues beyond just this project.

### E6 — re-scoping to the active project reproduces the original symptom
```
workspace(activate, path="C:\Users\MAILINCA.BRN.002\work\Mercury BOM") → {"status":"ok", ...}
artifact(find, filter={"or":[{"rel_path":{"contains":"bom-requirements"}}, ...]}, scope="project")
→ {"count": 0, "items": [], "scope": {"applied":"project",
    "abs_path":"\\\\?\\C:\\Users\\MAILINCA.BRN.002\\work\\Mercury BOM", ...}}
```
Confirms the `count: 0` symptom from the Symptom section persists post-restart when correctly
scoped to the active project — the restart did not fix the original bug, it changed its
presentation (home-dir-scoped queries now surface *stale* rows instead of nothing).

### E7 — the stale row points at a path that doesn't exist, and predates this session
```json
artifact(action="get", id="091f13098d244dbf", heading="## Gap summary")
→ {"id":"091f13098d244dbf",
   "abs_path":"//?/C:/Users/MAILINCA.BRN.002/work/proj X/docs/trackers/bom-requirements.md",
   "kind":"tracker","status":"active","created_at":1782226465169,"updated_at":1782226465169,
   "provenance":{"refreshed_at_commit":null,"commits_behind_head":null,
                 "head_commit":"21930f5b81aaba502ac6fe4b71fcf6a001b04903"},
   "body_error":"The system cannot find the path specified. (os error 3)"}
```
`work/proj X` does not exist on disk (`list_dir` on `work/` shows only `Mercury BOM/`,
`Mercury MRP Automation/`, `MRP.old/`, plus two stray files `work/proj_x_pytest_{pass,fail}.log`
whose captured pytest banners confirm the pre-rename path:
```
platform win32 -- ... C:\Users\MAILINCA.BRN.002\work\proj X\.venv\Scripts\python.exe
rootdir: C:\Users\MAILINCA.BRN.002\work\proj X
```
This session's *own* `pytest` runs (against the current `work/Mercury BOM` checkout) independently
corroborate the rename: a Dash deprecation warning emitted during this session's test output read
`C:\Users\MAILINCA.BRN.002\work\proj X\.venv\Lib\site-packages\dash\...` — the installed venv's
package metadata still carries the pre-rename absolute path, meaning the venv itself was created
before the `proj X` → `Mercury BOM` rename and was never recreated afterward.

### E8 — `doctor(fix="prune_missing")` against the dead root removed rows, but `find` stayed empty
```
doctor(action="doctor", fix="prune_missing", root="C:\Users\MAILINCA.BRN.002\work\proj X")
→ {"pruned": {"artifact_rows": 0, "commit_rows": 0}}   # backslash form: no match

doctor(action="doctor", fix="prune_missing", root="//?/C:/Users/MAILINCA.BRN.002/work/proj X")
→ {"pruned": {"artifact_rows": 37, "commit_rows": 99}}  # forward-slash //?/ form: matched

artifact(find, kind="tracker", scope="project")  # immediately after
→ {"count": 0, "items": [], "scope": {"applied": "project",
    "abs_path": "\\\\?\\C:\\Users\\MAILINCA.BRN.002\\work\\Mercury BOM", ...}}
```
Note the path-form mismatch reproduces hypothesis 1 from the original filing (`//?/C:/` vs
`\\?\C:\`) — `root=` had to be given in the forward-slash `//?/` form to match any rows at all,
the natural Windows backslash form silently matched zero. This is a second, independent
confirmation that the catalog's `abs_path`/root comparisons are inconsistent about path-string
normalization across at least two different code paths (this `doctor` call and the original
`find`/`get` symptom).

### E9 — forced reindex after the prune reproduces the original bug signature exactly
```
librarian(action="reindex", force=true, scope="project")
→ {"added": 0, "updated": 11, "removed": 0, "unchanged": 0, "orphans_removed": 0,
   "unknown_count": 7, "unknown_sample": ["366f4ce5e9714aa8", "a56101735484969b", ...],
   "scope": "project", "targets": ["\\\\?\\C:\\Users\\MAILINCA.BRN.002\\work\\Mercury BOM"]}

artifact(find, kind="tracker", scope="project")  # immediately after
→ {"count": 0, "items": []}
```
`updated: 11` matches the project's 11 `docs/trackers/*.md` files exactly (same as the very first
occurrence of this bug), and the 7 `unknown_sample` ids persist across the prune — i.e. whatever
those 7 ids are (hypothesis 2, still unresolved), they are not part of the 37 rows that were just
pruned. `find` returning `count: 0` immediately after a `reindex` that self-reports `updated: 11`
is the *exact* original symptom this issue was filed for — reproduced again, post-prune, on
demand, in the same session.

### E10 — cross-session finding: no single-instance guard on the main MCP server, multiple processes routinely share one catalog.db (2026-07-07, unrelated codescout-repo session)

While debugging an unrelated Windows path-normalization bug in a separate session against the
codescout repo itself (not Mercury BOM), a routine `cargo rb` rebuild + MCP reconnect cycle was
found to leave **3 concurrent `codescout.exe` processes** running simultaneously
(`Get-Process codescout` → 3 PIDs, all pointing at the same `target/release/codescout.exe`),
before any manual intervention. Tracing the catalog open path
([src/librarian/catalog/mod.rs:157-176](../../src/librarian/catalog/mod.rs#L157-L176)) confirms
the design is explicitly WAL + `busy_timeout=5000` to tolerate "cross-process writers (separate
codescout server instances sharing one catalog file)" — i.e. multiple concurrent server processes
against one shared, per-user `catalog.db` (`$XDG_DATA_HOME/librarian/catalog.db`, or the Windows
equivalent under `%LOCALAPPDATA%`) is an **acknowledged, designed-for** scenario, not a rare edge
case. No single-instance lock/guard exists for the main MCP server process itself (contrast with
`socket_discovery.rs`'s per-workspace lock file, which only covers the LSP mux and peer-delegation
servers, not the primary server). No `wal_checkpoint` call or `Drop` impl on `Catalog` was found —
shutdown relies entirely on SQLite's own WAL crash-recovery (which is correct-by-design for a
single writer, but was not verified against N>1 concurrent writers each independently mid-`reindex`
at kill time).

This does not by itself reproduce or confirm the restart-reverts-to-stale-snapshot mechanism this
issue is chasing (hypothesis 4's second clause) — no direct evidence ties Mercury BOM's specific
session to more than one concurrent process — but it establishes that the *precondition* (multiple
live processes against one shared catalog, at least one killed non-gracefully) is easy to hit by
ordinary use (a plain rebuild-and-reconnect loop), not a rare operator error. Confidence: medium —
plausible aggravating factor / reproduction path, not a confirmed root cause for this specific bug.

### E11 — re-tested after user-reported "codescout was fixed and reindex done" — Mercury BOM still empty, other projects now work
```
workspace(activate, path=".../Mercury BOM", read_only=false) → {"status":"ok", "project":"Mercury BOM", ...}
artifact(find, kind="tracker", scope="project") → {"count": 0, "items": [],
  "scope": {"applied":"project", "abs_path":"\\\\?\\C:\\Users\\<user>\\work\\Mercury BOM", ...}}

librarian(reindex, force=true, scope="project") → {"added":0, "updated":11, "removed":0,
  "unknown_count":7, "scope":"project", "targets":["\\\\?\\C:\\Users\\<user>\\work\\Mercury BOM"]}
artifact(find, kind="tracker", scope="project") → {"count": 0, "items": []}   # unchanged
```
The general claim "codescout was fixed" is **partially true**: a `find` issued **before**
re-activating Mercury BOM (still scoped to home from a prior turn) returned `count: 50` of real,
correctly-pathed rows — but every single one was from the **codescout project itself**
(`work/claude/codescout/docs/trackers/*.md`), not Mercury BOM. So the catalog/embedding pipeline
genuinely works again for at least one project. Once re-scoped correctly to Mercury BOM
(confirmed via `scope.abs_path` in the response), the exact original symptom reproduces on demand:
`reindex` reports `updated: 11` (matching the 11 tracker files on disk) and `find` still returns
`count: 0` immediately after. This is the **fourth** on-demand reproduction of the core symptom
in this project specifically (E1/E3, E6, E9, now E11), across two different server
restarts — strong evidence the break is specific to something about *this project's* catalog
state (most likely the still-unpruned or still-mis-rooted remnant from the `proj X` rename, per
Hypothesis 4), not a general server-health issue that a restart alone fixes.

Also observed in the same re-test: `doctor(scope="project")` while Mercury BOM was the active
project returned 17 `missing_file` violations, but **all 17 were rooted under `work/MRP`** — a
**different, unrelated dead project alias** (`work/MRP` → apparently renamed to `Mercury MRP
Automation` at some point, mirroring the `proj X` → `Mercury BOM` rename pattern exactly). Zero
of the 17 violations were Mercury-BOM-rooted, consistent with Mercury BOM's rows being **absent**
from the catalog rather than present-and-flagged (same observation as E4, now confirmed on a
fresh restart with an unrelated project as the corroborating example of the same failure mode).
### E12 — CONFIRMED root cause: `.git/info/exclude` + orphan-cleanup silently deletes still-existing files on every reindex (2026-07-07, codescout-repo session)

Reproduced live, then root-caused with certainty (not "likely" — confirmed by reading the actual code path and the actual `.git/info/exclude` file):

```
artifact(find, kind="tracker", scope="project")  → {"count": 0, "items": []}
artifact(find, scope="project")                  → {"count": 11, "items": [... none under docs/trackers/ ...]}
artifact(find, filter={"rel_path":{"contains":"trackers"}}, scope="project") → {"count": 0, "items": []}
```
`find(kind="tracker")` returning `count: 0` was itself a **red herring / misdiagnosis carried
forward from earlier in this bug's own investigation (including E1/E11's reproduction steps)**:
Mercury BOM's 11 catalogued artifacts are `kind: reference` (4) and `kind: unknown` (7) — **none
are `kind: tracker`** at all, so that specific query was never going to return anything regardless
of catalog health. The REAL signal is that **zero rows exist anywhere for `docs/trackers/*`**,
confirmed by the unfiltered `find` and the `rel_path contains "trackers"` filter both returning
none — even though `docs/trackers/` has 11 real `.md` files on disk right now, including
`bom-requirements.md` (the exact file E1 showed as a working catalog row with id
`7dc4b76a0c852674` early in the original session).

**Root cause, read directly from source and the repo's own git config:**
1. `C:\Users\<user>\work\Mercury BOM\.git\info\exclude` contains:
   ```
   # --- local-only: never stage onto the publish branch ---
   /docs/trackers/
   /docs/conversations/
   /docs/superpowers/
   ...
   ```
   (Mercury BOM's own workflow: these directories are tracked on a `dont_push`/local branch and
   deliberately excluded from whatever "publish" branch this repo publishes to.)
2. `index_repo_sync`'s walker ([src/librarian/indexer.rs](../../src/librarian/indexer.rs#L92),
   `WalkBuilder::new(abs_root).standard_filters(true)`) respects `.git/info/exclude` as part of
   `standard_filters` — so `docs/trackers/*.md` is **silently never visited** by any `reindex`
   call, full stop.
3. The orphan-cleanup at the end of `index_repo_sync` deleted every catalog row under `abs_root`
   whose id was **"not seen in this walk"** — treating "the walker didn't visit this path" as
   equivalent to "the file no longer exists on disk". Those are NOT the same thing: a file can be
   unvisited because it's ignore-excluded, permission-denied, or any number of walker-level
   reasons that have nothing to do with whether it still exists. **Every single `reindex` call
   against Mercury BOM was therefore silently deleting the `docs/trackers/*.md` catalog rows**,
   because they were never in `seen_ids`, regardless of force/force_embed settings.
4. A previous session already suspected something like this and added
   `[ignored_paths] force_include = ["docs/trackers", "docs/trackers/**", ...]` to Mercury BOM's
   own `.codescout/project.toml` as an attempted fix. **That key does not exist anywhere in
   codescout's source** (`grep -r force_include` across the entire codescout repo: zero matches)
   — it has always been a silent no-op. The workaround was never actually wired up; the person
   who added it had the right instinct but the feature was never implemented.

**This fully explains the entire bug**, superseding the earlier "likely" rename/restart-revert
theory (Hypothesis 4) as the PROXIMATE mechanism for the *current* symptom — the rename-orphan gap
and the restart-state-reversion observation (E5-E7, E10) may still be real, separate phenomena,
but they are not required to explain why `find`/`get` come back empty for Mercury BOM specifically:
this single, deterministic, 100%-reproducible-on-every-reindex mechanism is sufficient on its own.

**Fix landed** (codescout repo, local commit `f48e50ed` on `experiments`, not pushed — see
`docs/issues/2026-07-07-orphan-cleanup-deletes-walk-excluded-existing-files.md` for the isolated
write-up): `index_repo_sync`'s orphan-cleanup now checks `Path::exists()` on each "not seen"
candidate before deleting it, instead of assuming absence. A file that still exists on disk is
never deleted from the catalog again, regardless of why the walker skipped it this pass.

**What this fix does NOT do:** it does not make `docs/trackers/*.md` visible to `reindex` again —
the walker still respects `.git/info/exclude`, so those files still won't be (re-)discovered or
(re-)embedded. It only stops the *destructive* side effect (silent deletion of already-catalogued
rows). Restoring `docs/trackers/` to the catalog needs one of: (a) removing the
`.git/info/exclude` entries in Mercury BOM itself (a change to *their* repo, not codescout's), or
(b) actually implementing the `force_include` config as a real feature in codescout (the
generalizable fix, matching what was already — incorrectly — assumed to work).

### E13 — Mercury BOM re-verified working end-to-end after the E12 fix (2026-07-07, Mercury BOM session)

```
workspace(activate, path=".../Mercury BOM", read_only=false) → {"status":"ok", "project":"Mercury BOM", ...}
artifact(find, kind="tracker", scope="project") → {"count": 20, "items": [
    {"id":"7dc4b76a0c852674", "kind":"tracker", ..., "abs_path":"docs/trackers/bom-requirements.md", ...},
    ... 19 more, all real docs/trackers/*.md and docs/conversations/*.md rows ...
  ]}
artifact(get, id="7dc4b76a0c852674", heading="## Gap summary") → live body including
  "Newly-discovered scenarios (2026-07-07 — `User Stories - BOM (2).xlsx`)" and BOM-0028..0036,
  i.e. today's actual edits, not a stale snapshot.
```

Two things worth flagging against E12's own predictions:

1. **`bom-requirements.md`'s id is `7dc4b76a0c852674` — the original pre-break id from E1**, not
   a re-minted one (contrast E5, where ids were re-minted after the earlier restart). Identity
   was preserved this time, which is a good sign for whatever recovery path was used.
2. **This contradicts E12's "Remaining gap, NOT fixed by this change" note**, which predicted
   `docs/trackers/*.md` would keep being invisible to the walker going forward (since
   `.git/info/exclude` is still respected) even after the destructive-delete fix landed. Live
   evidence says otherwise: all 20 rows are present with content that is demonstrably *fresher*
   than the original E1 baseline. Either `force_include` got a real implementation, the
   `.git/info/exclude` entries were adjusted, or some other backfill path re-populated these rows
   — E12's write-up doesn't account for this, so the "remaining gap" claim should be treated as
   superseded/unverified rather than still-true. Worth reconciling in the codescout repo's own
   follow-up issue (`2026-07-07-orphan-cleanup-deletes-walk-excluded-existing-files.md`) rather
   than assumed fixed by extrapolation.

**For Mercury BOM's own purposes: fully working as of this check.** `find`/`get` are reliable
again. The file-path-fallback workaround (`read_markdown`/`edit_markdown` bypassing the catalog)
that this bug report's own Workarounds section recommended is no longer needed and has been
retired from Mercury BOM's own `CLAUDE.md` — normal `artifact(find)`/`artifact(get)` usage has
resumed.
## Hypotheses tried
1. **Hypothesis:** The two `abs_path` forms observed (`//?/C:/...` vs `\\?\C:\...`, see E1/E2) are
   normalized inconsistently somewhere between write-time (reindex) and read-time (find/get),
   causing a lookup miss even though rows exist.
   **Test:** None performed from this session — no direct DB access attempted.
   **Verdict:** deferred.
   **Evidence link:** E1, E2.

2. **Hypothesis:** `reindex`'s `unknown_sample` ids (7, byte-identical across two separate
   `force=true` calls) are catalog rows that reference something reindex can't resolve on this
   pass (e.g. `docs/trackers/communication/*.md` or `docs/conversations/*.md` files, which this
   project's own `.codescout/project.toml` force-includes past `.gitignore` — see the project's
   own `CLAUDE.md` "CodeScout librarian — known quirks" section), and whatever is failing to
   resolve them is also short-circuiting the rest of the project's rows from becoming
   `find`-visible.
   **Test:** Not performed — would require inspecting what those 7 ids used to point to (their
   `rel_path`s) via a catalog query this session didn't have tooling for.
   **Verdict:** deferred.
   **Evidence link:** reindex outputs in Symptom section.

3. **Hypothesis:** `doctor`'s default scope is not the active project (contrast with `reindex`
   and `find`, which respect `workspace(activate)`'s pinned project) — this would just mean
   `doctor` didn't happen to surface a Mercury-BOM-specific issue, not that there isn't one.
   **Test:** Not performed — would need `librarian(action="doctor", scope="project")` explicitly
   and a manual line-by-line scan of the 6413-line buffered violation list for any Mercury BOM
   path.
   **Verdict:** deferred.
   **Evidence link:** E4.

4. **Hypothesis:** This project's folder was renamed `work/proj X` → `work/Mercury BOM` prior to
   this session; the catalog's rename-orphan gap ([[2026-06-13-catalog-orphans-survive-repo-rename]])
   left dead `proj X`-rooted rows in place, and a codescout **service restart mid-workspace-session**
   ("codescout is up" — user-observed restart) reloaded/reverted the catalog to a snapshot that
   predates this session's `Mercury BOM`-rooted rows (including everything created/updated this
   session), rather than the live DB state.
   **Test:** Re-ran `artifact(find, kind="tracker")` scoped to the active `Mercury BOM` project
   after the restart → `count: 0` (same symptom as before the restart). Ran the same query with
   `scope="repo"` (which resolved to the **home directory**, not `Mercury BOM` — itself
   unexpected, see hypothesis 5) → surfaced a `proj X`-rooted row for `bom-requirements.md`
   whose `body_error` confirms the path doesn't exist on disk, and whose timestamps predate this
   session.
   **Verdict:** confirmed (the rename + dead-row part; the restart-reverts-to-old-snapshot
   mechanism is inferred, not directly observed at the DB level).
   **Evidence link:** E5, E6, E7.

5. **Hypothesis:** After the restart, `workspace(activate, path="…/Mercury BOM")` no longer
   correctly pins subsequent `artifact(find, scope="repo")` calls to that project — the first
   post-restart `find` call (before any fresh `workspace(activate)` in the new session) resolved
   `scope.abs_path` to the **user home directory** instead, surfacing results from at least three
   unrelated sibling projects (`Mercury MRP Automation`, `MRP`, and the dead `proj X`) in one
   `find` response. Re-issuing `workspace(activate)` for `Mercury BOM` and re-querying with
   `scope="project"` fixed the over-wide scope (but then reproduced the `count: 0` symptom from
   hypothesis 4 instead).
   **Test:** Compared `find`'s `scope.abs_path` field immediately post-restart
   (`\\?\C:\Users\<user>`) vs. after an explicit fresh `workspace(activate)` call
   (`\\?\C:\Users\<user>\work\Mercury BOM`).
   **Verdict:** confirmed as observed; mechanism (why activation state didn't survive/apply across
   the restart) not investigated further.
   **Evidence link:** E5.

6. **Hypothesis:** The "restart" that triggered hypothesis 4/5's symptoms was an ordinary MCP
   reconnect (new `codescout.exe` process spawned by the client), and — per E10, found in an
   unrelated same-day session against the codescout repo itself — such reconnects routinely leave
   multiple `codescout.exe` processes alive concurrently against the **same shared** per-user
   `catalog.db` (WAL mode, `busy_timeout=5000`, explicitly designed for this per
   `src/librarian/catalog/mod.rs:157-176`, but with no single-instance guard on the main server
   process). If Mercury BOM's session also had >1 live process at the time of the restart, a
   stale process's in-flight write (or a stale process's own memory-mapped read view) could
   plausibly explain rows appearing to "revert." Not confirmed for this session — no process-count
   telemetry was captured at the time.
   **Test:** Not performed against this session (inferred from a separate session's E10 finding,
   not reproduced here). Would need: capture `Get-Process codescout` (or platform equivalent)
   count at the moment of the reported restart, and check whether the catalog's WAL/SHM files
   were held open by more than one PID.
   **Verdict:** deferred — plausible contributing mechanism, not confirmed.
   **Evidence link:** E10 (cross-referenced from a separate debugging session).

## Fix
Not investigated — no server-side/DB access available from the affected session to identify,
let alone implement, a fix. **`doctor(fix="prune_missing")` was tried in this session as the
sibling issue's fix and is now confirmed NOT to be the fix for this bug** (E8/E9): it deleted 37
artifact rows + 99 commit rows anchored under the dead `proj X` root, but `find`/`get` for the
active `Mercury BOM` project remained empty both immediately after the prune and after a
subsequent forced reindex. The real fix needs two things neither exists yet: (1) whatever
mechanism is causing `reindex` to root this project's rows under the stale pre-rename path in the
first place (this bug's actual root cause, still open), and (2) ideally a non-destructive
`doctor` fix mode that **re-roots** matching rows to the current path instead of deleting them,
for the case where a "dead" root turns out to be this project's own rows under a stale alias
rather than truly orphaned data — `prune_missing` cannot tell the two cases apart and currently
only offers the destructive option. **Re-tested 2026-07-07 after a reported general fix and
server restart (E11): the symptom reproduces identically for Mercury BOM specifically**, even
though the catalog now works correctly for at least one other project (codescout) — this is not
a general server-health issue a restart resolves; something in this project's own catalog state
(most likely the `proj X` rename remnant, Hypothesis 4) is uniquely broken.

**Update 2026-07-07 (CONFIRMED + FIXED, codescout side) — see E12.** The proximate mechanism
is now fully understood and fixed at the source: Mercury BOM's own `.git/info/exclude` lists
`/docs/trackers/`; codescout's walker (`standard_filters(true)`) respects that, and the
orphan-cleanup was treating "not walked this pass" as "file deleted" — silently wiping the
`docs/trackers/*.md` rows on every single reindex. Fixed in codescout (`src/librarian/indexer.rs`,
local commit `f48e50ed` on `experiments`, not pushed): the cleanup now checks `Path::exists()`
before deleting a "not seen" row. Full write-up:
[docs/issues/2026-07-07-orphan-cleanup-deletes-walk-excluded-existing-files.md](2026-07-07-orphan-cleanup-deletes-walk-excluded-existing-files.md).

**Remaining gap, NOT fixed by this change:** the walker still skips `.git/info/exclude`-matched
paths, so `docs/trackers/*.md` will not be (re-)discovered by future reindexes either — this fix
only stops further silent deletion of already-catalogued rows. Restoring `docs/trackers/`
visibility needs either (a) removing the exclude entries in Mercury BOM's own repo, or (b) an
actual implementation of the `force_include` config key a previous session already tried to use
(currently a complete no-op — zero references anywhere in codescout's source). Neither is done
yet; flagged to the user as a scope decision (their repo's git config vs. a new codescout
feature).
## Tests added
N/A — bug not yet root-caused at the mechanism level (restart/persistence behavior); no
regression test possible until that's understood. This is an unminimized field observation, not
a confirmed reproducible defect, though hypothesis 4's rename/orphan half is now confirmed via
direct evidence (E5-E7).

**Update 2026-07-07:** `index_does_not_delete_still_existing_file_newly_matched_by_ignore`
(`src/librarian/indexer.rs`) — indexes a file, then re-indexes with an `ignore` glob newly
matching that same still-existing file; asserts the row survives (`removed == 0`). `cargo test
--lib`: 2870 passed, 0 failed, 10 ignored.
## Workarounds
- Bypass `artifact(find)`/`artifact(get)` entirely and operate directly on the known file paths:
  `read_markdown(path=...)` / `edit_markdown(path=..., heading=...)` continued to work normally
  throughout the session even while the catalog queries were broken, since they don't go through
  the `find`/`get` lookup path for non-augmented trackers. (`edit_markdown` does still correctly
  *refuse* direct writes to catalog-managed files it recognizes as `kind: tracker`, e.g.
  `data-quirks.md`, even while `find` can't locate that same file — so whatever gates that
  refusal is a separate check from the one `find`/`get` use.)
- **⚠️ Do NOT reach for `doctor(fix="prune_missing")` as a fix for this symptom** — tried in this
  session (E8) and confirmed to (a) not fix `find`/`get` returning empty for the affected project,
  and (b) irreversibly delete 37 artifact rows + 99 commit rows that were very likely this
  project's own live data, just mis-rooted. There is currently **no known safe workaround** for
  the underlying `find`/`get`-returns-empty symptom beyond bypassing the catalog entirely (below);
  do not attempt catalog surgery without confirmed DB-level backup/rollback available first.
- Bypass `artifact(find)`/`artifact(get)` entirely and operate directly on the known file paths —
  this remains the only verified-safe workaround.

## Resume

1. **Do not re-run `doctor(fix="prune_missing")` against this project's dead-root alias again** —
   already tried (E8/E9), did not fix `find`/`get`, and destroyed 37 artifact rows + 99 commit
   rows that were probably this project's own mis-rooted data. Instead, get direct catalog DB
   access (same technique as [[2026-06-13-catalog-orphans-survive-repo-rename]]: locate
   `~/.local/share/librarian/catalog.db` or the Windows-equivalent path, `sqlite3` in with the
   `vec0` extension loaded) and check, **read-only first**: (a) do any rows for
   `abs_path LIKE '%Mercury BOM/docs/trackers/%'` exist right now (post-prune, they may be fully
   gone rather than just unreachable); (b) what `git_root`/root-mapping value is actually stored
   for this project's commits/workspace registration — is it still the pre-rename `proj X` path
   even for rows whose `abs_path` correctly says `Mercury BOM`? That root-mapping field, not the
   per-row `abs_path`, is the more likely place the stale rename data actually lives, and would
   explain why `reindex` keeps re-deriving the wrong root for freshly-scanned files.
1a. Before any further catalog mutation on a project hit by this bug, take a DB-level backup/copy
    of `catalog.db` first — this session had no way to do that and lost 37 rows as a result.
2. Root-cause the **restart-reverts-catalog-state** mechanism (hypothesis 4's second clause): does
   the codescout server load the catalog from a different file/path on startup than the one
   `reindex` writes to during a session? Is there a periodic snapshot/backup mechanism that
   restored an older state on restart? This is the part of the bug the sibling issue's
   `prune_missing` fix does **not** address — pruning dead rows doesn't explain why live,
   correctly-scoped rows created *during this session* stopped being queryable even before the
   dead `proj X` rows resurfaced.
3. Root-cause why post-restart `artifact(find, scope="repo")` resolved to the user home directory
   instead of the previously-active project (hypothesis 5) — check whether `workspace(activate)`
   state is meant to persist across a server restart, and if so, why it didn't here.
4. Resolve the 7 `unknown_sample` ids from `reindex`'s pre-restart output (hypothesis 2, still
   deferred/unconfirmed) to their `rel_path`s and check whether they're related to hypothesis 4
   at all, or a separate issue.
5. Confirm/deny Hypothesis 3 by re-running `doctor` with `scope="project"` while a project
   exhibiting the `find`-returns-empty symptom is active, and fully scanning (not sampling) the
   violation list for that project's own paths.
6. If reproducible, capture a minimal repro: single project, single tracker file, one `create`,
   one `reindex`, check `find` before/after — bisect how many interleaved operations it actually
   takes to trigger the break.
7. **New lead (hypothesis 6 / E10):** capture `codescout.exe` (or platform-equivalent) process
   count the next time this bug reproduces, at the moment of/immediately after any "restart."
   If >1 process is confirmed live against the same shared `catalog.db` at that moment, that
   would upgrade hypothesis 6 from deferred to confirmed and point at either (a) adding a
   single-instance guard for the main MCP server (mirroring `socket_discovery.rs`'s per-workspace
   lock, currently scoped only to the LSP mux/peer-delegation servers), or (b) an explicit
   `wal_checkpoint(TRUNCATE)` + clean connection close on graceful shutdown, neither of which
   exist today for `Catalog`.
## References
- Session: Mercury BOM project work, 2026-07-07 (BOM impact-analysis tool; unrelated domain, only
  relevant as the host project where this was noticed).
- Sibling: [[2026-06-13-catalog-orphans-survive-repo-rename]] — prior art on catalog rows
  becoming stale/orphaned and `doctor` not fully covering the failure mode; same DB-access
  technique would apply here.
- `CLAUDE.md` "CodeScout librarian — known quirks" (Mercury BOM project) — documents that
  `docs/trackers/communication/` and `docs/conversations/` are `.gitignore`-excluded-then-
  force-included via `.codescout/project.toml`, relevant to Hypothesis 2.
