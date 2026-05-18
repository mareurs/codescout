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
       artifact(action="find", kind="bug", status="open")
     No manual index file. (Pre-2026-05-18 there was a docs/issues/INDEX.md
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
  investigating — Actively being worked on this session.
  fixed         — Root cause addressed, regression test added, verified.
  mitigated     — Workaround in place; root cause not addressed.
  wontfix       — Intentionally not fixing; justification in the file.
  zombie        — No longer observed but root cause not confirmed; kept
                  open in case it recurs. Pair with `last_observed:` in
                  frontmatter and a "Status: zombie" section documenting
                  the re-open trigger.
  `closed:` stays empty at creation — fill in YYYY-MM-DD only when
  status flips to fixed/mitigated/wontfix.

Archive trigger: move the file into docs/issues/archive/ AFTER the fix
ships to master, not when status flips to fixed. Detect with:
  git branch --contains <fix-sha>
If `master` is in the output, the fix is on master.

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
*Plan first, implementation second. When implemented, list commit SHAs and
where the actual change lives (e.g. `src/server.rs:202-358`). If "Fix" is
just a workaround, say so explicitly and keep status `mitigated`, not `fixed`.*

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
