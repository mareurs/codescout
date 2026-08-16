---
id: d61e3b535e2cae4a
kind: bug
status: fixed
title: 'BUG: artifact(create, kind="bug") defaults status to `draft` — not in the bug vocabulary, and invisible to the canonical open-bug query'
tags:
- librarian
- artifact
- tracker-conventions
- bookkeeping
- silent-failure
closed: 2026-08-06
---

## Summary

`artifact(action="create", kind="bug", …)` without an explicit `status` writes
`status: draft`. For bug files that value is **not in the vocabulary at all** —
`get_guide("tracker-conventions")` defines exactly six: `open`, `investigating`,
`fixed`, `mitigated`, `wontfix`, `zombie`. `draft` belongs to the *tracker* vocabulary
(`active | draft | archived | superseded`), and the create path applies the tracker
default to every kind.

The consequence is not cosmetic. The canonical triage query in that same guide is:

```
artifact(action="find", kind="bug", status="open")
```

A `draft` bug matches nothing. So a bug file created correctly in every other respect —
right path, right slug, right frontmatter, committed, pushed — **never appears in the
answer to "what's open?"** and no gate anywhere notices.

## Symptom (Effect)

Found by walking the ledger a few hours after filing a bug in the same session:

```
artifact(find, kind="bug", status="open")  → 5 items
```

The five were all pre-existing. Absent: the bug filed earlier that same day
(`docs/issues/2026-08-06-audit-doc-refs-gate-is-nondeterministic.md`), which had been
written, committed, pushed, and cited from three other documents. Its row read
`status: draft`, and the file's own frontmatter agreed:

```yaml
kind: bug
status: draft
```

Also absent from the same query, for a **different** reason worth separating:
`docs/issues/2026-06-19-kotlin-lsp-uncapped-jvm-heap.md`, which is
`status: investigating` — a perfectly valid, non-terminal bug status that
`status="open"` simply does not match. See § Root cause, second half.

## Reproduction

```
artifact(action="create", kind="bug", title="…", rel_path="docs/issues/…md", body="…")
artifact(action="find", kind="bug", status="open")     # the new bug is not there
artifact(action="find", filter={"rel_path":{"contains":"<slug>"}})   # status: draft
```

Deterministic. No timing, no LSP, no environment dependence.

## Environment

`experiments`, codescout v0.15.0. Observed 2026-08-06 while triaging the open-bug
ledger at the end of the doc-drift work.

## Root cause

Not read in code yet — the behaviour is confirmed from the outside only, so treat the
attribution below as a hypothesis with a named place to check.

**Half one — the default.** `artifact(action="create")` almost certainly applies one
default status for all kinds, and that default is the tracker-appropriate `draft`. The
guide documents two disjoint vocabularies keyed on `kind`, and the create path appears
not to key on it. Check the `status` defaulting in `src/librarian/tools/create.rs`
(or wherever `create` normalises frontmatter) against
`get_guide("tracker-conventions")` § *Status vocabulary*.

**Half two — the query, which is a separate defect.** The bug vocabulary has **two**
non-terminal states, `open` *and* `investigating`, but the canonical query names only
`open`. So even with the default fixed, "what's open?" under-reports any bug someone
marked `investigating` — which is what the guide tells them to do when actively working
it. The documented triage query is structurally incapable of listing all live bugs.

That is the more interesting half: the default produces one wrong row, the query hides a
whole legitimate state.

## Evidence

- `get_guide("tracker-conventions")` § *Status vocabulary* (bug files) lists six values;
  `draft` is not among them. The tracker table immediately below lists `draft` as a
  tracker status. Two vocabularies, one defaulting path.
- The guide's own archive rationale already warns about exactly this failure mode in a
  neighbouring form: *"holding the file back only grows a pile of `fixed`-but-unarchived
  bugs that no query ever surfaces (`find(kind="bug", status="open")` filters on
  `status`, not on path)"*. The same filter-on-status reasoning cuts the other way for
  `draft` and `investigating`, and that was not anticipated.
- Two live examples on `experiments` today, one per half: a `draft` bug (default) and an
  `investigating` bug (query).

### Fresh reproduction 2026-08-06 ~18:35Z — the fix is committed but not live

Filing `docs/issues/2026-08-06-audit-doc-refs-gitignore-cap-ignores-the-index.md` through
`artifact(action="create", kind="bug")` with **no** `status` param produced `status: draft`
again — at `a53f1760`, which is downstream of the fix commit `e58ad463`. The source carries
the fix; the **running MCP binary does not**, because it was never rebuilt (`cargo rb`) and
reconnected after that commit. The new bug had to be corrected with an explicit
`artifact(action="update", patch={status:"open"})`, and until that correction it was invisible
to the canonical open-bug query — this bug reproducing itself during the filing of another.

Consequence for this bug's own verification: the regression test passes under `cargo test`,
but the live tool surface still misbehaves. **Do not archive on the strength of the test
alone** — the archive gate is behaviour verified through the live MCP path, and this
reproduction is evidence that is not yet true.

## Hypotheses tried

None — the reproduction is deterministic and the cause is a defaulting decision, so
there is nothing to bisect. Reading the create path is the next step, not an experiment.

## Fix
**2026-08-06 — all three parts implemented.** Gate green: fmt, `clippy --all-targets -D
warnings`, 3504 tests / 0 failed.

1. **Default keyed on `kind`.** `src/librarian/tools/create.rs` now resolves status
   through `resolve_status(kind, requested)`: `kind: bug` → `open`, everything else →
   `draft`. The hypothesis in § Root cause was right — `create.rs:118` read
   `a.status.as_deref().unwrap_or("draft")`, with no reference to `kind`.
2. **Out-of-vocabulary bug statuses refused**, with the six listed in the error so the
   caller does not have to go read a guide. Tracker statuses are deliberately *not*
   validated: that vocabulary documents four values but the guide also says unrecognised
   ones "appear as active", so free-form is load-bearing there. Narrowing only where the
   vocabulary is closed.
3. **The documented query fixed at all three sites that tell you to run it** —
   `src/prompts/guides/tracker-conventions.md`, `src/prompts/guides/project-activation-bootstrap.md`
   (auto-injected on project activation, so the highest-leverage one), and
   `docs/issues/_TEMPLATE.md`. The two remaining mentions in
   `tracker-conventions.md` and `docs/RELEASE.md` were left alone on purpose: they
   explain *how the filter behaves* ("filters on `status`, not on path") rather than
   prescribing a query, and are correct as written. `CLAUDE.md` needed no change — it
   delegates to the guide instead of quoting the query.

**Empirical confirmation of the harm, before the fix:** the corrected `in` filter
returned **8** live bugs where `status="open"` returned **5**. Three were hidden — one
`draft` (this bug's half one) and one `investigating` (half two), plus the bug filed
minutes earlier in the same session.

**Tests:** a create-then-read-back round trip asserting a new bug is `open` *and* that a
new tracker is still `draft` (over-match guard — "default everything to open" would have
passed a one-sided test and broken the tracker vocabulary); and a refusal test that also
loops every one of the six documented statuses to confirm none was caught by the new
validation.

Not implemented. Three parts, and they are independent:

1. **Key the default on `kind`.** `kind: bug` → `open`; `kind: tracker` → `draft`;
   leave other kinds as they are. One-line-ish, and it is the half that silently
   produced an out-of-vocabulary value.
2. **Reject out-of-vocabulary bug statuses**, or at least warn. A `status` that is not
   one of the six on a `kind: bug` row is a bookkeeping bug every time; `RecoverableError`
   with the six listed is the house style for this class
   (`get_guide("error-handling")`).
3. **Fix the documented triage query** to cover both non-terminal states, in
   `get_guide("tracker-conventions")`, `CLAUDE.md`, and anywhere else it is quoted:

   ```
   artifact(action="find", kind="bug",
            filter={"status": {"in": ["open", "investigating"]}})
   ```

   Part 3 is worth doing **even if 1 and 2 are declined**, because it is the half that
   hides a state the guide actively instructs people to use.

### Implemented and verified on the live MCP path

**Fix commit:** `e58ad463` — **experiments** (`git branch --contains e58ad463` lists
`experiments` only; not yet on `master`). `resolve_status(kind, requested)` in
`src/librarian/tools/create.rs` resolves the default per kind and refuses an
out-of-vocabulary bug status.

**Live verification 2026-08-06, after a release rebuild and MCP reconnect** — the check this
file's Resume demanded, because the earlier reproduction proved the source fix was inert in a
stale binary:

- `artifact(create, kind="bug")` with no `status` → `status: open`
- `artifact(create, kind="bug", status="wip")` → refused:
  `status "wip" is not a bug status; use one of: open, investigating, fixed, mitigated, wontfix, zombie`

CI also ran the regression tests on `fcb6598f`, whose tree contains this fix: three test jobs
green (ubuntu no-features, windows default, windows local-embed).

## Tests added

None yet. The regression test for part 1 is a create-then-find round trip asserting the
new row appears in the canonical query; for part 2, that an invalid status is refused;
for part 3, nothing code-side — it is a documentation contract.

## Workarounds

Pass `status="open"` explicitly on every `artifact(create, kind="bug", …)`, and triage
with the `in` filter above rather than `status="open"`.

## Resume

Fixed and verified — no investigation remains. **One bookkeeping item outstanding:**
`e58ad463` is an **experiments** SHA. After the cohort reaches `master` (fast-forward or
cherry-pick), record the master-side SHA in the Fix section above via `git rev-parse HEAD` on
`master`. An experiments SHA orphans on rebase, and nothing re-reads `docs/issues/archive/` to
repair it.
## References

- `get_guide("tracker-conventions")` § *Bug files* — the six-value vocabulary and the
  canonical query, both quoted above.
- `docs/issues/2026-08-06-audit-doc-refs-gate-is-nondeterministic.md` — the bug this was
  found through; its status has been corrected to `open`.
- `docs/issues/2026-06-19-kotlin-lsp-uncapped-jvm-heap.md` — the `investigating` example
  for half two.
- `CLAUDE.md` § *Bug Tracking* — quotes the status vocabulary and points at the guide.
