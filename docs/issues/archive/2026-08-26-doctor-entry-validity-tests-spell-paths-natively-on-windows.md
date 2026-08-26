---
id: d3f0e01c3b2f8f51
kind: bug
status: fixed
title: 'BUG: 31 doctor entry-validity tests fail on every Windows lane because two fixtures spell paths natively while the scans key on the catalog''s forward-slash form'
tags:
- windows
- ci
- doctor
- path-form
- test-fixture
- cross-platform
closed: 2026-08-26
opened: 2026-08-26
owner: marius
severity: high
---

## Summary

CI run `32740102144` (`047dd433`, 2026-08-24) failed **46** tests on `windows-latest /
default`, and the same core set on `no-features` and `local-embed`. **31 of the 46 are one
cluster** — `librarian::tools::doctor::tests` — and every one of them asserts `1` and gets
`0`, or `Some(n)` and gets `None`.

The cause is not in the scans. Two **test helpers** spell paths with `to_string_lossy()`,
which yields the platform-native form, while the scans key their exposure map on the
catalog's `abs_path`, which is forward-slash by this module's own check #1. On Linux the
two spellings are byte-identical, so the fixtures were green from the day they were
written. On Windows every map lookup missed, exposure read as zero, and nothing was
reported.

**30 of the 31 were fixture defects, not production ones** — Windows users were never
handed empty worklists; only CI was red. The 31st, `outside_roots_group`, was a real
production bug.

## Symptom (Effect)

Three Windows lanes red since 2026-08-20, on a repo whose Linux gate is green. Because the
count is large and the module is one nobody reads on Linux, the natural reading of the CI
summary — *"scoping is broken on Windows"* — is wrong in a way that would have shipped a
fix to the wrong layer.

## Reproduction

Local, not from the CI log. `mingw-w64`, `wine` and the `x86_64-pc-windows-gnu` target are
all installed on this box:

```
scripts/build-windows.sh test --lib doctor::tests
→ 132 passed; 31 failed
```

Same 31 as MSVC, so the defect is not ABI-specific. That matters: it made the cluster
debuggable in ~7 s per iteration instead of one CI round-trip each.

## Root cause

`deg_key` — the exposure-map key helper used by the entry-validity scan tests:

```rust
fn deg_key(path: &std::path::Path, token: impl Into<String>) -> (String, String) {
    (path.to_string_lossy().into_owned(), token.into())   // NATIVE spelling
}
```

The scans look that key up using the catalog's `abs_path` (`C:/users/.../a.md`). The
fixture supplies `C:\users\...\a.md`. Miss → exposure `0` → below `EXPOSURE_THRESHOLD` →
no violation → `left: 0, right: 1`, thirty times.

`call_accumulates_scoped_out_counts_per_root_across_checks_and_keeps_roots_distinct` had
the same defect in its own two lookup keys — the 31st failure, and the only one that
printed a populated map, which is how the shape became visible at all.

**`outside_roots_group` is the one genuine production bug.** It took a forward-slash
string, walked it with `Path::components()`, and rebuilt the prefix with `PathBuf::push`
— which joins with the **native** separator. On Windows it returned
`\home\u\work\proj`: the exact spelling `check_backslash` exists to forbid, emitted by
the module that enforces it.

## Evidence — three hypotheses, all refuted before any code was written

Recorded because each is a plausible thing for the next reader to re-derive, and each is
wrong:

1. **"The bare-id focus-switch never opens the sub-project's `MemoryStore`."** Refuted:
   `Agent::activate_within_workspace` calls `MemoryStore::open(&abs_root)` on the
   dormant→activated promotion.
2. **"`with_project_at` resolves the workspace default rather than the focused project."**
   Refuted: it ends in `ws.focused_active().and_then(|p| p.as_active())`.
3. **"`containing_root` cannot bridge the Windows spelling mismatch."** Refuted, and this
   is the important one: it compares via `comparable_path` on both sides and carries a
   `#[cfg(windows)]` regression test proving `//?/C:/…` resolves under `\\?\C:\…`.

The comparison layer was never at fault. Only reading the *populated* map in the 31st
failure — `{"C:/users/marius/AppData/Local/Temp/.tmpYvdoGN/sibling-a": 2}` against a
lookup returning `None` — pointed at the fixture instead.

A fourth guess died too: the MSVC log shows `C:/Users/RUNNER~1/…`, an 8.3 short path, which
looked like a long-vs-short mismatch. Under wine the same 31 fail with ordinary long paths,
so the short name was a red herring the local repro removed.

## Hypotheses tried

Beyond the four above: none. The local reproduction arrived early enough that guessing
stopped being the cheapest move.

## Fix

Three edits, all in `src/librarian/tools/doctor.rs`:

- `deg_key` → `crate::util::fs::RepoPath::from_path(path).into_string()`. **30 of 31.**
- the accumulate test's two root lookup keys → same treatment. **The 31st.**
- `outside_roots_group` no longer round-trips through `PathBuf`. It now splits the
  forward-slash string directly, which is both correct on every platform and simpler than
  what it replaced.

## Tests added

None — the repair is to fixtures and to one pure function whose existing tests already
pinned the behaviour; they were simply passing for the wrong reason on Linux and failing
for the right one on Windows.

Verified on both platforms rather than one:

| lane | before | after |
|---|---|---|
| wine, `x86_64-pc-windows-gnu`, `--lib doctor::tests` | 132 passed / **31 failed** | **163 passed / 0 failed** |
| linux, `cargo test --workspace` | — | **4525 passed / 0 failed / 51 ignored** |

`cargo fmt` clean; `cargo clippy --all-targets -- -D warnings` clean. The Linux run had
`CODESCOUT_EMBEDDER_URL` unset, per
`docs/issues/archive/2026-08-26-ci-test-lanes-red-because-one-test-reads-ambient-embedder-config.md`.

## Workarounds

None needed — no user-facing behaviour changed except `outside_roots_group`'s reporting
key, which was wrong only on Windows.

## Resume

**This does not turn the Windows lanes green.** 14 of the 46 remain unexamined:

- `config::global::tests` ×3 (XDG vs Windows config dir)
- `util::path_security::tests` ×2
- `tools::config::tests` ×2 (linked-worktree topology)
- `retrieval::index_lock::tests` ×2 (holder pid)
- one each in `usage::content_tests`, `tools::rendezvous`, `tools::grep`, `tools::core`,
  `server::guide_hint_tests`

The 46th was `agent::tests::memory_embedder_is_built_from_the_shared_code_embedder`, fixed
separately in `d81064f7`. Expect the remaining 14 to split into two or three causes rather
than one; the wine loop above is the way to work them.

## References

- `src/librarian/tools/doctor.rs` — `deg_key`, `outside_roots_group`, and the module header
  documenting this same class from WIN-30: *"the catalog stores `//?/C:/...` while
  `current_project` holds `\\?\C:\...`"*. The entry-validity work landed 2026-08-20, three
  months after that lesson was written into this very file.
- `src/librarian/tools/mod.rs` — `containing_root` and `comparable_path`, exonerated above
- `scripts/build-windows.sh` — the local Windows loop
- `docs/issues/archive/2026-08-26-ci-test-lanes-red-because-one-test-reads-ambient-embedder-config.md`

## Fix provenance

- **SHA:** `af5d0dab` (`experiments`)
- **patch-id:** `de915596b0fbdaac4c9e06fc471ca0ef662227d1`

`master` is a strict ancestor of `experiments`, so promotion is a fast-forward and this is
already the master-side SHA; there is no second one to record.

