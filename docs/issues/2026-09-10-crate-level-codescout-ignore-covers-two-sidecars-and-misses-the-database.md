---
id: '6e1f4ae6d5262ba8'
kind: bug
status: open
title: 'BUG: crate-level .codescout ignore rules cover two sidecars and miss the 250 MB database'
owners:
- marius
tags:
- cluster/guard-narrower-than-its-name
---

# BUG: the crate-level .codescout ignore rules cover two sidecars and miss the 250 MB database

## Summary
`.gitignore:25-27` reads:

```
# Same runtime files, but for crates with their own .codescout/ (e.g. librarian-mcp)
crates/*/.codescout/usage.db-wal
crates/*/.codescout/usage.db-shm
```

The comment claims parity with the root block's runtime-file rules. The pattern covers **two
files**, both of them SQLite sidecars. `usage.db` itself — 250 MB in this checkout —
`write.lock`, `librarian.db` and `embeddings.db` are **not ignored** at any `crates/*/` path.

## Symptom (Effect)
A crate with its own `.codescout/` offers its runtime database to git as untracked and
unignored. Nothing announces it: an unignored file is not an error, it is a candidate. The
next broad `git add` — `git add -A`, `git add crates/`, or an editor's "stage all" — commits
it. On this checkout that is a 250 MB binary entering history, where it cannot be removed
without rewriting it.

**It has already fired.** `crates/librarian-mcp/.codescout/usage.db` and
`crates/librarian-mcp/.codescout/write.lock` are tracked today, in a directory that is not
even a crate (no `Cargo.toml`, zero `.rs` files, three tracked files total). That is what the
gap looks like when it lands, and it landed on a stub where nothing reads the files, which is
why nobody has noticed.

## Reproduction
Measured 2026-09-10, `git check-ignore -v` against paths that do not exist, so the answer is
about the RULES rather than about what happens to be on disk:

```
crates/newcrate/.codescout/usage.db        *** NOT IGNORED ***
crates/newcrate/.codescout/usage.db-wal    IGNORED  <- .gitignore:26
crates/newcrate/.codescout/usage.db-shm    IGNORED  <- .gitignore:27
crates/newcrate/.codescout/write.lock      *** NOT IGNORED ***
crates/newcrate/.codescout/librarian.db    *** NOT IGNORED ***
crates/newcrate/.codescout/embeddings.db   *** NOT IGNORED ***
```

`du -h .codescout/usage.db` → **250M**.

## Environment
`git` on Linux; repo-wide `.gitignore`. Nothing tool- or session-specific.

## Root cause
**A gitignore pattern containing an internal `/` is anchored to the `.gitignore`'s own
directory, with or without a leading slash.** So every root-block rule reaches only the root:

- `/.codescout/usage.db` (line 19) — explicitly anchored.
- `.codescout/write.lock` (line 77) — **looks** unanchored and is not, because of the internal
  slash. This is the trap: the two lines behave identically while reading as if one is general
  and the other specific.

Someone hit exactly this and added the `crates/*/` block at lines 25-27 — so the mechanism was
understood. What did not follow is the SET: two sidecars were transcribed and the base
database, the lock and the other two databases were not. The comment then asserts the parity
the pattern does not have, which is why re-reading the file does not reveal it — the sentence
answers the question a reader would ask.

`.codescout/` is deliberately NOT blanket-ignored here (line 11-12: *"NOT ignored,
deliberately: /.codescout/audit/*.jsonl are the committed audit shards (T-7). Adding a blanket
`.codescout/` rule would silently stop sharing"*), and 101 files under `.codescout/` paths are
tracked on purpose. So the fix is not a blanket rule; it is completing the per-crate
enumeration.

## Evidence
### The stub's tracked files
```
crates/librarian-mcp/.codescout/usage.db
crates/librarian-mcp/.codescout/write.lock
crates/librarian-mcp/.gitignore
```
Three tracked files, no `Cargo.toml`, and `Cargo.toml`'s `members = [".",
"crates/codescout-embed"]` does not list it — so no cargo command reaches it and nothing has
ever complained.

### The root block it claims parity with
Lines 14-24 and 77-90 enumerate roughly fifteen runtime paths (`project.toml`,
`index-state.json`, `embeddings.db{,-wal,-shm}`, `usage.db{,-wal,-shm}`,
`librarian.db{,-wal,-shm}`, `debug.log*`, `write.lock`, `write.lock.holder`,
`cc_session_id`, `guide_hints/`, `tantivy/`, …). The `crates/*/` block covers two.

## Hypotheses tried
1. **Hypothesis:** the stub's tracked files are an ignore-rule violation.
   **Test:** `git check-ignore -v` on the tracked paths, plus the hypothetical-path table above.
   **Verdict:** rejected, and the correction matters. A tracked file is unaffected by
   `.gitignore` — ignore rules gate *untracked* candidates only. So the tracked pair is not a
   violation of the rules; it is a **consequence** of the rules never having covered them. The
   defect is prospective, not historical.

2. **Hypothesis:** this is dead-directory cruft, i.e. a tidy-up rather than a defect
   (`get_guide("tracker-conventions")` excludes tidy-ups from the bug tracker).
   **Test:** ask whether the mechanism is confined to the dead directory. It is not — the
   `crates/*/` gap applies to any future crate, and `codescout-embed` is a live crate one
   `.codescout/` away from the same state.
   **Verdict:** rejected. The stub is the *evidence*; the gap is the defect. Raised by a peer
   session as seen-and-judged-not-filable, which was the right call about the stub and the
   wrong one about the rule — reproduced here rather than relayed.

## Fix
*Not fixed by this bug file.* Complete the enumeration at `.gitignore:25-27`: add
`crates/*/.codescout/usage.db`, `write.lock`, `write.lock.holder`, `librarian.db{,-wal,-shm}`,
`embeddings.db{,-wal,-shm}`. Do **not** add a blanket `crates/*/.codescout/` rule — line 11's
note explains why the root block is an enumeration and not a directory rule, and the same
reasoning applies per crate.

Separately and optionally: `git rm --cached` the stub's two tracked runtime files, or prune
`crates/librarian-mcp/` entirely since it holds no code. That is the tidy-up half and is
genuinely optional; the rule completion is not.

**The durable half is a guard, not a longer list.** A rule set maintained by transcription
will drift again the next time the root block grows. The shape that would hold: a test
asserting that for every path the root block ignores, the `crates/*/` equivalent is also
ignored — derived from the root block rather than restated, so adding a root rule without its
per-crate twin reds. That is a real test this repo can express (`git check-ignore` is
scriptable and already used in `tests/`), and it is the difference between fixing this
instance and closing the class.

## Tests added
None yet. See the Fix's last paragraph for the guard worth having — asserting parity by
DERIVING the crate-level expectation from the root block, so the assertion cannot go stale by
someone extending one list and not the other. A test that restates both lists is the same
transcription defect one level up.

## Workarounds
Never `git add -A` or `git add crates/` in this repo — already the standing rule for a
different reason (`get_guide("tracker-conventions")`: a directory is `-A` scoped to a
subtree). Stage explicit paths. Before committing anything under a crate, check
`git status --short --untracked-files=all crates/`.

## Resume
Complete `.gitignore:25-27` per Fix, then decide on the stub. Then consider the parity guard,
which is the part that closes the class rather than the instance.

## References
- `.gitignore:11-12` (why `.codescout/` is not blanket-ignored — the committed audit shards)
- `.gitignore:14-24`, `:77-90` (the root runtime enumeration this claims parity with)
- `.gitignore:25-27` (the two-line crate block)
- `crates/librarian-mcp/` (the non-crate stub; the gap's only landed instance)
- `docs/trackers/issue-clusters/IC-14-guard-narrower-than-its-name.md` (cluster membership)
