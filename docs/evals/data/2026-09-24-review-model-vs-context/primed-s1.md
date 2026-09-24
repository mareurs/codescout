The trackers are updated and all my work is committed, including the temp results from the scratchpad. Nothing of mine is left uncommitted, and nothing is pushed.

**Temp work saved** (`f3321e40`): the scratchpad only exists for this session, so I copied every file behind a reported result into `docs/evals/data/2026-09-24-rule-tell/`. That's 76 files, 5.5 MB, unchanged, covering gate logs, row data, replays, forks, checker fixtures, JevK5's probability log, the phase-0 judge rows and the two Jev research reports. A README maps each file to the result it backs.

Before committing I scanned every file for API keys, tokens and your email addresses (no hits), and checked the README's file references won't fail CI. I left some files out on purpose, each with its reason in the README:
- **The recorded API requests:** they contain your private global `CLAUDE.md`.
- **Raw `claude -p` startup logs:** they carry account metadata.
- **The judge config folders:** they link to your credentials.

**Trackers:**
- **Workflow ledger:** added DCS-7, this interval's coverage receipt (`a8835d06`).
- **Codex's bug file:** it had never been committed. I confirmed the fix is `0fef5562` by re-running Codex's own reproduction: it now reports the sweep as incomplete and exits 2. I marked it fixed with the SHA and patch-id, and moved it from `cluster/unclassified` to IC-13 ("a capped result is presented as complete"), with the reason on that class's member list. It isn't archived, because that needs a regression test and this script has none.
- **Scoring doc:** the handoff header is current and points at the data folder. A-40 was updated in the previous commit.
- **codescout memory:** where the JevK5 environment lives, that `rtd8c` replaces `rtd8`, running scorers with `python -u`, how to rebuild the clean judge folder, and that the recorded requests were deliberately not committed.
- I deleted `phase2-stripped.log`, a stray log of mine at the repo root.

---

My recommendation for next is a **zero-call check of the 9 `partial` texts**: does each excerpt actually show its violation?

**Why this one first.** The injection side works. The clean re-score made RTD-8, RTD-9 and RTD-10 all ship: once the right rule and claim are named, the reminder stops or cuts the violation. The weak part is the selector, which picks the rule and the claim, and one open question decides how to read its results:

- S0 finds the violation on 3 of 17 violating texts. On 10 of the 17 it fires no rule at all.
- Most of that silence is the 9 texts the corpus labels `partial`, where S0 found 0 of 9 under both question forms.
- The scoring doc already says whether those excerpts show enough to judge "is a question about the corpus, not the selector". No experiment has answered it yet.

**Why before Stage 2.** Stage 2, the local-route data build (1.5–3 weeks), would label training data with the same excerpt convention. If `partial` excerpts can't be judged, those labels are unjudgeable too. The check takes hours and could change how weeks of labelling are done.

**Not now:** building the actual rule injector. The observation window defers implementation until 2 October, and its review is tomorrow (25 September), so "ships" means the evidence holds, not that we wire it in.
