---
kind: bug
status: fixed
tags:
- security
- run_command
- windows
- tokenizer
closed: null
opened: 2026-08-08
owner: marius
related: []
severity: medium
---

# BUG: the dangerous-command gate tokenizes with `split_whitespace`, not with the shell's rules — and the tokenizer written for that job has no callers

## Summary

`run_command` executes every command through a POSIX shell on both platforms (`sh -c` on
Unix, Git Bash `bash -c` on Windows, since WIN-32). The safety layer that decides whether
a command is dangerous read it with `split_whitespace()`, and the POSIX tokenizer written
to close that gap — `crate::platform::posix_tokenize` — had **zero production call
sites**. So the layer that judged a command and the shell that ran it could disagree about
where the words are.

**Fixed 2026-08-08**, in two commits: `is_dangerous_command` gained a normalized second
pass (a union, so it can only add catches), and the six `split_whitespace` helpers were
moved onto `shell_tokens` (a replacement, so each conversion is a real behaviour change
with its own discriminating test). See *Fix* for the per-helper direction, and *The
seventh site* for what this work uncovered and handed to its own bug file.
## Symptom (Effect)

No user-visible failure is filed against this. It is a latent divergence, and it is
recorded because the code asserted the opposite. `src/platform/mod.rs` carried:

```
/// One tokenizer here is load-bearing rather than tidy: this feeds the security layer's
/// dangerous-command and pipeline checks, so tokenizing with rules the executing shell
/// does not use would let a command read as safe here and then behave differently when
/// it actually runs.
```

That comment named the hazard precisely and claimed it was handled. It was not.

## Reproduction

```
git grep -n 'shell_tokenize\|posix_tokenize' -- 'src/**/*.rs'
```

Every hit is a definition, a delegation (`unix.rs` / `windows.rs` → `super::posix_tokenize`),
or one of the tokenizer's own 6 tests. Nothing calls `platform::shell_tokenize`.
Then read `is_dangerous_command` (`src/util/path_security.rs`): `split_whitespace()`.

## Environment

codescout 0.15.0, branch `fix/windows-paths-and-doctor` (PR #10) and `experiments` alike —
the divergence predates the PR on Unix; the PR makes it live on Windows by replacing
`cmd.exe` with bash.

## Root cause

**Corrected 2026-08-08 — the original text below named the wrong subject, and the real
shape is worse.** It said `is_dangerous_command` "splits on whitespace". It does not
tokenize at all: it runs regexes over the **raw** command string (`src/util/path_security.rs`,
read this session). The `split_whitespace` readers are elsewhere in the same file.

The accurate statement: the security layer holds **three** different models of the same
string, and the only model that matches the shell that will execute it is the unused one.

| reader | model | on the security path? |
|---|---|---|
| `is_dangerous_command` | regexes over the raw string, no tokens | yes |
| `stage_trims`, `grep_is_counting`, `is_unbounded_lhs`, `has_recursive_flag`, `extract_grep_pattern`, `check_source_file_access` | quote-blind `split_whitespace` | yes |
| `posix_tokenize` / `shell_tokenize` (`src/platform/mod.rs`) | quote- and escape-aware, matches `sh -c` / Git Bash `bash -c` | **no callers** |

A fourth model lives next door: `OutputBuffer`'s buffer-only classifier has its own
path-likeness heuristic, and a command it judges buffer-only skips the dangerous-command
gate outright — tracked separately in
`docs/issues/archive/2026-08-08-buffer-only-gate-misses-tilde-and-home.md`.

So this is not "one function tokenizes wrongly", and it cannot be fixed by correcting one
call. **There is no shared notion of what a command's tokens are**, which is why the
sibling bug file states the same defect in different words ("the heuristic answers *does
this word look like a path to a reader*, when the question that matters is *will the shell
turn this word into a path*"). Both are instances of: safety is decided by reading the
command as text, while the shell decides what runs. Every gap between those two models is
a hole.

**measured 2026-08-08:** `grep -n 'split_whitespace' src/util/path_security.rs` → six
production sites at lines 755, 774, 785, 816, 898, 952/962, none inside
`is_dangerous_command`; `symbols(name="is_dangerous_command", include_body=true)` → pure
regex body, no tokenization. `git grep -n 'shell_tokenize\|posix_tokenize'` → definitions,
two delegations, six tests, no production call site (first measured on
`refs/pull/10/merge`, re-confirmed on `experiments`).

<details><summary>Original text, kept for the record</summary>

> Two independent readers of the same string, neither aware of the other:
>
> - `src/util/path_security.rs` — `is_dangerous_command` and its helpers split on
>   whitespace and run substring regexes over the raw command.
> - `src/platform/mod.rs` — `posix_tokenize` implements quote/escape-aware splitting,
>   and `shell_tokenize` exposes it per-platform. Nothing consumes either.
>
> **measured 2026-08-08:** `git grep -n 'shell_tokenize\|posix_tokenize'` over
> `refs/pull/10/merge` returned definitions, two delegations and 6 tests — no production
> call site. Confirmed independently by two reviewers in the PR #10 review.

The `posix_tokenize`-has-no-callers half was correct and measured. The
`is_dangerous_command`-splits-on-whitespace half was asserted without reading the body,
and `src/platform/mod.rs`'s own doc comment repeated it — corrected there in the same pass.

</details>
## Evidence

### The gate itself is NOT trivially evadable

Worth recording so a future session does not over-scope the fix: `is_dangerous_command`
matches substrings over the **raw** command, so `echo $(rm -rf x)` still trips it.
Command substitution does not hide a dangerous verb. The divergence is about word
boundaries, not about hiding text.

### Where it bites

`split_whitespace` sees `"a b"` as two words and `a\ b` as two words; bash sees one in
both cases. A path-shaped or verb-shaped token that the shell assembles from quoted
fragments is not the same token the gate inspected.

## Hypotheses tried

1. **Hypothesis:** `shell_tokenize` is called somewhere behind a trait or macro.
   **Test:** `git grep` across all of `src/`, plus a read of `path_security.rs`'s
   parsing helpers. **Verdict:** rejected — the only callers are its own tests.

## Fix

**Complete — status `fixed` as of 2026-08-08.** All four string models named in *Root
cause* now agree with the shell, or diverge on purpose with the reason written down. The
work landed in two commits: the dangerous-pattern union first, the six helpers second.

### Landed on `experiments`

`is_dangerous_command` matches every pattern against the raw command **and** a
shell-normalized form (`shell_normalized`, new, in `src/util/path_security.rs`). That is
`posix_tokenize`'s **first production call site** — the tokenizer written for this job had
none.

Union, not replacement, and that choice is the safety argument: the raw string catches
shapes a token list does not, so matching only the normalized form would LOSE catches.
Matching both can only add them, so every command the gate rejected before, it still
rejects — true by construction, and pinned by `raw_only_matches_are_still_caught`.

What it closes: quote and escape evasion that leaves the raw string unmatchable while the
shell still runs the destructive command — `r''m -rf /tmp/x`, `r""m -rf /tmp/x`,
`rm -r\f /tmp/x`, `'rm' -rf /tmp/x`, `git push --force'' origin main`.

**The cost, stated rather than discovered later.** Rejoining tokens erases the difference
between "two words" and "one quoted word", so `grep 'rm' '-rf' notes.txt` is now flagged.
That is a false positive and it is the price of the pass. Acceptable because a flag is not
a refusal (the caller re-invokes with the returned `@ack_*` handle), and because
`is_dangerous_command` has never checked command *position*, so this class already existed
via the raw pass — `grep 'rm -rf' notes.txt` was flagged long before normalization, since
the raw string literally contains `rm -rf`. Both are asserted in
`quoted_dangerous_text_is_flagged_and_the_raw_pass_did_it_first` so the property is
recorded rather than rediscovered.

A NUL-substitution scheme was written to keep quoted arguments un-bridgeable, then removed:
no case could be constructed where it changed the outcome, because for whitespace to sit
*inside* a token the quotes must sit outside it, which leaves the dangerous substring
intact in the raw string where the raw pass already finds it. Reinstating it needs a
demonstrated case, not a plausible one.

### Still open — what keeps this `mitigated`

*(Section kept under its original heading so the citations that point at it still
resolve. Nothing here is open any more.)*

The six `split_whitespace` readers — `stage_trims`, `grep_is_counting`, `is_unbounded_lhs`,
`has_recursive_flag`, `extract_grep_pattern`, `check_source_file_access` — now take their
tokens from `shell_tokens`, a thin wrapper over `posix_tokenize` that falls back to
`split_whitespace` when tokenization fails. **The fallback is the load-bearing part:** an
unclosed quote must never be a way to make a check answer "nothing to see here", so the
fallback reproduces exactly the pre-conversion model.

This was a REPLACEMENT, not a union, and the union's safety-by-construction argument does
not transfer — these helpers read head tokens and flags, so quote-awareness changes their
answers. Each conversion therefore carries a test that fails on the pre-conversion code.
What each one changed:

| helper | direction | what it closes |
|---|---|---|
| `check_source_file_access` | stricter | `'cat' src/main.rs`, `\cat …`, `c"at" …` read the file; the first token used to carry its quotes, match no blocked name, and skip the block outright |
| `has_recursive_flag` | stricter | `grep '-r' p .` is recursive to the shell; quoting hid an unbounded LHS from IL3 |
| `is_unbounded_lhs` | stricter | `'cargo' test` read as an unknown command and fell through to bounded |
| `stage_trims` | stricter | `'head' -50` read as an unknown command and was not a trimmer |
| `extract_grep_pattern` | correctness | `grep "foo bar" f` yielded `foo` — half a pattern, and the half that decides symbol-ladder vs generic hint. Returns `Option<String>` now; the tokenizer owns its output |
| `grep_is_counting` | **looser** | a quoted `-c` made a counting grep read as a trimmer and blocked a pipe IL3 is meant to allow |

`is_unbounded_lhs` also stopped matching `-maxdepth` as the substring `" -maxdepth "` and
now matches it as a token, so a tab-separated or quoted flag is found. That trades one
pathological case in the other direction — a file literally named `-maxdepth` — which is
pinned by an assertion so the next reader sees a decision rather than a gap.

The fourth model — `OutputBuffer`'s path-likeness heuristic — is fixed under
`docs/issues/archive/2026-08-08-buffer-only-gate-misses-tilde-and-home.md`, including the
discovery that it existed in two already-diverged copies.

### The seventh site, found while fixing the six

The list of six was never the whole layer. `il3_offending_lead` splits a pipeline with a
bare `segment.split('|')`, so a quoted `|` fabricates stages the shell will never create.
Measured on `2b1c9ec6`: `git log --grep='fix|head foo'` — a command with **no pipe** — is
reported as an IL3 violation, and the remediation the error prints back is an unterminated
fragment. Filed as `docs/issues/2026-08-08-il3-splits-pipeline-on-quoted-pipe.md`; it is
not a regression from this work (the split predates it), and it is scoped narrowly enough
to stand on its own rather than hold this bug open.
## Tests added

Four, in `src/util/path_security.rs`:

- `dangerous_command_catches_quote_and_escape_evasion` — the five evasions above. Each
  leaves the raw string unmatchable while the shell still runs `rm -rf` / `git push
  --force`. A revert to raw-only matching fails these.
- `raw_only_matches_are_still_caught` — guards the *other* arm. If someone later replaces
  the raw pass with the normalized one, these must keep passing on their own. Asserting
  both directions is the point: the union is the invariant, not either arm.
- `normalization_does_not_flag_benign_commands` — negative control. A gate returning
  `Some` unconditionally would pass every evasion case, so this belongs beside them.
  Includes quoted-but-benign commands, since normalization rewrites before matching.
- `quoted_dangerous_text_is_flagged_and_the_raw_pass_did_it_first` — records the
  false-positive class, and distinguishes the pre-existing instance from the one
  normalization adds.
- `unclosed_quote_still_gets_the_raw_pass` — a tokenizer error must fall back to the raw
  pass, never *skip* the gate. Recorded as a decision rather than incidental behaviour.

Eight more for the six-helper conversion, in the same file, under the comment banner
`The six split_whitespace -> shell_tokens conversions`:

- `a_quoted_recursive_flag_is_seen`, `a_quoted_count_flag_makes_grep_an_aggregator_again`,
  `a_quoted_multiword_grep_pattern_survives_whole`, `a_quoted_head_still_names_the_command`,
  `maxdepth_is_matched_as_a_token_not_a_substring`,
  `quoting_the_command_name_no_longer_bypasses_the_source_file_block` — one per conversion.
  Every assertion that pins a behaviour change is labelled with its old answer
  (`"was: false"`, `"was: None"`), so the failure message names the delta.
- `an_unclosed_quote_falls_back_instead_of_skipping_the_check` and
  `shell_tokens_never_returns_nothing_for_a_command_with_words` — the controls for the
  fallback. Without them, returning an empty token list on a tokenize error would be a
  universal bypass of all six checks and every other test here would still pass.

**The discrimination was measured, not assumed.** `shell_tokens` was temporarily reverted
to `split_whitespace` and the suite re-run: 7 failures, one in each of the six
conversion tests, each panicking on its labelled assertion. (The seventh was a
pre-existing test the probe disturbed, since the probe does not restore
`extract_grep_pattern`'s old `trim_matches` — probe noise, not a finding.) The two control
tests correctly did not fail; they assert unchanged behaviour. One assertion is reasoned
rather than probed: the tab case in `maxdepth_is_matched_as_a_token_not_a_substring`
discriminates against the old `contains(" -maxdepth ")` substring form, which the
tokenizer-only probe does not restore.

Gate: `cargo fmt`; `cargo clippy --all-targets -- -D warnings` clean; `cargo test` 3570
passed / 0 failed / 44 ignored.
## Workarounds

None needed; no known exploit. `acknowledge_risk` remains the escape hatch for a command
the gate wrongly blocks, and the gate's substring matching means the common dangerous
verbs are still caught regardless of word boundaries.

## Resume

Fixed and green; the remainder this section used to describe is done. Remaining
bookkeeping, in order:

1. **Confirm CI on `experiments`** at the commit carrying the six-helper conversion, then
   archive via `artifact(action="move", …)` — never a bare `git mv`, which orphans the
   catalog row (`id = sha256(abs_path)`).
   **Re-point the inbound citations in the same commit as the move.** This file is cited
   from `CHANGELOG.md` (`[Unreleased]` → Fixed), from
   `docs/issues/2026-08-08-il3-splits-pipeline-on-quoted-pipe.md`, and from the doc comment
   on `posix_tokenize` in `src/platform/mod.rs` — note that last one is **source**, which
   `audit_doc_refs` does not scan, so the gate will not catch it.
   `grep -rl 'security-layer-tokenizes-unlike-the-shell' . ` before moving.
2. **Label the fix SHA `experiments`.** An `experiments` SHA orphans on rebase, so the
   master-side SHA still needs recording after cherry-pick (task #14, on the operator's
   hold as of 2026-08-08).

Do **not** reopen this for the pipeline-splitting defect found during the fix — that is
`docs/issues/2026-08-08-il3-splits-pipeline-on-quoted-pipe.md`, filed separately and
scoped to `il3_offending_lead` alone.

If a future session wants to keep unifying this layer, the honest statement of what is
left is in that file plus the doc comment on `posix_tokenize` in `src/platform/mod.rs`,
which now enumerates what agrees with the shell and what deliberately does not. That
comment has been wrong twice in opposite directions; it asks you to run
`references(symbol="posix_tokenize")` before editing it, and that instruction is there
because it was earned.
## References

- `src/platform/mod.rs` — `posix_tokenize`, and the corrected comment pointing here
- `src/util/path_security.rs` — `is_dangerous_command`
- PR #10 review, 2026-08-08 — found independently by the platform/security and
  test-rigor reviewers
- `docs/issues/archive/2026-08-08-buffer-only-gate-misses-tilde-and-home.md` — sibling defect in
  the same layer: the `is_buffer_only` path heuristic, which gates this check
