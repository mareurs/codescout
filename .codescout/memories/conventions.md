# codescout — Conventions

## Pre-Commit Requirements

Four commands, and **the order is load-bearing** — not a checklist you may reorder:

1. `cargo fmt`
2. `cargo clippy --workspace --all-targets --features local-embed -- -D warnings`
3. `cargo test --workspace --no-default-features` — the **LEAN** lane, runs **THIRD**
4. `cargo test --workspace` — the default lane, runs **LAST**

All four must pass. No exceptions.

**The bare `cargo clippy -- -D warnings` is not a gate step.** It lints only the root
package's non-test targets with default features, so it passes trees CI fails. CI runs both
forms (`ci.yml:50` narrow, `:61` wide — verified 2026-08-31); only the wide one reaches
`#[test]` code and `codescout-embed`'s feature-gated `local` module. Running the narrow one
locally buys nothing the wide one does not already cover.

**Why the lean lane runs third and the default lane last.** The workspace shares one
`target/`, and `tests/cli_artifact.rs` resolves `target/debug/codescout` by path at run
time. The lean lane leaves a librarian-less binary there, so a later default-features run
execs it and fails 10 of 11 tests with `unrecognized subcommand 'artifact'` — reading
exactly like a feature-gating regression in whatever you just committed. Ending on the
default lane rebuilds it correctly, so following the gate cannot leave the trap armed for
the next session. Costs ~8s.

**Both test lanes carry `--workspace`, and the lean one is `test` not `check`.** Bare
`cargo test` builds only the root package's targets, so every workspace-member test is
invisible to it — an inline `#[cfg(test)]` module included, not just `tests/`. A `check`
compiles the lean targets without running them, so a lean-only *runtime* failure never
surfaces.

Full evidence and measurements: CLAUDE.md § *Development Commands*. Executable summary with
the build commands and the binary-freshness probe: memory `development-commands`.
## Error Handling

- `RecoverableError` for expected, input-driven failures → `isError: false` (sibling calls survive)
- `anyhow::bail!` for genuine tool failures → `isError: true` (fatal)
- Write tools return `json!("ok")` — never echo content back. Reserve richer responses only for genuinely new info (e.g. LSP diagnostics after a write).
- See `get_guide("error-handling")` for the full decision tree

## Repair-and-Continue Input Handling

When a tool can **deterministically** infer intent from a malformed input, repair it, execute, return the result, and attach an advisory `corrections` note — do NOT return `RecoverableError`. Every `RecoverableError` forces a retry = a second LLM call; repairing saves it. Reserve `RecoverableError` for input that is genuinely missing or ambiguous.

- **Repair + note** (exactly one correct reading): synonym (`file_path`/`relative_path`/`file`→`path`, `regex`/`query`→`pattern`), mechanical shape inversion (`{op:{field,value}}`→`{field:{op:value}}`), coercible scalar.
- **Error + teaching hint** (never guess): absent input ("no implicit current file"), unknown-not-op field, uncoercible/ambiguous value.
- **Writes get a higher bar than reads:** accept an *explicit* write target; never auto-*guess* one (a wrong guess on a write is unrecoverable).
- Repair at the tool's input boundary; keep core validators strict (defense-in-depth). Notes ride only on object-shaped responses; `json!("ok")` tools repair silently.
- Helpers: `crate::fs::PATH_PARAM_ALIASES`, `require_str_param_or_hint`, `filter::repair_inverted_leaves`. Full rationale: ADR `docs/adrs/2026-07-10-repair-and-continue-input-handling.md`. No prompt-surface change needed — the correction note is self-describing at the moment of the mistake.

## MCP Entry Point

`call_content()` is the MCP entry point — it handles buffer routing via `OutputGuard`.
Do NOT call `call()` directly from `ServerHandler`; it bypasses buffer routing.

## Progressive Disclosure

Every tool defaults to compact/exploring output. Full bodies only with `detail_level: "full"`.
Two modes via `OutputGuard` (`src/tools/output.rs`): Exploring (compact, capped at 200 items) / Focused (full detail, paginated).
Overflow → actionable hint + `by_file` distribution map, never truncated garbage.
See `docs/PROGRESSIVE_DISCOVERABILITY.md` before adding or modifying any tool.

## Agent-Agnostic Design

codescout serves multiple agents (Claude Code, Copilot, Gemini, Antigravity). The server must be **self-contained** — its gate logic, error messages, and instructions must guide any MCP client to the right tool without relying on external hooks:

- Error hints name codescout tools (`edit_code`, `grep`), never host tools (`Edit`, `Write`) — the LLM must never be nudged to sidestep codescout via native file editing.
- The companion plugin (`codescout-companion`) adds Claude-Code-specific enforcement (PreToolUse hooks), but the server itself must not depend on it.
- Project workflows, standards, and artifacts live in the repo (`docs/…`, `CLAUDE.md`), NOT in `claude-plugins/`. Plugin content is a thin UX wrapper over repo-resident source of truth — a non-CC client must never be locked out. When in doubt: would a Copilot user lose access? Then it belongs in the repo.

## Environment-Agnostic Tuning — never ship a measured constant as a default

Sibling of Agent-Agnostic Design, and the same question one layer down: users run **different models, different hardware, different corpora**. A number we measured is an observation about *our* setup, never a default for theirs. We can only ever test for us.

- **A threshold expressed in the units of a third-party model's output is model-specific by construction.** `RERANK_MIN_SCORE = -5.0` is the worked example: sensible for a logit-emitting reranker, provably inert against TEI's sigmoid `[0,1]`, and no single value spans both. Same trap for `*_BOOST`, `*_WEIGHT`, overfetch multipliers, latency budgets, and batch sizes.
- **Prefer scale-free forms.** Relative/rank rules (top-k, fraction of the top score, percentile of the observed batch), ratios between values we measured in the same run, or "keep everything above the largest gap in this batch". These carry no assumption about the scale, so they survive a model swap.
- **When a constant is unavoidable, default to INERT and make the active value opt-in.** Nobody should inherit our calibration silently. A filter that is off by default and documented as needing calibration is honest; one preset to our number quietly degrades every other setup.
- **A scale-free form does not conjure signal that is not there.** 2026-08-07: `cross-encoder/ms-marco-MiniLM-L-6-v2` (22M params, MS MARCO, lexically driven) scored a hedged correct answer at `1.05e-5` and an absurd one at `1.14e-5` — overlapping bands, so *no* rule, absolute or relative, separates them. Check that the bands separate at all before shipping any filter, and ship the probe so the user can check on their model.
- **Ship the calibration probe, not the calibration.** Give users the four-tier measurement (directly-answers / same-domain / tangential / unrelated, the middle tier adversarial), state what our model measured *labelled with the model name*, and let them derive their own value. See F-13 in `docs/trackers/release-promotion-session-log.md` for why a two-point probe is not enough to characterise a scale.

- **Classify before flagging — one of these three classes is in scope, not all `unwrap_or`s.**
  1. *Compatibility constants* — a wrong value means **broken**, not degraded, and there is nothing to calibrate. `model_dim` `unwrap_or(768)` (`src/retrieval/config.rs`) must match the embedding model or nothing works. **Out of scope.**
  2. *Degradation deadlines* — a wrong value costs one slower or coarser call and self-heals on the next. `LSP_FIRST_CALL_BUDGET` (2 s, `src/lsp/mod.rs`): over budget, callers fall back to tree-sitter marked `"lsp": "warming"` while the start continues detached. **Out of scope**, provided the fallback is documented at the constant.
  3. *Tuning constants* — a wrong value silently returns **worse results**, with no error and no self-healing, and the value was calibrated against a specific model / corpus / hardware. **The only class the rule targets.**
  Discriminating question: *would a wrong value raise an error, or quietly degrade output?* Quietly degrade ⇒ in scope. Swept 2026-08-07: exactly one in codescout (`bm25_boost`), four in researcher (all `researcher:src/config.rs` — a sibling repo, so the path does not
  resolve from here).
- **Labelling is a legitimate fix, and usually the right one.** The rule is not "no constants", it is "no *unlabelled* constant presented as a universal default". A number that names the model/corpus/hardware it was measured on and points at the probe that re-derives it has discharged the obligation; changing the value is a separate decision needing its own evidence. Worked example of the fixed shape: the `CODESCOUT_BM25_BOOST` block in `.env.example`. **A contradiction between two labels is the tell that neither was written as an observation** — `.env.example` said "Tuned to 3.0", `.env.gpu` said "5.0 was the measured peak (35/75)", and both were true of different sweeps.

## Testing Patterns

- Cache-invalidation tests use a **three-query sandwich** (baseline → assert-stale → invalidate → assert-fresh), not two. The step-3 stale assertion is what makes it a *regression* test — it fails if the system ever changes to eager-reread. Canonical example: `did_change_refreshes_stale_symbol_positions` in `src/lsp/client.rs`.
- **Never write env-mutating tests.** `EnvGuard` + `#[serial_test::serial]` is option B in `docs/conventions/test-env-isolation.md` and is marked **NOT VIABLE** there: `#[serial]` locks only among *annotated* tests, so any untagged test elsewhere reading the same var still races, and `std::env::set_var` is process-global with no way to scope it. The guard restores faithfully and the race happens anyway. The class was removed project-wide (default-build UB warnings 119 → 0), which also deleted the two exemplars this memory used to point at; the ruling itself lives in `docs/conventions/test-env-isolation.md`. Only `src/agent/mod.rs` still uses one — server-stack gated, explicitly exempt. Do option A instead: resolve env once at the edge into a struct and pass it inward (`LibrarianEnv::from_env`, `ServerEnv::from_env`, and `EmbedEnv::from_real_env` in `src/retrieval/config.rs` are the shape), then unit-test the decision as a pure function taking its inputs as arguments — no guard, no `#[serial]`, no cleanup, and they run in parallel. *Corrected 2026-08-11: this entry previously mandated the banned pattern, and a plan written from it propagated the instruction into three task briefs before an implementer read the convention doc and refused. See `docs/trackers/local-onnx-embedding-session-log.md` F-3.*
- Fallback-path tests gated on an exact-match miss must avoid substring overlap in fixtures (else the exact path fires first → false green). Assert on a path-specific marker so a mis-route fails loudly. See `docs/trackers/reconnaissance-patterns.md` R-16.

## Prompt Surface Consistency

Any tool rename, addition, or behavior change requires updating all three prompt surfaces.
The build-time test `prompt_surfaces_reference_only_real_tools` catches stale tool names; `claude_md_contains_no_deprecated_tool_names` guards `CLAUDE.md`.
Bump `ONBOARDING_VERSION` only for `onboarding_prompt` surface changes — never for `server_instructions`.
The static slice cap is **1900 characters** (`STATIC_SLICE_CHAR_BUDGET`), below the measured 2048-**char** client cliff (`CLIENT_INSTRUCTIONS_CHAR_LIMIT`, `src/prompts/mod.rs:39`). Count characters, not bytes — the old rule said "2200 bytes", which was both the wrong unit and above the cliff it existed to protect, and stayed green for months. Never raise it; move content to a `get_guide(topic)` **and wire the topic's trigger**. Full operational detail (bump matrix, verify-slice hazard) + the writing style guide live in `src/prompts/README.md`.


## Bug Tracking

Every noticed bug gets a file in `docs/issues/YYYY-MM-DD-<slug>.md` (copy `_TEMPLATE.md`).
Archive to `docs/issues/archive/` once the fix is **verified on `experiments`** — gate green plus a regression test. Reaching `master` is NOT required (`experiments` is never deleted). Archive via `doc(action="move", …)`, never a bare `git mv` (`id = sha256(abs_path)`, so a hand-move orphans the catalog row). An experiments-only archive carries the SHA labelled `experiments` plus a Resume line saying the master-side SHA is still owed — an experiments SHA orphans on rebase.
Every `## Root cause` cites both the mechanism (`path:line`) and what **measured** it (command + date); a mechanism inferred from code but never observed at runtime says so (W-13).
Frontmatter/status vocabulary: `get_guide("tracker-conventions")`.

## Commit Style

Conventional commits: `feat`, `fix`, `docs`, `chore`, `refactor`, `test`.
Subject: imperative, ≤72 chars.
**Pass the body through `git commit -F <file>`, never an inline `-m`.** Backticks inside a double-quoted `-m` are live **command substitution** — and this project's style puts backticks on every branch, tool, path, and memory topic it names. Measured 2026-08-07 (F-16): `` `conventions` `` in a `-m` body ran `conventions`, printed `command not found`, and interpolated the empty result, so the word vanished from the message while `git commit` exited 0 and the push went out. The same construct would *execute* any command a message quotes for documentation. Write the message to a scratchpad file and `-F` it; the body is then never shell-interpreted. Cherry-pick to master after all checks pass + MCP verify.
Full release + ship procedures: `docs/RELEASE.md`. SHA-citation + cross-repo-prefix discipline: memory `gotchas`.
