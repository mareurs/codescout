## Update 2026-07-08 02:45 — live capture of a real spike, GC sawtooth confirms Fix 1 containment

At 02:45:35, a second concurrent kotlin-lsp spawned (`.worktrees/single-stage`, pid 195014)
while another was already running for the main cwd (pid 4063541, `CONCURRENT instances=2`).
The new instance's RSS: 2184MB (t+11s) -> 2312MB (t+~20s) -> 1818MB (t+~30s, GC reclaim).

This matches the exact heap-driven GC-sawtooth signature documented in
`docs/issues/2026-06-19-kotlin-lsp-uncapped-jvm-heap.md`'s "Live growth curve" (rapid rise
to a ceiling near the `-Xmx` bound, then partial GC reclaim) — just now bounded at ~2.3GB
instead of the pre-fix 35.7GB, because the `-Xmx2g` cap (Fix 1) is doing its job. Available
host memory stayed healthy throughout (46-50GB free) — not a host-threatening event.

**Working theory for "kotlin-lsp went rogue":** the user is very likely observing this fast
(<20s) near-cap RSS ramp during kotlin-lsp startup/initial indexing — especially pronounced
when a second concurrent instance spins up against the same underlying Kotlin sources
(two worktrees of backend-kotlin). It LOOKS alarming (near-zero to ~2GB in under 20s,
visible as a spike in any system monitor) but is the capped, self-correcting behavior the
Fix-1 heap cap was designed to produce — a vast improvement over the pre-fix unbounded
default (~31GB ceiling). Not yet confirmed whether it ever breaks past the heap-only
ceiling into uncapped native growth (the still-open Fix 2 gap) — keep watching for a
breakout past ~2.3-2.5GB that doesn't reclaim via GC.