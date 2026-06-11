# ADR-2026-06-11 — kotlin-lsp Upgrade Is Decoupled From the Sharing Fix

## Status

Proposed — design recorded on `experiments` 2026-06-11. Sibling of
[[2026-06-11-mux-single-owner-invariant]] (the sharing fix). This ADR exists
to prevent a future conflation: "just upgrade kotlin-lsp" is **not** the
multi-instance sharing solution.

## Context

- codescout launches the kotlin LSP as a bare `kotlin-lsp` binary
  (`src/lsp/servers/mod.rs:96`, via `platform::lsp_binary_name`). The
  `faketime 2026-06-04 … kotlin-lsp.sh` wrapper observed in `ps` is **not** in
  this repo — it lives in the user's externally-installed launcher script,
  papering over an **expired EAP build's** time-bomb.
- Upstream `Kotlin/kotlin-lsp` state as of 2026-06-11 (verified against
  `RELEASES.md`, verbatim):
  - **v262.7569.0** (released 2026-06-09, latest): workspace model persisted
    across restarts; license changed to the **JetBrains Free Plugin License**;
    published to the VS Code Marketplace and Homebrew
    (`brew install JetBrains/utils/kotlin-lsp`).
  - **v262.4739.0**: *"Index storage migrated to RocksDB — more robust state
    management and better performance"*; *"Kotlin LSP now requires JDK 25 to
    run."*
  - **v261.13587.0**: *"Indicies are now stored in a dedicated folder and are
    properly shared between multiple projects and LS instances"* — this is the
    multi-instance sharing feature, and it **predates** the RocksDB migration.
  - Current README / `RELEASES.md` document **no** build-expiry / EAP time-bomb;
    the post-EAP Free-Plugin-License + Marketplace distribution looks like the
    expiry mechanism is gone.

## Decision

**Upgrade the external kotlin-lsp launcher to v262.7569.0, drop the faketime
hack, and move to JDK 25 — as a separate work-stream, explicitly NOT the
multi-instance sharing fix.**

The sharing fix is [[2026-06-11-mux-single-owner-invariant]]. They are decoupled
because the RocksDB migration (v262.4739.0) means even the newest build takes an
**exclusive** per-directory index lock — the very contention the mux exists to
arbitrate. The v261 "shared between multiple LS instances" line describes a
pre-RocksDB build and does not transfer.

## Consequences

### Now easier

- Retires the faketime/expiry hack and its operational fragility (a pinned fake
  date that silently rots).
- Gains workspace-model persistence (faster cold start), the RocksDB perf work,
  and supported Marketplace/Homebrew distribution.

### Now harder

- Requires JDK 25 on every machine running the kotlin LSP.
- Touches the **external launcher**, which is outside this repo's scope — a
  different reviewer and a different change-control surface than codescout's
  Rust code.

### Revisit-when

- A **two-instance test on the upgraded build** shows concurrent index access
  actually works (RocksDB secondary / read-only multi-open). If it does, reopen
  [[2026-06-11-mux-single-owner-invariant]] ADR-1: the mux could demote from
  lock-arbiter to pure connection-pooling.

**Confidence:** high that the upgrade is worth doing (expiry, perf,
persistence); high that it is **not**, on its own, the sharing fix.

## Alternatives considered

1. **Fold the upgrade into the sharing fix — "upgrade and the lock problem goes
   away."** Rejected — conflates two scopes. The RocksDB migration means the
   newest build still locks exclusively; our directly-observed `rocks/v492/LOCK`
   contention is on a post-migration build. The sharing fix is needed
   regardless of the upgrade.

2. **Stay on the pinned expired build with faketime indefinitely.** Rejected —
   a faked system clock is a latent failure (anything date-sensitive in the JVM
   or its deps misbehaves), and it blocks the perf/persistence gains. The hack
   was a bridge, not a destination.

## Related

- [[2026-06-11-mux-single-owner-invariant]] — the actual multi-instance sharing
  fix; this ADR's revisit trigger feeds ADR-1.
- Upstream: `https://github.com/Kotlin/kotlin-lsp/blob/main/RELEASES.md`.
- `src/lsp/servers/mod.rs:96` — where codescout invokes `kotlin-lsp`.
