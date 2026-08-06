---
id: d965b9fd79298e46
kind: bug
status: draft
title: 'BUG: audit-doc-refs high-finding count varies between identical runs — the CI gate is non-deterministic'
tags:
- audit_doc_refs
- ci
- flake
- lsp
- determinism
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

Not confirmed. The leading hypothesis, in order of fit:

1. **LSP warmth changes the verdict band, not just the latency.** `resolve_file_symbol`
   returns `Verdict::Unknown` (severity `low`) when no LSP is available, and
   `SymbolMissing` (`high`) when an LSP answers and the symbol is absent. So the
   *same* stale symbol ref lands below the gate on a cold run and above it on a warm
   one. That is exactly a 2→0 / 1→0 flap, and it needs no bug anywhere else —
   the severity model simply makes gate membership a function of LSP readiness.
2. Mux ownership races on a shared socket, related to the known concurrent-instance
   class (memory `gotchas` § LSP).

If (1) is the cause, the honest fix is a policy one: a degraded scan must not be
allowed to produce a *green* gate either. Today `scan_meta.degraded: true` is
reported and ignored.

## Evidence

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
## Fix

Not implemented. Two candidate directions, and they are not exclusive:

1. **Make degradation gate-visible.** If `scan_meta.degraded` is true, `--fail-on high`
   should not report a clean pass; either exit non-zero with a distinct code or refuse
   to render a verdict. A gate that silently downgrades its own coverage is worse than
   a gate that fails loudly.
2. **Make symbol resolution deterministic for CI.** Either require the LSP for the
   languages present (fail the run if unavailable), or drop `file_symbol` refs to a
   non-gating band entirely so the gate depends only on the filesystem. The second is
   cheap and removes the whole class; the first keeps the check but needs a warm-up
   barrier.

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
- `docs/issues/2026-08-06-docs-ref-drift-backlog-across-eleven-subdirs.md` — the
  backlog this was found while working.
