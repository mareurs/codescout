---
kind: tracker
status: archived
title: Codescout Usage Frictions — archive 2026 Q2
owners: []
tags:
  - pika
  - iron-law
  - usage
  - archived
---

# Codescout Usage Frictions — archive 2026 Q2

Archived U-N entries from `docs/trackers/codescout-usage-frictions.md`,
moved here per the archive cadence policy at
`docs/trackers/archive-cadence-policy.md`. Each entry's status had reached
a terminal state with no SHA-on-master dependency (by-design,
wontfix, or substrate-caught).

This file is `status: archived` — the librarian hides it from default
`artifact(action="find", kind="tracker")` queries; pass
`include_archived=true` to surface it.

Pilot archive pass: 2026-05-24. Three entries.

---

### U-4 — Iron Laws triplicated in context (canonical + companion + buddy)

**When:** 2026-05-23, user-requested prompt-surface self-reflection during a `/buddy:summon pika` session. Discovered by reading `src/prompts/source.md`, `claude-plugins/codescout-companion/hooks/session-start.sh` output, and `claude-plugins/buddy/data/gates.md` side-by-side.

**Iron Law / pattern:** surface design — single-source principle. The same five Iron Laws appear in three places in the loaded context:
1. `src/prompts/source.md::server_instructions` (canonical, build-sliced — 44 lines, terse 5-bullet table).
2. `claude-plugins/codescout-companion/hooks/session-start.sh` "CODESCOUT RULES (compression-resilient reminder)" (~10 lines, bulleted; *intentionally* designed to survive compaction).
3. `claude-plugins/buddy/data/gates.md` `## Tool gates — codescout Iron Laws` (~20 lines, prose narration).

**Tool called (surface):** all three surfaces re-state the same five rules.

**Should have called:** one canonical copy. The two derived surfaces should be *pointers* ("see Iron Laws in MCP server instructions") unless they add information canonical doesn't have. Whichever copy is most likely to survive `/compact` should be the only one — currently the weakest (compression-reminder) is most compaction-resilient because SessionStart rebroadcasts on resume, which inverts the design intent of "canonical is the source of truth."

**Whistle delivered:** yes (chat U-1 in this session; promoted to this tracker entry).

**Recurrence:** 1st observed and recorded.

**Severity:** low — current copies are *consistent in content*; the cost is token bloat (~30 redundant lines in every session prefix) plus drift risk for future edits. Drift already realized in U-5, U-6.

**Status:** **by-design, not drift (revised 2026-05-23).** The three copies serve three lifecycle stages:
1. Canonical `source.md::server_instructions` — primary at MCP session init; cut at ~2 KB by Claude Code's instructions channel.
2. Companion compression-reminder (SessionStart hook) — post-`/compact` safety net; refires on session resume.
3. Buddy `gates.md` — per-specialist defense-in-depth (U-11 reduced this from full-prose to a pointer + at-a-glance cheat sheet, which is what the layer actually needs).

The triplication is correctly layered; the failure mode I worried about (drift between copies) is now substrate-prevented by **H-3** (companion-surface lint, shipped 2026-05-23). The buddy copy was simplified by **U-11** (gates.md rewrite). The remaining cost is bloat, not contradiction, and the bloat is paid in exchange for compaction-survival.

**Archived:** 2026-05-24 (Q2 pilot pass). Category: by-design (no SHA dependency).

---

### U-9 — Caveman SessionStart payload injected twice

**When:** 2026-05-23, session start of this conversation.

**Iron Law / pattern:** hook coalescing / harness dedup.

**Tool called (surface):** caveman plugin's SessionStart payload appears as two consecutive `<system-reminder>` blocks at session start, content near-identical (level: full both times).

**Should have called:** one copy. Either the hook runs twice (likely two SessionStart hooks registered in different profile dirs — see U-10 cross-CC-profile config drift) or the harness fails to dedupe identical SessionStart payloads.

**Whistle delivered:** yes (chat U-6 → this tracker entry).

**Recurrence:** 1st observed this session; needs cross-session confirmation.

**Severity:** low — bloat only, no semantic harm.

**Status:** wontfix — user declined to pursue (2026-05-24). Out of scope for codescout repo; would need a bug filed against the caveman plugin or CC harness. Listed for awareness only; not blocking anything.

**Archived:** 2026-05-24 (Q2 pilot pass). Category: wontfix (out of scope for this repo).

---

### U-16 — Pika invoker hit IL3 on first survey move (substrate caught)

**When:** 2026-05-23, post-/compact reload of the codescout-pika specialist. First exploratory git log of the new session was piped: `git log --oneline master..experiments | head -20`. The `pre-tool-guard.sh` (or its codescout-server counterpart) blocked it with the standard IL3 message; required a re-run as `git log --oneline master..experiments` (bare) followed by `grep`/`tail` on the returned `@cmd_*` buffer.

**Iron Law / pattern:** Iron Law 3 — `run_command` output piped to a log-trimmer (`| head -20`). `git log` is an unbounded-LHS command.

**Tool called (surface):** `run_command(command="git log --oneline master..experiments | head -20")` — invoked from the main Claude Code agent operating as Pika.

**Should have called:**
1. `run_command("git log --oneline master..experiments")` — bare, full output stored in `@cmd_*`.
2. `grep PATTERN @cmd_*` or `read_file(@cmd_*, start_line=..., end_line=...)` for trimming.

**Whistle delivered:** yes — self-whistle, recorded inline ("→ pika whistles: own IL3 slip on first move"). Self-correction acknowledged in the same turn.

**Recurrence:** 3rd observed IL3 slip in this tracker (after U-1, U-3). Notable that the slip came from the Pika operator itself — the agent watching for IL3 violations was the one that committed the violation. **Update: 2nd self-slip same session** — operator subsequently piped `cargo build --release 2>&1 | tail -5` later in the same exploration loop. Substrate held both times. Pattern: even after the first whistle, the watcher slipped again ≤30 turns later on a structurally identical shape (unbounded-LHS command + log-trimmer pipe). Reinforces severity assessment: discipline alone is insufficient.

**Severity:** low — substrate held. The gate caught the slip; cost was one wasted tool call + a "BLOCKED" reply turn. No code change reached disk.

**Status:** substrate-caught — no code fix required. The IL3 gate (companion `pre-tool-guard.sh` + codescout server-side check) worked as designed.

**Lesson / counterfactual (W-N material):** the IL3 substrate is now demonstrably robust against the actor that should know it best. This is evidence that gate-by-substrate beats gate-by-discipline — the rule "always whistle on IL3 violations" doesn't prevent the watcher from violating IL3; the substrate does. Reinforces the rationale for H-3 (companion-surface lint) and the worktree-write-guard test pattern: durable behavior comes from gates, not prompts.

**Note on observability:** the slip would have been invisible to the user without the BLOCKED error message in the tool result. There is no separate "Iron Law violation log" surface — the gate's deny path doubles as the observability mechanism. If the deny message were silent (e.g. swallowed by an auto-retry), this U-16 entry would not exist.

**Archived:** 2026-05-24 (Q2 pilot pass). Category: substrate-caught (no code fix required, U-18 carries the unresolved reflex thread forward).
