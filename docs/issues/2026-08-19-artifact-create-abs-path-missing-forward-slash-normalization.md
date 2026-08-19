---
status: open
opened: 2026-08-19
closed:
severity: medium
owner: marius
related: []
tags: [windows, librarian, artifact, guide-routing]
kind: bug
---

# BUG: `artifact(action="create")` emits `abs_path` via raw `PathBuf::display()`, breaking Windows tracker-guide routing

## Summary
`src/librarian/tools/create.rs:368` builds the create-response's `abs_path`
field with a raw `PathBuf::display().to_string()` instead of the project's
`to_forward_slash` helper, which every sibling librarian tool (`mv.rs`,
`context.rs`, `doctor.rs`, `get.rs`, `reindex.rs`, `util.rs`) uses. On
Windows this leaves backslashes in the path, which breaks
`names_tracker_path`'s forward-slash-only `"docs/trackers/"` substring
match — so creating a tracker artifact never routes to the
`tracker-conventions` guide hint on Windows.

## Symptom (Effect)
```
thread 'server::guide_hint_tests::an_artifact_call_naming_a_tracker_path_delivers_the_tracker_guide' panicked at src\server.rs:4600:
expected hint to contain "tracker-conventions", got "...topic 'librarian'..."
```

## Reproduction
1. `git checkout experiments` at `5b54848fd2a4e7fe5da6bf277dc85de39958ff27`
2. `cargo +1.97.1-x86_64-pc-windows-gnu test --release --features server-stack --lib server::guide_hint_tests::an_artifact_call_naming_a_tracker_path_delivers_the_tracker_guide -- --nocapture`
3. Observe the panic above.

## Environment
Windows 11 Enterprise 10.0.26200 (VDI), `1.97.1-x86_64-pc-windows-gnu`
toolchain (host toolchain forced to gnu — this VDI has no MSVC C++ Build
Tools; see `docs/issues/archive/2026-08-08-cyberark-epm-blocks-ort-sys-build-script.md`).
`codescout` repo, `experiments` branch.

## Root cause
`src/librarian/tools/create.rs:368`:
```rust
"abs_path": row.abs_path.display().to_string(),
```
— a raw `PathBuf::display()`, never passed through
`crate::util::fs::to_forward_slash`. Confirmed via grep that `create.rs` is
the one librarian tool missing from the `to_forward_slash` call-site list;
every sibling tool that emits path fields calls it.

Downstream, `names_tracker_path` (`src/librarian/adapter.rs:276-292`)
checks `abs_path`/`rel_path` for the literal substring `"docs/trackers/"`
(forward-slash). Its own unit test passes fine (hand-written forward-slash
strings), so the discriminator logic itself is correct — the break is
purely in `create.rs`'s un-normalized path emission feeding it a
backslash-separated string that can never contain that substring on
Windows. `LibrarianAdapter::relevant_guide_topic`
(`src/librarian/adapter.rs:182-205`) then falls through to the default
`"librarian"` guide instead of `"tracker-conventions"`.

This is Windows-only: on Unix, `PathBuf::display()` is naturally
forward-slash, so the missing normalization is invisible there.

The guide-routing feature (`names_tracker_path`) was wired in commit
`73ccb495` ("feat(prompts): wire the three guides you chose, and route by
what the call touched"), part of the experiments fast-forward — this is a
genuine regression in freshly-merged code, not a stale test expectation;
the test matches the feature's documented intent (see
`docs/issues/archive/2026-08-16-cap-evicted-guidance-lands-in-guides-nothing-triggers.md`,
cited in both the test and `names_tracker_path`'s own doc comment).

*Inferred from source comparison (create.rs vs sibling tools) and the
passing-synthetic-vs-failing-integration test split — not independently
re-verified at runtime with a standalone probe binary: this sandbox's
CyberArk EPM policy blocks execution of any freshly-compiled binary,
including a minimal isolated repro (see
`docs/issues/archive/2026-08-08-cyberark-epm-blocks-ort-sys-build-script.md`,
which the 2026-08-19 investigation suggests may be a general fresh-binary
execution block rather than ort-sys-specific — worth a follow-up if this
resurfaces elsewhere).*

## Evidence
### Subagent investigation (2026-08-19)
```
grep for to_forward_slash call sites across src/librarian/tools/*.rs:
mv.rs, context.rs, doctor.rs, get.rs, reindex.rs, util.rs — all present.
create.rs — absent.
```

## Hypotheses tried
1. **Hypothesis:** `names_tracker_path`'s own matching logic is broken.
   **Test:** Read `names_tracker_path` (adapter.rs:276-292) and its
   existing unit test (adapter.rs:766-795, forward-slash literals).
   **Verdict:** rejected — the discriminator's own test passes; the logic
   itself is correct given a forward-slash input.
2. **Hypothesis:** `create.rs`'s `abs_path` field is un-normalized on
   Windows, so the input `names_tracker_path` receives never contains the
   substring it looks for.
   **Test:** Compared `create.rs:368` against every sibling librarian
   tool's path-emission call sites.
   **Verdict:** confirmed — `create.rs` is the sole outlier missing
   `to_forward_slash`.

## Fix
Not yet implemented. Change `src/librarian/tools/create.rs:368` from
`row.abs_path.display().to_string()` to go through
`crate::util::fs::to_forward_slash`, matching every sibling tool.

## Tests added
N/A — not yet fixed. The existing test
(`src/server.rs`, `guide_hint_tests::an_artifact_call_naming_a_tracker_path_delivers_the_tracker_guide`)
already covers this once corrected.

## Workarounds
None; on Windows, creating a tracker artifact does not surface the
`tracker-conventions` guide hint. The artifact itself is created correctly
— only the guide-routing hint is affected.

## Resume
Apply `to_forward_slash` at `src/librarian/tools/create.rs:368`, matching
the pattern in `mv.rs`/`context.rs`/`doctor.rs`/`get.rs`/`reindex.rs`/`util.rs`.
Re-run the cited test to confirm. Also worth checking whether other
recently-added `create.rs` response fields have the same gap.

## References
- `src/librarian/tools/create.rs:368` (the missing normalization)
- `src/librarian/adapter.rs:182-205` (`relevant_guide_topic`)
- `src/librarian/adapter.rs:276-292` (`names_tracker_path`)
- `src/server.rs:4600` (the failing test)
- commit `73ccb495` ("feat(prompts): wire the three guides you chose, and route by what the call touched")
- `docs/issues/archive/2026-08-16-cap-evicted-guidance-lands-in-guides-nothing-triggers.md`
