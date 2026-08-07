---
id: d965b9fd79298e46
kind: bug
status: fixed
title: 'BUG: audit-doc-refs'' summary counters did not partition n_refs_found, and a mid-index LSP produced false SymbolMissing verdicts (both fixed 2026-08-07; the original gate flap was removed earlier by severity re-banding)'
tags:
- audit_doc_refs
- ci
- flake
- lsp
- determinism
closed: 2026-08-07
opened: 2026-08-06
owner: marius
related:
- '523233935cc53bc4'
severity: medium
---

## Summary
> **FIXED 2026-08-07 in `6e608545` (experiments). CI 15/15 attempt 1.** All three questions this
> file carried are resolved, and two of them turned out to be one defect:
>
> 1. **The counters now partition.** They were three independent filters covering 7 of the 10
>    `Verdict` variants, so `ResolvedBasename`, `AmbiguousBasename` and `External` were counted in
>    `n_refs_found` and in no bucket — 2,426 of 47,094 refs on this repo, invisible in the summary.
>    Replaced by one `bucket()` fn whose match is **exhaustive**, so adding a variant is a compile
>    error until it is bucketed. A test can assert a partition; an exhaustive match is one.
>    Verified live: full corpus `47141 = 14240 + 9063 + 23838`, residual **0**.
> 2. **A mid-index LSP no longer fakes missing symbols.** `resolve_file_symbol` treated
>    `Some(syms)` without the name as authoritative absence — the branch comment even said so
>    (*"the LSP responded (not offline), so an empty symbol list means the symbol genuinely isn't
>    there"*), which is false during warm-up. It now cross-checks tree-sitter
>    (`ast_has_symbol`): if the AST has the name, the verdict is `Unknown` + `note_degraded`
>    rather than `SymbolMissing`. This was the tally-drift mechanism, since `SymbolMissing` is the
>    only LSP-dependent verdict in the broken bucket and that branch never marked the scan
>    degraded.
> 3. **`degraded` is honest again**, as a free consequence of (2). Measured after the fix: `true`
>    with `offline: ["rust"]` on a cold full-corpus run, `false` with zero `lsp_behind_ast`
>    findings on a warm scoped run — so neither silent nor saturated, the failure mode
>    `note_degraded`'s own docstring warns about.
>
> **The original symptom — the flapping high count — was already fixed before this file was
> reopened, and that is why 99 invocations could not reproduce it.** `default_severity` reserves
> `high` for `Missing`/`FileMissing`, both deterministic filesystem answers, so no LSP-dependent
> verdict can reach the gating band. It is structurally impossible now, not merely unobserved.
> Closing as **fixed** rather than `zombie` for exactly that reason: the cause was found and
> removed.
>
> Regression guards: `summary_counters_partition_every_verdict`,
> `resolved_basename_counts_as_resolved_everywhere`,
> `ambiguous_basename_still_counts_against_the_gate`,
> `resolver_defers_to_the_ast_when_the_lsp_lags_behind_disk`,
> `resolver_still_reports_symbol_missing_when_the_ast_agrees`. The last two are the mutation-
> verified pair — making `ast_has_symbol` return `true` unconditionally kills the second, and
> without it that mutation would silently stop the audit reporting stale symbol refs at all.
>
> **Resume:** N/A. Master-side SHA still owed after the `experiments` -> `master` promotion (the
> SHA above is experiments-side and orphans on rebase).

`codescout audit-doc-refs --fail-on high` returned **different high-severity finding
counts for the same tree and the same command** within a few minutes. A gate that
flaps cannot be trusted to mean "the docs drifted": a red run may be noise, and —
worse — a green run may be luck.

Observed while working the doc-drift backlog on `experiments`, after the extractor
precision work. Not caused by that work: the varying refs are `file_symbol` kinds
whose resolution depends on the LSP, and the scan already self-reports
`scan_meta.degraded: true` with `lsp_languages_offline: ["unknown"]`.

**Update 2026-08-06, later the same day — the stakes inverted.** When this was filed the
gate had several hundred high findings, so a flap of one or two changed nothing: the run
was red either way. `Audit Doc Refs` now reports **0** high findings
(`a7c1d7f6` + `297e1074`). At zero, a single flap into a `high` verdict is the difference
between green and red, and there is no cushion absorbing it.

So the practical severity rose without the bug changing. Two consequences worth acting on:

- **A spuriously red run is now possible on an unchanged tree.** Anyone seeing
  `Audit Doc Refs` fail should re-run before believing it, and should compare the finding
  list rather than the exit code — a flapped finding will be a `file_symbol` ref.
- **The `scan_meta.degraded` hole is now the bigger half.** A degraded scan that reports
  green is indistinguishable from a genuinely clean one, and green is now the expected
  state, so the failure mode is silent rather than loud. Fix direction 1 below (make
  degradation gate-visible) should be preferred over waiting for a forcing recipe.

The zero also gives a much sharper reproducer than the one in § Reproduction: any flap at
all now shows up as a **non-zero exit on a tree that just exited zero**, which is trivial
to detect in a loop — run the full scan N times and compare exit codes, no `--paths`
narrowing and no finding-set diffing required.
## Symptom (Effect)

Two consecutive sweeps over the same directories, no edits in between:

```
# first sweep
docs/conventions high: 1
docs/evals       high: 2

# immediately after, listing the same findings
docs/conventions → (empty)
docs/evals       → (empty)

# three more repeats
run 1: evals=0 conventions=0
run 2: evals=0 conventions=0
run 3: evals=0 conventions=0
```

The count is the *gating* number — `--fail-on high` exits 1 on any of them. So the
same commit can pass or fail CI depending on which side of the flap the runner lands.

## Reproduction

1. `cargo build --release --bin codescout`
2. Run repeatedly, comparing the count:

```
./target/release/codescout audit-doc-refs --no-emit-tracker --fail-on high \
  --json --project . --paths "docs/evals/**/*.md" \
  | jq '[.findings[]|select(.severity=="high")]|length'
```

Not yet reduced to a deterministic reproducer — the flap was seen once in each
direction and has not been forced on demand. That is the first thing the next
session should fix, because without a forcing recipe any "fix" is unfalsifiable.

## Environment

- `experiments`, codescout v0.15.0, Linux.
- `rust-analyzer` reached through the LSP **mux** (`/run/user/1000/codescout-rust-mux-*.sock`).
  Log lines show both `mux process ready for rust` and `mux already running for rust,
  connecting to …` across runs — i.e. some runs start the mux, others attach to a
  warm one.
- `typescript-language-server` is also started during a scan (~102 ms).

## Root cause

**Mechanism identified in code (2026-08-06), not inferred.** `resolve_file_symbol` ends in
a two-arm match on the LSP symbol lookup, and the two arms sit in *different severity
bands*:

```rust
match <lsp document_symbols lookup> {
    Some(syms) => if symbol_tree_contains(&syms, name) { Resolved }
                  else { verdict_with_drops(Verdict::SymbolMissing, ..) },  // High
    None => { ctx.degraded_languages.borrow_mut().push(lang.to_string());
              Resolution { verdict: Unknown, severity: Low, .. } }          // Low
}
```

`default_severity` maps `SymbolMissing` → **High** and `Unknown` → **Low**. So for one
stale `file_symbol` ref, *whether the LSP answers* decides whether the finding gates.
Answered-and-absent is a gating drift report; unanswered is a silent low. Nothing about
the reference changed — only whether a language server responded on that call.

That is the whole flap, and it needs no bug anywhere else: the severity model makes gate
membership a function of per-request LSP responsiveness. It also explains the shape
observed twice — counts moving *up* then back *down* — which a monotonic warm-up story
would not produce.

**It is per-ref, not per-run.** Scan-level availability is constant while the verdict is
not — whatever moves is one lookup, not the session. (The original form of this paragraph
read *"`scan_meta.degraded` is permanently `true`"*; that was true when written and is now
inverted — see § Status of these fix options below. The per-ref conclusion is unaffected,
since it never depended on which constant value the flag held.)

The earlier ranking of mux-ownership races (kin to the concurrent-instance class in memory
`gotchas` § LSP) is now secondary: it may explain *why* an individual lookup occasionally
returns `None`, but the reason a returned `None` changes the **exit code** is the fork
above.

### What this makes the fix

The honest options are narrower than they looked:

1. **Take `file_symbol` out of the gate.** Cap `SymbolMissing` below `high`, or route it
   through a drop. One line, removes the entire class, and costs only that a genuinely
   renamed symbol reports at `med` — which is where a `file_line` range check already
   sits. Recommended.
2. **Require the LSP for languages present in the repo** and fail the run when a lookup
   returns `None`, so the two arms stop straddling the gate. Correct but needs a warm-up
   barrier and makes CI depend on language-server availability.

**Not viable as written:** "exit non-zero when `scan_meta.degraded`". That flag is saturated
— `"unknown"` is pushed for every ref into a file whose extension is outside the six
languages `detect_language` knows, so it is `true` on every run including all 15 green CI
runs. Fixing the flag to mean "a server I expected was missing" is a prerequisite, and
arguably its own defect.
### Status of these fix options — verified 2026-08-07, and option 1 already shipped

Re-read the code before acting on the two options above; both of this section's premises had
moved since it was written.

**Option 1 is DONE.** `default_severity` (`src/librarian/tools/audit_doc_refs/severity.rs`)
now reads `Missing | FileMissing => High`, `SymbolMissing | AnchorMissing | LineOob |
AmbiguousBasename => Med`, everything else `Low`. Its docstring cites this bug file by path,
and `default_severity_gates_only_on_deterministic_filesystem_verdicts`
(`src/librarian/tools/audit_doc_refs/resolver.rs`) pins the split verdict-by-verdict. So
`SymbolMissing` was capped below `high` exactly as recommended, and the recommendation should
not be made a third time.

**Consequence for the gate — the flap is not merely unreproduced, it is now structurally
impossible.** `high` is reserved for `Missing` and `FileMissing`, and both are deterministic
filesystem answers on an unchanged tree. No LSP-dependent verdict can reach the `high` band,
so at `--fail-on high` no amount of language-server variance can move the exit code. That is
the mechanism behind § Evidence's "99 invocations, exit code and high count never moved": the
runs are confirmation of a shipped fix, not an unexplained absence. It also means item 3 of
the direction decision is closer to **fixed** than to `zombie` — the original symptom had a
cause, and the cause was removed.

**The `degraded` premise inverted.** "Not viable as written" rested on the flag being
saturated `true`; `note_degraded` (`src/librarian/tools/audit_doc_refs/resolver.rs`) now excludes `"unknown"` languages for
precisely that reason, with a docstring that also cites this bug file. Measured 2026-08-07:
`degraded: false` with `lsp_languages_offline: []` on a fresh-clone full-corpus run, and false
in all 57 paired warm/cold runs. So the flag is no longer saturated — it is now *silent* on
the case that matters, which is the opposite failure and a genuinely different problem: a
partial-but-successful `document_symbols` reply calls neither `note_degraded` branch.

**What the severity re-banding did NOT fix, and is what remains open here.** The three summary
counters are computed from **verdicts, not severities** (`build_response`), so
`SymbolMissing` still lands in `n_refs_broken` regardless of its band. The resolved <-> broken
migration under cold start is therefore untouched by option 1 and is the live defect: the gate
is deterministic, the *tally* is not. Anyone reading this section for the tally must not
mistake option 1's landing for a fix to it.

**Measured vs deduced** (per the `## Root cause` convention in `docs/issues/_TEMPLATE.md`):
measured 2026-08-07 — `default_severity`'s mapping and its test read directly from the source;
`degraded: false` from a full-corpus run and from the 57 paired runs; counter arithmetic
(47094 found vs 44668 bucketed repo-wide, 44 vs 41 on a single uncapped file). Deduced, not
measured — that this repo's language servers return partial symbol trees during warm-up. The
cheapest falsification is logging `syms.len()` on the `Some(syms)` + `!contains` branch across
one cold and one warm run for the same file.
## Evidence
### Second instance, observed 2026-08-06 at the zero boundary — and it changes the picture

Immediately after archiving the backlog bug and repointing its three inbound citations,
two **consecutive** full scans on an identical tree:

```
run A: {"exit": 1, "high": 1, "broken": 8494}
run B: (same command, seconds later)  high: 0     <- empty finding list
```

Then 8 further runs: `high=0` every time. So the flap is real and it is rare — this was
roughly the 43rd invocation of the session after 42 clean ones — and it landed exactly
where it costs the most, at the zero boundary where one finding is the whole verdict.

**The evidence was lost, and that is my error, not the tool's.** Run A printed only the
count and discarded the JSON; run B was a *fresh scan*, so by the time the finding list was
asked for, the flap had passed. The identity of the flapped ref is therefore unknown.
**Always persist the full JSON per run when hunting a flake** — a count is not evidence, and
re-running to "look closer" destroys the only sample.

### `scan_meta.degraded` is permanently true, which kills Fix direction 1 as written

Across all 10 runs, without variation:

```
degraded=true  offline=["unknown"]
```

Not a transient. `"unknown"` is what `detect_language` returns for a path whose extension
has no LSP mapping, and `resolve_file_symbol` pushes that onto `degraded_languages` before
returning `Verdict::Unknown`. Any `file_symbol` ref into such a file therefore marks the
whole scan degraded — permanently, benignly, on every run.

So **"exit non-zero when `scan_meta.degraded`" would fail every single run**, including all
15/15-green CI runs. The signal is saturated and carries no information as it stands.
Before that fix direction is usable, `degraded` has to distinguish *a language whose server
is missing* from *a file that has no server by design* — which is arguably the real defect
here, since the flag exists to tell a caller its coverage was incomplete and currently
cannot.

That also weakens hypothesis 1 as the sole explanation: degradation is constant while the
verdict is not, so whatever moves must be finer-grained than the scan-level flag — a
per-ref LSP response, not a per-run LSP availability.

- `scan_meta` in every local run of the full scan: `{"degraded": true,
  "lsp_languages_offline": ["unknown"]}`. The tool already knows it is not seeing
  the whole picture, and still emits an authoritative exit code.
- The flapping refs were in `docs/conventions/` and `docs/evals/`, both of which had
  just been edited to fix genuine drift — so the surviving refs there are
  symbol-bearing, matching hypothesis 1.
- `docs/issues/2026-07-18-symbols-overview-include-body-ignored-and-search-flake.md`
  (open) records a sibling flake: `symbols` search mode occasionally 0-matches then
  succeeds on retry. Same substrate.


**Superseded 2026-08-07 — it is permanently FALSE, which is worse.** At `cdfbbe0f`, `degraded` was
`false` and `lsp_languages_offline` was `[]` in **all 57 runs** recorded below, including every cold
run that lost 60-69 resolutions to an LSP that was not ready. The claim above does not hold and the
inverted version is the finding: the scan reports itself undegraded while silently reclassifying
refs it could not check. Whatever `degraded` currently tracks, it is not *"the LSP was not ready"*
— the one condition a determinism gate would need it to track. Fix direction 1 is therefore not
dead-as-written, but it cannot be built on `degraded` in its current form.

### 2026-08-07: the non-determinism is FORCED on demand — and it is not the gate

The previous Resume's step 1, executed. Release binary at `cdfbbe0f`. The project's rust mux is
`codescout mux --socket /run/user/1000/codescout-rust-mux-7e868829c00fa9b2.sock --idle-timeout 180
-- rust-analyzer` (PID observed 1660662). **cold** = `pkill` that process immediately before the
run, with the kill confirmed per-run by counting `mux process ready` in stderr rather than assumed;
**warm** = mux already up. 57 invocations total.

Note `--idle-timeout 180`: the mux self-exits after three idle minutes, so warm-vs-cold is not an
artificial state. Two identical gate invocations more than 180 s apart land in different LSP states
with nobody killing anything, which is why back-to-back repetition (all 42 earlier attempts) can
never surface this — back-to-back runs are always warm.

**`docs/issues/**/*.md`, 8 runs — the cleanest signal:**

| arm | `n_refs_resolved` | `n_refs_broken` | sum | high | rc | degraded |
|---|---|---|---|---|---|---|
| warm ×3 | 2768, 2768, 2768 | 998, 998, 998 | 3766 | 0 | 0 | false |
| cold ×5 | 2707, 2704, 2699, 2709, 2701 | 1059, 1062, 1067, 1057, 1065 | 3766 | 0 | 0 | false |

Three facts:

1. **Warm is exactly deterministic** — identical to the digit across repeats.
2. **Cold is a race** — five distinct values, spread 10, none equal to the warm value. What varies
   is how much of rust-analyzer's index existed at the moment each ref was queried.
3. **The sum is conserved at 3766 in all eight runs.** Refs do not appear or vanish; they *migrate*
   from `resolved` to `broken`.

**Full corpus — same shape, and the diff is two lines.** warm ×3 all `12500 / 8570`; cold ×3
`12423/8647`, `12427/8643`, `12426/8644`; sum 21070 in all six. `diff warm1.json cold1.json` is
exactly and only:

```
<   "n_refs_resolved": 12500,        >   "n_refs_resolved": 12423,
<   "n_refs_broken": 8570,           >   "n_refs_broken": 8647,
```

`overflow.total` is **47299 in all six**, and all **879** `by_file` finding counts are identical
(verified after checking the extraction was non-empty — 879 lines each — rather than reading an
empty-vs-empty diff as agreement).

**Localisation.** The delta lives under `docs/`: `docs/issues` **64**, `docs/trackers` 2,
`docs/superpowers` 2, and **zero** in adrs / manual / research / conventions / plans / archive /
architecture, `src/**`, and `tests/**`.

**Latency.** Full scan warm 7.1-9.7 s, cold ~14.9 s — cold roughly doubles.

### What this does to this bug's own title claim

The original title said the **high-finding count** varies and the **gate** is non-deterministic.
Across every pair above — 9 `docs/` subtrees, 4 scopes, the full corpus — the exit code and the high
count were identical in warm and cold. In `docs/issues`, where 64 refs migrate, `high` is **0** in
both arms; because findings are emitted most-severe-first and only 50 are shown, a shown `high=0`
alongside `med=50` means there are genuinely no high findings being hidden by the cap. The gate
verdict did not flap once, in either direction, under the only mechanism now known to perturb the
scan.

What *is* non-deterministic is the **summary tally**, and it is inconsistent with the findings it
summarises: 64 refs change from `resolved` to `broken` while the finding set's size and its per-file
distribution stay bit-identical. The tally and the findings are computed on different paths, and
only the tally is LSP-timing-sensitive. So the headline numbers move while the substance does not.

**Caveat on the cap.** For any scope containing the delta, `high` saturates at the OutputGuard's
50-finding limit, and `audit-doc-refs` has no `--limit` flag, so per-severity totals are not
observable through `--json` at full scope. The `high=0` reasoning above works only because zero is
below the cap and the ordering is most-severe-first. A scope where the true high count exceeds 50
remains unmeasured for severity movement — the uncapped route is the emitted `audit_issues`
tracker, deliberately not taken here because it mutates the catalog.
## Hypotheses tried

**Exit-code stability at zero — 6 consecutive full scans, all `exit=0`** (2026-08-06,
`297e1074`, warm LSP mux). Run bare, no `--paths` narrowing:

```
for i in 1 2 3 4 5 6; do
  ./target/release/codescout audit-doc-refs --no-emit-tracker --fail-on high \
    --json --project . >/dev/null 2>&1; echo "run $i: exit=$?"
done
```

So the flap does **not** reproduce on demand under a warm mux, which bounds its rate
rather than clearing it: the original observation was two flaps in one sitting, both on
`--paths`-narrowed scans of `docs/evals/` and `docs/conventions/`. Two candidate
explanations survive, and they are distinguishable:

- **Cold vs. warm mux.** Both original flaps came early in a sweep, where one invocation
  starts the mux (`mux process ready for rust`) and later ones attach
  (`mux already running for rust`). All six runs above attached to a warm mux. **Next
  experiment:** kill the socket between runs (`ls /run/user/$UID/codescout-rust-mux-*`)
  and repeat. If the exit code moves, hypothesis 1 in § Root cause is confirmed and the
  fix is a policy decision, not a hunt.
- **`--paths` narrowing changes which LSP languages are touched**, so a narrowed scan can
  be degraded in a way a full scan is not. Worth checking because CI runs the full scan,
  which would make the flap CI-invisible — and therefore mean the local observation
  overstates the CI risk.

Not yet tried: forcing the cold-mux case, and diffing the finding *sets* rather than the
counts across runs.
**Narrowed-scan stability — 6 runs each of the two directories where the flap was
actually seen, all 0** (2026-08-06, `297e1074`):

```
for i in 1 2 3 4 5 6; do
  ... --paths "docs/evals/**/*.md"       -> high=0
  ... --paths "docs/conventions/**/*.md" -> high=0
done
```

**So the flap does not reproduce in either shape — 12 attempts, 0 reproductions.** That is
worth stating plainly rather than leaving the LSP-warmth hypothesis looking confirmed: it
is one observation, and it has now survived no replication.

What is still certain, and is the part that matters: the two measurements were taken
minutes apart on an unchanged tree with the same binary and the same jq selection, and
they disagreed (`conventions` 1→0, `evals` 2→0). Something moved that was not the input.

A third explanation now looks at least as likely as the two above, and it is the one the
original observation is most consistent with: **the flapping counts came from a
rapid-fire sweep of ~11 back-to-back invocations**, each starting a fresh process against
the shared mux socket. Contention or a mid-restart mux during that burst would degrade one
invocation and not its neighbours. The 12 replication attempts were paced (two per
iteration), which is exactly the condition that would *not* trigger it.

**Next experiment, and it is cheap:** re-run the original 11-directory sweep verbatim,
several times, and watch for any non-zero count. That reproduces the *burst*, which the
paced runs deliberately did not. If it flaps there and never in paced runs, the fix is
about mux contention under concurrent short-lived processes, not about warmth — and the
gate could be made deterministic simply by warming the LSP once before scanning.
**Burst reproduction — 2 sweeps of 15 back-to-back invocations each, no flap** (2026-08-06).
This was the experiment the paragraph above nominated, and it failed to reproduce:

```
for sweep in 1 2; do for d in <15 docs subdirs>; do <scan --paths docs/$d/**/*.md>; done; done
→ sweep 1: nonzero: none
→ sweep 2: nonzero: none
```

### Running total: 42 invocations, 0 reproductions, across three shapes

Paced full scans (6), paced narrowed scans (12), and two burst sweeps of 15 invocations
each (30). The bug has resisted every attempt. What remains certain is only the original pair of
measurements: minutes apart, unchanged tree, same binary, same jq selection, and
`docs/conventions` went 0 → **1** → 0 and `docs/evals` 0 → **2** → 0 across three
consecutive sweeps, with the only edits in between landing in *other* directories
(`docs/manual/**`). So it went up and came back down on its own.


**Updated 2026-08-07: 99 invocations (42 + 57).** Zero reproductions of a *high-count* or
*exit-code* change, now including 30+ explicitly paired warm/cold trials designed to force one.
One reproduction, on demand and every time, of *tally* non-determinism. The distinction is the
result: the thing this bug was opened about has not been seen in 99 tries, and a different,
adjacent non-determinism is now forceable in a single command.
### The reason further replication here may be worthless

A candidate that deserves stating because it changes what the next session should do:
**the flapping refs may no longer exist.** `docs/conventions/test-env-isolation.md` and
`docs/evals/reconnaissance-output.md` were both edited during that same stretch to fix
genuine drift, and the refs removed were symbol- and path-bearing. If hypothesis 1 in
§ Root cause is right — LSP warmth flipping a `file_symbol` between `Unknown`/low and
`SymbolMissing`/high — then the specific refs that could flap are gone from those files,
and every replication attempt since has been run against a corpus with the cause removed.

That makes the 42 clean runs weak evidence rather than strong. **Do not close this on
count of clean runs.** The way to settle it is to reproduce the *conditions*, not the
corpus: check out the tree as it stood at `2377e9c1` (before those two files were fixed)
and sweep that, where the flappable refs still exist.

This is also a methodological note worth keeping: replicating a flake *after* fixing what
flaked tests nothing, and it is easy to do accidentally when the flake surfaces during a
cleanup pass.
## Fix
**2026-08-06 — both halves implemented. The flap can no longer move the exit code, and
`scan_meta.degraded` carries information for the first time.**

1. **`SymbolMissing` no longer gates** (`c8efc17a`). `high` is now reserved for verdicts
   that are deterministic filesystem facts — `Missing`, `FileMissing`. `SymbolMissing`
   joins `LineOob` and `AmbiguousBasename` at `med`: still reported, no longer able to
   swing an exit code. This is Fix option 1 from § Root cause. Nothing had asserted the
   old value, so the whole verdict-to-band map is now pinned by a test.
2. **`degraded` de-saturated.** `note_degraded` skips `"unknown"` — the value
   `detect_language` returns for any extension outside its six, i.e. a `.sh`, `.toml` or
   `.md` path part that never had a server to lose. Measured before and after on the same
   tree:

   | | before | after |
   |---|---|---|
   | `scan_meta.degraded` | `true` (every run) | **`false`** |
   | `lsp_languages_offline` | `["unknown"]` | **`[]`** |

   **This is the prerequisite that made Fix direction 1 in § Root cause unusable, and it
   is now met.** Gating on `degraded` was a proposal that would have failed every run
   ever, including all fifteen green CI jobs; the flag can now actually mean "a server I
   expected was missing".

**Why this stays open rather than closing here.** The two changes remove the *consequence*
— a flap can no longer decide green-versus-red — without establishing *why* an individual
`document_symbols` call occasionally returns `None`. That question is still open, it is
shared with `docs/issues/2026-07-18-symbols-overview-include-body-ignored-and-search-flake.md`,
and it now has a clean instrument: with `degraded` no longer saturated, a run where a real
language server fails to answer is visible in `lsp_languages_offline` instead of being
buried under a permanent `["unknown"]`.

**A note on the mistake worth not repeating.** One of this file's own earlier fix
directions — "exit non-zero when `scan_meta.degraded`" — was written before anyone checked
what that field actually contained on a real run. It was wrong for a reason a single
`jq .scan_meta` would have shown. Reading the field beat reasoning about it, which is the
same lesson as W-5 in `docs/trackers/release-promotion-session-log.md`.

**Superseded — kept for the record.** The block below was the pre-implementation plan, and it
contradicted the "both halves implemented" opening above until 2026-08-06. Both directions are
now resolved:

1. ~~Make degradation gate-visible.~~ **Invalidated on measurement** — `degraded` was
   saturated `true`, so gating on it would have failed every run ever, including all fifteen
   green CI jobs. It is de-saturated above, and deliberately *not* wired to the exit code.
2. ~~Make symbol resolution deterministic for CI.~~ **Its cheap option was adopted** —
   `SymbolMissing` moved to a non-gating band, so the gate now depends only on deterministic
   filesystem facts. The warm-up-barrier variant was not pursued.

**Confirmed live on a release rebuild, 2026-08-06.** A full default scan through the freshly
built MCP binary: 884 files, 8500 broken refs, **0 high**, `exit_code: 0`,
`scan_meta.degraded: false`, `lsp_languages_offline: []`. The earlier before/after table was
measured mid-session; this re-measures it on the binary the fix actually ships in.

## Tests added

None yet. A regression test needs the forcing recipe first — see Reproduction.

## Workarounds

Re-run the audit before believing a red `Audit Doc Refs` job, and treat a single
green run on a symbol-heavy diff as unconfirmed.

## Resume

Step 1 of the previous Resume is **done** — there is now a forcing recipe, so any proposed fix is
falsifiable. Steps 2 and 3 are superseded by what it found.

**The next action is a decision about which bug this is, and it is the maintainer's.** The evidence
now supports two separable defects, and they want different fixes:

1. **The tally is non-deterministic and disagrees with the findings** (confirmed, forceable).
   `n_refs_resolved`/`n_refs_broken` migrate by up to 69 refs under LSP cold start, exactly
   compensating, while the finding set stays bit-identical. Either the tally should be computed from
   the findings (making it definitionally consistent), or refs whose resolution was skipped because
   the LSP was not ready need a third bucket rather than being folded into `broken`.
2. **`degraded` does not track LSP readiness** (confirmed). It was `false` in all 57 runs, including
   every cold run that lost resolutions. Until it does, no determinism gate can key off it, and a
   consumer cannot tell an incomplete scan from a clean one.

**And the original symptom should probably be reclassified.** At 99 invocations with 0 reproductions
— 30+ of them adversarially paired — the high-count flap is not supported by anything measured here.
Candidate explanations worth one cheap check each before spending more on it: the two original
observations may have compared *different scopes*, may have read the tally rather than the finding
count, or the tree may genuinely have changed between them (uncommitted edits during a doc-heavy
session). If none can be established, `zombie` with a re-open trigger is more honest than `open`.

**Do not** re-run back-to-back repetition to hunt the original flap. 42 attempts of that shape
found nothing, and the reason is now known: back-to-back runs are always mux-warm, and warm runs are
exactly deterministic.

The measurement harness is `scratchpad/audit-determinism.sh` (session-local, not committed):
`audit-determinism.sh <outdir> <warm|cold> [n] [paths-glob]`, printing per-run high/med/low,
resolved/broken/unknown, `degraded`, whether a mux was spawned, bytes, and ms. Re-create it from
the Evidence section above if needed; the load-bearing details are the `pkill` target and counting
`mux process ready` to confirm the kill landed.
## References

- `src/librarian/tools/audit_doc_refs/resolver.rs` — `resolve_file_symbol`, and the
  `degraded_languages` field on `ResolveCtx` that records the cold-LSP path.
- `src/librarian/tools/audit_doc_refs/severity.rs` — `default_severity`, where
  `SymbolMissing` is `High` and `Unknown` is `Low`.
- `docs/issues/2026-07-18-symbols-overview-include-body-ignored-and-search-flake.md` —
  sibling flake, still open.
- `docs/issues/archive/2026-08-06-docs-ref-drift-backlog-across-eleven-subdirs.md` — the
  backlog this was found while working.
