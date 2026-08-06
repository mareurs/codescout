---
id: d965b9fd79298e46
kind: bug
status: open
title: 'BUG: audit-doc-refs high-finding count varies between identical runs — the CI gate is non-deterministic'
tags:
- audit_doc_refs
- ci
- flake
- lsp
- determinism
opened: 2026-08-06
owner: marius
related:
- '523233935cc53bc4'
severity: medium
---

## Summary

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

**It is per-ref, not per-run.** `scan_meta.degraded` is permanently `true` (see § Evidence),
so scan-level availability is constant while the verdict is not. Whatever moves is one
lookup, not the session.

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

1. Force the flap: run the same `--paths` scan with the mux killed vs. warm
   (`ls /run/user/$UID/codescout-rust-mux-*`) and confirm the count moves. That
   settles hypothesis 1 in one experiment.
2. If confirmed, decide between Fix 1 and Fix 2 — this is a gate-semantics call,
   not a code detail.
3. Cross-check against the open `symbols` flake bug; if the mux is implicated in
   both, they may share a fix.

## References

- `src/librarian/tools/audit_doc_refs/resolver.rs` — `resolve_file_symbol`, and the
  `degraded_languages` field on `ResolveCtx` that records the cold-LSP path.
- `src/librarian/tools/audit_doc_refs/severity.rs` — `default_severity`, where
  `SymbolMissing` is `High` and `Unknown` is `Low`.
- `docs/issues/2026-07-18-symbols-overview-include-body-ignored-and-search-flake.md` —
  sibling flake, still open.
- `docs/issues/archive/2026-08-06-docs-ref-drift-backlog-across-eleven-subdirs.md` — the
  backlog this was found while working.
