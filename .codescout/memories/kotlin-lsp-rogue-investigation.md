## Resolved 2026-07-08 — doc updated, 3 live captures, 2 manual kills

Wrote up the full live-capture session into `docs/issues/2026-06-19-kotlin-lsp-uncapped-jvm-heap.md`
(new "### Live capture 2026-07-08" evidence subsection + updated "## Resume").
Key corrected finding now documented there: the doc's prior assumption that
`-Xmx2g` "collapses the heap component... leaving only the small native residual"
is WRONG — native memory alone reached 35.6GB and 37.4GB on 2 of 3 captures,
matching/exceeding the pre-fix ceiling. Fix 2 (kill LSP process group at a
memory threshold) is now flagged as the priority, not a residual cleanup.

Two of three organic kotlin-lsp spawns against codescout's own Kotlin fixture
were killed manually this session (`kill -TERM <mux_pid> <jvm_pid>`) after
crossing avail<15GiB — the doc now records this as the suggested Fix-2 threshold.
The real `backend-kotlin` project's own kotlin-lsp instances stayed healthy
(<500MB) throughout, confirming the "went rogue" report traced to codescout's
own small Kotlin fixture, not backend-kotlin itself, despite backend-kotlin
being the trigger for starting this investigation.

Live monitor (Monitor task in that session, exact-name `pgrep -x kotlin-lsp`
polling) is the reusable pattern for next time — avoid `pgrep -f` (self-matches
the monitor's own script text) and avoid grepping logs for `"oom"` case-insensitive
in this codebase (matches `Room`/`Classroom` domain vocabulary).