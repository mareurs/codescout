---
kind: tracker
status: active
title: Archive cadence policy for usage / friction trackers
owners: []
tags:
  - archive
  - lifecycle
  - dx
  - convention
---

# Tracker — Archive cadence policy

## Why

Trackers (U-N, F-N, H-N, R-N, T-N) accumulate without a rhythm. `codescout-usage-frictions.md` reached **18 entries / 528 lines** by 2026-05-23; **13 of 18 are closed** but none have been archived. The pattern is the same across the other trackers. Two problems compound:

1. **Bloat without triage** — every reader (human or agent) loads the full 528 lines of history every time they look at the file. Closed-and-cited entries earn their place historically, but their cost lives in present-tense context budgets.
2. **"Shipped" is ambiguous** — T7 (2026-05-24) discovered that `Status: fixed-shipped` in U-N has been used to mean "shipped to experiments" silently, never "shipped to master." Four entries (U-7, U-8, U-15, U-17) had citations to orphaned experiments-side SHAs for fixes that never reached master users. T11 reconciled the citations; the semantic ambiguity remains.
3. **Citation drift on long-lived branches** — T11's reconciliation itself demonstrated the deeper problem: every rebase of `experiments` reassigns SHAs, so any non-master citation is a moving target. Within the same session, my own 8 commits were re-SHA'd by a concurrent agent's rebase. The U-N tracker's "fixed-shipped" claim is honest only at the instant it's written.

The proposal — define a policy that addresses **all three** dimensions:
1. When does an entry move from active to archived?
2. What does "shipped" actually mean — and how do we cite SHAs that survive rebases?
3. Where do archived entries live, and how are they recovered for forensic queries?



## Decision (2026-05-24)

Promoted from draft to active. Lean chosen for each design surface:

**1. Status vocabulary — chosen: A (SHA-citation qualifier).**
The T11 convention is the canonical format. New U-N / H-N / R-N closures cite as one of:
- `(master:<sha>)` — fix has shipped to master and is reachable by `git branch --contains <sha>` from master.
- `(experiments:<sha>, not-yet-on-master)` — fix exists on experiments only; awaiting cherry-pick or release cut. The qualifier is mandatory — bare `<sha>` without branch scope is ambiguous and not allowed.
- `(claude-plugins:<sha>)` / `<repo>:<sha>` — for cross-repo fixes. The repo prefix carries branch context (master assumed unless qualified).
- `(in-place)` — for files outside git (e.g. `~/.claude-kat/CLAUDE.md`). No SHA citation needed.

No new status enum values. `fixed-shipped` is retained as the closure label; precision lives in the citation.

**2. Archive trigger — chosen: ii + iii hybrid.**
- Eligibility: status is closed AND (a) SHA is on master, OR (b) closure is `wontfix` / `by-design` / `substrate-caught` (no SHA dependency), OR (c) cross-repo closure with verified target-branch coverage.
- Cadence: manual quarterly pass + accelerated by release cuts. Each archive pass moves eligible entries; ineligible entries (e.g. `experiments-only`) stay in the active file until promoted.
- First pass: today (2026-05-24) — pilot on the unambiguous category (b) cases to validate the flow.

**3. Archive destination — chosen: time-partitioned per-tracker archive files at `docs/trackers/archive/`.**
- Naming: `docs/trackers/archive/<tracker-basename>-<YYYY>-q<n>.md`. E.g. `docs/trackers/archive/codescout-usage-frictions-2026-q2.md`.
- Frontmatter on the archive file: `kind: tracker`, `status: archived`, `title: <original title> — archive 2026 Q2`, `tags: [..., archived]`.
- The active tracker stays at `status: active` and retains the entries that weren't yet eligible.

**4. Recovery — chosen: rely on the librarian's archived-but-indexed model.**
- `artifact(action="find", kind="tracker", include_archived=true)` returns archived trackers alongside active ones.
- Cross-references in active entries cite the archive artifact id, not the file path. The librarian's `id` field is durable across renames.
- No new infra.

### What this changes going forward

- Every new U-N / H-N / R-N closure must use one of the citation shapes above. The T11 convention is now mandatory, not optional.
- Quarterly archive pass: scheduled for end of Q2 2026 (June 30) and Q3 2026 (September 30). Execute via a focused session.
- Release-cut acceleration: when a release ships, run an archive pass against the shipped SHA range as part of the release checklist.

### Status of this proposal

Active. Implementation begins with the pilot pass today. Promote-when criterion (one option per surface + a first pass executed) is satisfied on commit.## Design surfaces (open)

### 1. Status vocabulary — qualifying "shipped"

The current `fixed-shipped` / `partially-shipped` / `wontfix` / `open` / `closed via H-X` is informal. Two paths to make branch-scope explicit:

- **A. Add the qualifier to the SHA citation, keep the status field unchanged.**
  T11 introduced this convention organically: `(experiments:<sha>, not-yet-on-master — awaiting cherry-pick)` and `(master:<sha>)`. Read-friendly; no schema change; relies on the *author* to use the qualifier consistently.
- **B. Add new status enum values.**
  `fixed-on-experiments` / `fixed-on-master` / `wontfix` / `open`. Schema-enforceable (a status field validator could reject `fixed-shipped` going forward). Stricter; requires migration of existing entries.
- **C. Both.**
  Status enum captures the *coarse* state (one of three values); SHA citation captures the *precise* commit. Most rigorous; highest taxonomy load.
- **Lean: A.** Smallest disruption, captures the distinction where it matters (the citation), leaves the lifecycle informal. The T11 convention has already shipped on 4 entries; doubling down on it is cheaper than introducing a parallel enum.

### 2. Archive trigger — when does an entry move?

- **i. Status + time.** Entry is `fixed-on-master` (or whatever closed status) AND `last_verified_at > 30 days ago` → archive. Strict but predictable. Issue: requires a `last_verified_at` field on every entry (currently only present on some).
- **ii. Status alone, manual archive pass.** Entry is closed → eligible. Archive is a scheduled manual operation (quarterly, or after each release cut). Cheap; relies on human cadence.
- **iii. Release-tied.** Archive on each release cut: every entry whose fix is in the released SHA range moves to archive. Tightest coupling to ship cadence; needs a release-cut rhythm to exist first.
- **iv. Promote-or-die.** Entry past its declared `promote-when` criterion + N days without action → flips to `wontfix` and archives. Aggressive; treats promote-when as a real deadline.
- **Lean: ii + iii hybrid.** Manual pass at quarterly cadence (cheap default), accelerated by release cuts when they happen. Promote-or-die (iv) is too aggressive — promote-when criteria are often legitimately deferred for good reasons.

### 3. Archive destination — where do they go?

- **a. One archive file per tracker, time-partitioned.**
  `docs/trackers/archive/codescout-usage-frictions-2026-q2.md`, etc. Easy to grep across history; partition keys avoid one mega-file. Lean.
- **b. Per-entry archive files.**
  `docs/trackers/archive/U-7.md`, etc. One file per archived entry. Maximally surgical but high file count.
- **c. Move to repo-wide archive directory.**
  `docs/archive/trackers/codescout-usage-frictions-<timestamp>.md`. Consistent with existing `docs/issues/archive/` pattern. Lean.
- **Lean: a or c, picking one.** Both are reasonable; pick the one that matches the existing archive convention. Currently `docs/trackers/archive/` is the active convention.

### 4. Recovery — how do archived entries get found?

The librarian indexes archived trackers but hides them by default (`status: archived`). `artifact(action="find", kind="tracker", include_archived=true)` should surface them. Cross-references in active entries that point at archived ones need to keep working — either via the artifact graph or by leaving forwarding stubs.

- **Lean: rely on the librarian's existing archived-but-indexed model.** No new infra. Active entries that reference archived ones use the librarian's link/find APIs, not file-relative paths.

## Counter-arguments

- **"Why archive at all? Trackers are append-only history; readers can scroll past closed entries."**
  Disagree. The 528-line load every read is a real cost when the median entry is `fixed-shipped` and only 4-5 are genuinely active. Archive is the standard fix for append-only history that grows monotonically.

- **"Status vocabulary changes will break grep-based queries against old entries."**
  Mitigation: keep `fixed-shipped` as a deprecated alias (or skip option B entirely in favor of A). Option A introduces NO new vocabulary — just refines the SHA-citation format.

- **"Citation drift is unfixable as long as `experiments` is long-lived."**
  True. The proposal does not try to fix the rebase-induced drift directly; it makes the drift VISIBLE via the `experiments:<sha>` qualifier so readers know to expect it. The structural fix is in ship cadence (which T8 doesn't own).

- **"Manual quarterly archive passes will be forgotten."**
  True risk. Mitigation: pair with a scheduled job (`/loop` or `mcp__codescout__librarian audit_tracker_freshness`) that surfaces archive candidates each quarter. Skill-driven, not automated.

## Migration cost (rough)

For the existing 18 U-N entries:
- 13 closed → eligible immediately under any policy. 8 of those have orphan SHA citations to clean up (already done in T11).
- 4 entries have already been moved into citing master-side SHAs via T11 convention; they'd pass any policy filter unchanged once a master ship lands.
- New entries written under the policy are zero-cost.

One-shot archive pass on existing trackers: estimate ~30-60 min once policy is decided.

## Decision criteria (draft → active or wontfix)

Promote to **active** (and start implementation) when:
- You pick one option each for surfaces 1, 2, 3 above (status vocabulary, archive trigger, archive destination).
- A first quarterly archive pass is scheduled or executed (proving the policy is operable, not just paper).

Mark **wontfix** if neither happens within a month. The trackers can keep growing in the meantime — the cost is real but not blocking — and the policy isn't worth designing in a vacuum.

## Pointers

- **Origin:** 2026-05-24 task batch from the Yin/Yang critique of session intelligence trackers, surfaced after T7 (U-N triage) found 13/18 entries already closed and T11 (SHA reconciliation) demonstrated citation drift recursively.
- **Affected trackers:**
  - `docs/trackers/codescout-usage-frictions.md` (18 U-N entries, 528 lines)
  - `docs/trackers/codescout-usage-hookify.md` (6 H-N entries)
  - `docs/trackers/reconnaissance-patterns.md` (5 R-N entries)
  - `docs/trackers/tool-usage-patterns.md` (augmented artifact `b3fa993849ac83ab`)
  - Per-work-stream `<topic>-session-log.md` files (F-N / W-N) — already archived when their work stream wraps; policy may not need to extend here.
- **Related conventions:**
  - `CLAUDE.md § After cherry-pick: cite the master SHA` (extended in T10).
  - `docs/TAXONOMY.md` (T6 — taxonomy cheatsheet; would gain a "Status vocabulary" section once policy lands).
  - `docs/issues/_TEMPLATE.md` (bug-file archive flow at `docs/issues/archive/` — the existing precedent for archive-on-ship).
- **Cross-cutting:** the ship cadence question is out-of-scope here but obviously linked. Without a way to land experiments→master deliberately, the "shipped-to-master" state is rarely achievable, and the policy degenerates to "archive when shipped-to-experiments + old enough."
