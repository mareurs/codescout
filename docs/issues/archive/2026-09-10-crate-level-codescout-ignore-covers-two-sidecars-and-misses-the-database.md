---
id: 4a5dcdc1802be5ea
kind: bug
status: fixed
title: 'BUG: crate-level .codescout ignore rules cover two sidecars and miss the 250 MB database'
owners:
- marius
tags:
- cluster/guard-narrower-than-its-name
closed: 2026-09-10
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

**Fixed** in `0fc39268` (patch-id `306ceba1fdfa3ae0bf01ec59a6ba859b77e223bb`) on `experiments`.

The enumeration is complete: **30** `crates/*/.codescout/…` rules, one per root-block rule,
replacing the two-line block. It is **derived, not transcribed** — the 28 missing twins came out
of the failing test rather than out of reading the file, and the block itself was generated from
the root rules.

That mattered, and is the correction this section owes its own earlier draft: the list above
named roughly ten files and the § Evidence estimate said *"roughly fifteen"*. The derived count
is **30**. A hand count of the root block taken while writing the fix said 29. Every one of those
under-counts has the same cause as the original defect — a pattern with an internal `/` is
anchored either way, so `.codescout/write.lock` reads as "at any depth" and gets skipped when
enumerating "the root block". Three readings of this file by the same author, three different
numbers, all low.

No blanket `crates/*/.codescout/` rule, for the reason this file already gave. And **no blanket
rule plus a negation either**, which the earlier draft did not rule out and should have: git will
not descend into an excluded directory to reconsider a negation. That is not a preference — it
does not work, and it is already measured in this repo for `**/.claude/`
(`docs/issues/archive/2026-09-07-gitignore-claude-rule-is-root-anchored-so-nested-dirs-escape.md`).

The stub's two tracked runtime files are **deliberately still tracked**. `.gitignore` never
untracks, so that is a separable `git rm --cached` decision about a directory holding no code,
and it is not part of completing the rule set.
## Tests added

`every_root_codescout_ignore_rule_has_its_crates_twin` (`tests/hook_config.rs`) — parity by
**derivation**: it parses every root-anchored `.codescout/…` rule out of `.gitignore` and
requires a `crates/*/` twin for each. Adding a root rule without its twin reds. Verdicts are
taken against a probe crate that does not exist, so the answer is about rules, not disk.

**Mutations, on the production path** (`.gitignore` itself, never the test's inputs):

| mutation | result |
|---|---|
| drop `crates/*/.codescout/usage.db` | RED, naming that exact rule |
| append a root rule with no twin | RED, naming the twin to add |

The second is the direction that matters — it is the drift the guard exists for, and no assertion
covered it before. The first alone would have been monotone under "the twin block never grows".

Non-vacuity, three ways, because a derived assertion can pass by deriving nothing:

- A **per-pattern control** asserts the root rule matches the path built from it. Without it, a
  bad probe construction reports *every* twin as missing and the failure reads as a `.gitignore`
  gap rather than as the test's own construction being wrong.
- A floor of `>= 20` rules catches a parser that silently matches nothing. Deliberately well under
  the derived 30 — it is there to catch an empty population, not to pin a number every added rule
  falsifies.
- `the_root_codescout_rule_parser_discriminates` pins that the parser rejects comments,
  negations, and all three scoped siblings. A parser that accepted `crates/*/.codescout/…` would
  compare the twin block against itself and pass while every root rule went unchecked.

The refusal message names the exact lines to add **and** rules out both wrong repairs, since a
guard's remedy text is untested by construction.
## Workarounds
Never `git add -A` or `git add crates/` in this repo — already the standing rule for a
different reason (`get_guide("tracker-conventions")`: a directory is `-A` scoped to a
subtree). Stage explicit paths. Before committing anything under a crate, check
`git status --short --untracked-files=all crates/`.

## Resume

Nothing owed on the rules — the enumeration is complete and guarded.

One separable decision is left open on purpose: whether to `git rm --cached` the stub's two
tracked runtime files, or prune `crates/librarian-mcp/` entirely. It holds no `Cargo.toml`, no
`.rs` files, and `Cargo.toml`'s `members` does not list it. That is a tidy-up, not this defect.
## References

Cited by content rather than by line, because this fix moved every line number the earlier
draft of this section named.

- `.gitignore`, the `NOT ignored, deliberately` note — why `.codescout/` is not blanket-ignored
  (the committed audit shards)
- `.gitignore`, the `/.codescout/…` root block — the enumeration the crate block mirrors
- `.gitignore`, the `crates/*/.codescout/…` block — 30 rules, derived from the above
- `tests/hook_config.rs`, `every_root_codescout_ignore_rule_has_its_crates_twin` — the guard
- `docs/issues/archive/2026-09-07-gitignore-claude-rule-is-root-anchored-so-nested-dirs-escape.md`
  — why a blanket rule plus a negation is not an available repair
- `crates/librarian-mcp/` (the non-crate stub; the gap's only landed instance)
- `docs/trackers/issue-clusters/IC-14-guard-narrower-than-its-name.md` (cluster membership)
