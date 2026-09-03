---
status: template
opened: YYYY-MM-DD
closed:
severity: medium
owner: marius
related: []
tags: []
kind: bug
---

<!--
BUG TRACKER TEMPLATE — do not edit content; copy this file.

To open a bug:
  1. Copy this file to docs/issues/$(date -I)-<slug>.md
  2. Replace this comment block with the bug content.
  3. Done — the librarian discovers the file on next reindex via its
     `kind: bug` frontmatter. List active bugs with:
       doc(action="find", kind="bug",
           filter={"status": {"in": ["open", "taken", "investigating", "zombie"]}})
     status="open" alone hides any bug marked `taken` (a live session holds
     it), `investigating` (worked, no live owner) or `zombie`
     (recurring-but-unconfirmed -- a "has this come back?"
     check, not a task to pick up). No manual index file. (Pre-2026-05-18 there was a docs/issues/INDEX.md
     to maintain by hand; that workflow was retired when bug files gained
     `kind: bug` frontmatter and the librarian classifier started picking
     them up automatically — see CLAUDE.md "Querying active trackers".)

Trigger rules — open a tracker for ANY bug noticed during work:
  ✓ User explicitly asks ("log this", "open a tracker")
  ✓ Bug blocking the current task (fix-now or parking-lot)
  ✓ Incidental bug we won't fix in the current session
  ✓ Just-fixed bug whose investigation is worth preserving
  ✓ Tool quirks / misbehaviors (formerly the BUG-XXX log)
  ✗ Pure typos / one-token corrections — commit message is enough
  ✗ Feature ideas / refactors — those go in docs/trackers/ or docs/plans/
  ✗ Subjective dislikes that aren't bugs

Status field semantics:
  open          — Logged, investigation not started or paused.
  taken         — A live session holds this right now. Requires
                  claimed_by: <sessionId> in frontmatter — set it through
                  artifact(action="update", id=..., extra={"claimed_by":
                  "<sessionId>"}), never by hand-editing the frontmatter:
                  a raw file edit does not reach the catalog (BL-48), so
                  the claim sits on disk while every find() reports the
                  bug unclaimed. Decays to `investigating` when that
                  session exits; run librarian(action="doctor") to find
                  dead claims.
  investigating — Worked, but no live owner. The residue of an
                  unconcluded claim, not a synonym for `taken`.
  fixed         — Root cause addressed, regression test added, verified.
  mitigated     — Workaround in place; root cause not addressed.
  wontfix       — Intentionally not fixing; justification in the file.
  zombie        — No longer observed but root cause not confirmed; kept
                  open in case it recurs. Pair with `last_observed:` in
                  frontmatter and a "Status: zombie" section documenting
                  the re-open trigger.
  `closed:` stays empty at creation — fill in YYYY-MM-DD only when
  status flips to fixed/mitigated/wontfix.

Optional `unverified:` — the caveat, made queryable:
  Add `unverified: '<what is NOT established>'` to frontmatter whenever the
  record's status overstates what was actually verified: no regression test,
  root cause not addressed, a claim not re-checked since a rebuild, a fix
  applied only to a gitignored file, or a blocker that turned out to be an
  obsolete rule. ABSENCE means nothing outstanding — do not add it empty,
  because presence is the signal a query filters on.
  Why it exists: measured 2026-08-19, 14 of 16 terminal-but-unarchived bug
  files stated their blocker in prose, where no query reads. That is how
  find(kind="bug", status="open") came to miss a `fixed` record whose own
  body said "Tests added: None" and "does not prevent recurrence".

Archive trigger: move the file into docs/issues/archive/ once the fix is
verified on experiments — gate green plus a regression test. Reaching
master is NOT required; experiments is never deleted.
When archiving an experiments-only fix the file must carry the fix SHA
labelled experiments.

Record its patch-id alongside the SHA:
  git show <fix-sha> | git patch-id --stable
The SHA is positional and dies when experiments is rebased (which happens
after every ship). The patch-id is a content hash of the diff and survives
both rebase and cherry-pick, so it still finds the change afterwards.
There is NO promotion path to check, NO pending-master-SHA Resume line, and
nothing to come back and reconcile. Measured 2026-08-19: 10 of 63 archived
bug files had already lost their SHA; zero patch-id collisions in 3594
commits.
Check where a SHA lives with:
  git branch --contains <fix-sha>
Archive via doc(action="move", id=..., new_rel_path="docs/issues/archive/...")
— never a bare git mv, which orphans the catalog row.

Use `N/A` or `Unknown — under investigation` for sections that don't
yet apply. `N/A` in `Tests added` requires justification — empty Tests
added without justification means the bug isn't really closed.
-->

# BUG: <one-line summary>

## Summary
*1–3 sentences. What's broken, who's affected, the elevator pitch.*

## Symptom (Effect)
*Capture the EXACT observable behavior. Verbatim error string in a code
fence (no paraphrasing). Exit code if any. Timing if relevant. What was
observed, not what it means.*

## Reproduction
*Minimal copy-pasteable steps. Include git commit (`git rev-parse HEAD`)
and how to invoke (`cargo run --release` / `/mcp` / etc). If not yet
reproducible, write `Not yet reproducible — best lead: …` and stop.*

## Environment
*OS, language/runtime versions, MCP transport, project, branch. Anything
that moves the reproducibility line.*

## Root cause
*Mechanism, in mechanism-language ("X holds a lock while Y waits on it"),
not symptom-language. Cite `path:line` for every claim. If unknown, write
`Unknown — see Hypotheses tried` and link.*

*Also cite what **measured** it — the command and the date, on one line:
`measured 2026-08-07: ps -o ppid= <pid> → every parent alive`. A mechanism read
out of the code but never observed at runtime says so instead: `inferred from
src/x.rs:12 — not measured`. This is the premise a later session re-checks
BEFORE working the bug, and an unmeasured mechanism is a hypothesis wearing a
conclusion's clothes: of the five bugs worked on 2026-08-07, all five had a
false premise or a wrong prescription, and four fell to a single command (W-13,
`docs/trackers/release-promotion-session-log.md`).*
## Evidence
*One subsection per piece of evidence. Include the source of the evidence
(`.codescout/diagnostic-XXXX.log`, session JSONL path, command output).
Quote rather than summarize — copy the relevant lines into a code fence.*

## Hypotheses tried
*Numbered list. Each entry: **Hypothesis** / **Test** (what we did to check) /
**Verdict** (confirmed | rejected | deferred) / **Evidence link** (anchor
to the Evidence subsection). Append; never delete rejected ones — they
are how future-me avoids re-walking dead ends.*

## Fix

*Plan first, implementation second. Include where the actual change lives (e.g.
`src/server.rs:202-358`). If "Fix" is just a workaround, say so explicitly and keep status
`mitigated`, not `fixed`.*

*Record **two** identifiers for the fix commit, because they fail differently:*

- ***SHA** — positional. It names a commit's place in a branch's history, and dies if
  `experiments` is rebased. Label which branch it is on.*
- ***patch-id** — `git show <sha> | git patch-id --stable`, a content hash of the diff.
  It survives rebase **and** cherry-pick. Measured 2026-08-19 across 3594 commits: zero
  genuine collisions, and all 104 duplicate patch-ids were the same change on two
  branches. 10 archived bug files had already lost their fix pointer to a rebase.*

*There is **no promotion path to check**, no pending-master-SHA line to write, and nothing
to come back and reconcile. Record both once and the record stays resolvable whichever way
the fix reaches `master`.*
## Tests added
*Regression test name + `path:line`. If the test is intentionally absent,
say why (timing-dependent, env-specific, manual-only). Empty `Tests added`
without justification means the bug isn't really closed.*

## Workarounds
*What users can do RIGHT NOW to unblock themselves while a fix lands.*

## Resume

*Concrete next action, not a goal. Bad: "investigate the LSP path". Good:
"diff src/lsp/client.rs between commits X and Y; check if `did_change` is
sent before `hover` query. Run `cargo test did_change_refreshes` to anchor
behavior." Wipe and replace each session. `N/A` once fixed.*

*Cite paths with prefix (`src/lsp/client.rs`, not bare `client.rs`). The
audit_doc_refs lint resolves bare basenames via fallback (severity Low) but
the prefixed form is unambiguous and survives renames cleanly. If your fix
moves a file, update the Resume sections of any open bugs that cite the
old path.*
## References
*Files, dashboards, related issues, external links, session log paths.*
