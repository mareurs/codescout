---
kind: tracker
status: active
title: Session Log — Embedder Stack Ops
owners: ["marius"]
tags: ["embeddings", "retrieval", "docker", "gpu"]
topic: embedder stack ops
entry_prefix: ["F", "W"]
entry_high_water_F: 2
entry_high_water_W: 1
---

# Session Log — Embedder Stack Ops

> **Purpose:** Two-sided observation log for a multi-session work stream.
> Captures frictions (F-N) and wins (W-N) that the session producing it
> wants to preserve so future sessions inherit the lesson.
>
> **How to use:** Copy this file to `docs/trackers/<topic>-session-log.md`
> in the active project on first reconnaissance pass. Append F-N / W-N
> entries with:
>
> ```
> artifact(action="append_entry", id="<artifact id>", id_prefix="F",
>          anchor_heading="## Template for new entries",
>          title="<one-line title>", body="**Observed:** ...")
> ```
>
> One call, one write: the server allocates the next id, formats the
> heading as `## F-N — <title>` (the only shape `link_scan` accepts as a
> definition), records the ledger's high-water mark, and stamps
> `**Valid:** dated <today>` unless your body declares a class. **Then**
> add the Index / Wins Index row, using the id the call returned — the
> indexes are the eval surface, the sections are the evidence.
>
> **Do not hand-allocate ids, and do not pre-write index rows.** A max-id
> is a fact about an instant, and a peer session in the same checkout can
> take the number between your scan and your write. Pre-written rows are
> worse: the allocator counts an id claimed by an index row, so rows
> written ahead of their sections consume the ids they name — which is why
> codescout's `statement-validity-session-log` starts at `statement-validity-session-log:F-2`/`statement-validity-session-log:W-3`
> rather than `statement-validity-session-log:F-1`/`statement-validity-session-log:W-1` (see `statement-validity-session-log:F-3` there).
>
> **`edit_markdown` is not the append path**, though it works at first.
> This template ships without frontmatter, so a fresh copy is directly
> editable — but once you declare `entry_prefix` to make the ledger
> guarded (which `get_guide("tracker-conventions")` tells you to do), the
> librarian guard refuses direct edits and only `append_entry` writes.
> Reach for `edit_markdown` for the prose sections and the index tables,
> never for allocating an entry.
>
> **Lifecycle:**
> - Created at the start of a multi-session work stream.
> - Appended-to across every session that touches the work.
> - Entries with `Status: open` carry forward across sessions.
> - Promotion to permanent surfaces (CLAUDE.md, ADRs, formal bug
>   trackers) happens when the entry's `Promote-when` / `Fix idea`
>   criteria fire.
> - File archived (moved to `docs/trackers/archive/`) when the work
>   stream wraps.

---

## Index

| ID | Date | Severity | Category | Status | Title |
|----|------|---------:|----------|--------|-------|
| F-1 | 2026-08-29 | high | infra-diagnosis | fixed-verified | Asserted VRAM-contention root cause before measuring; real cause was a host reboot |
| F-2 | 2026-08-29 | high | monitoring | fixed-verified | Docker reported the whole GPU stack healthy for 15 hours while every inference request hung |

## Wins Index

| ID | Date | Impact | Pattern | Counterfactual | Status |
|----|------|-------:|---------|----------------|--------|
| W-1 | 2026-08-29 | high | test-via-real-invocation-path | .env's stale CODESCOUT_MODEL_DIR would have stayed masked indefinitely, first breaking on the unit's own first real boot | validated |

---

## Promotion status

**Audited:** <YYYY-MM-DD>, against the target surface itself — opened and read,
not recalled.

One line per `W-N` (and any `F-N` with a `Fix idea` bound for a permanent
surface). Check the **target**, not the entry: a `Promote-when` that fired is
invisible from inside the tracker, because `Status: validated` reads as healthy
either way. Record one of:

- **already promoted, no action** — quote the promoted text verbatim and name
  where it landed, so the next reader verifies instead of re-deriving.
- **UNFIRED, carried forward** — restate the criterion and the current datapoint
  count.
- **FIRED but not yet applied** — the one that leaks. Name the exact target
  surface and the exact text to add. This is an action item, not a note; set the
  entry's `Status:` to `promotion-due` so a query can find it.

> ⚠️ **Name every instance of the target, not the target's type.** This machine
> runs three Claude Code profiles (`~/.claude`, `~/.claude-sdd`,
> `~/.claude-kat`), each with its own `CLAUDE.md`. An audit that concluded
> *"not found in the user's global CLAUDE.md"* — singular — led to a promotion
> that reached one file of three on 2026-08-18. The session that found the gap
> was running on a profile **without** the rule, and applied it only because
> another profile's copy happened to be injected as project instructions. Three
> files that should be byte-identical have an md5; compare them.

> ⚠️ **For an INSTALLED artifact the target is the SERVING copy — not the repo
> source, and not the other copies.** Measured 2026-08-20: three rules promoted
> into a plugin skill were byte-identical across all three profile caches *and*
> stale against source, because the commit never bumped the version the cache is
> keyed on. Comparing the copies to each other reads **green** there — only
> comparing each copy to the claim catches it. And the session that made the edit
> is the **least representative observer**: its own reload resolved the skill from
> the repo source, so the confirming evidence sitting in front of it was evidence
> about the wrong artifact.

> ⚠️ **Anchor on a back-citation, not a verbatim quote.** A quote goes red when the
> promoted rule is legitimately reworded — a false positive produced by the
> promotion working as intended, observed 2026-08-20 when `codescout:R-89`'s bullet was
> rewritten and the tracker's stored quote had to be edited to match. The durable
> form is the promoted text citing its own entry id —
> *"(codescout:R-1 + codescout:R-7 in codescout's `docs/trackers/reconnaissance-patterns.md`.)"* — so
> verification is a `grep` for the id and survives every rewording. Keep the quote
> as a reading aid; do not make it the predicate.

Run this when the work stream wraps, **and** whenever a criterion fires
mid-stream — an audit that only happens at archive time is one that happens
after the lesson was needed. Prior art:
`eduplanner-ui:docs/trackers/archive/calendar-insight-panel-session-log-2026-08-18.md`, whose
audit correctly caught its own `calendar-insight-panel-session-log-2026-08-18:W-4` as fired-and-unapplied and named the exact
text to promote.

## Category conventions

Use a short kebab-case category to group similar frictions. Prior
sessions have used:

| Category | When to use |
|---|---|
| `codescout-tool` | Friction in a codescout MCP tool (`grep`, `read_file`, `edit_markdown`, etc.) |
| `subagent` | Subagent produced unexpected output or diverged from instructions |
| `plan-prose` | Plan document had drift vs reality (wrong file paths, fictional code, mismatched counts) |
| `architectural` | Discovered structural property of the system that the plan / docs didn't surface |
| `self-friction` | Predicted a friction that turned out to be a false alarm — recorded for transparency |
| `<language>-<library>` | Language- / library-specific footgun (`rust-serde`, `python-typing`) |
| `release-pipeline` | Deployment-time gap (release binary missing, MCP reload needed, etc.) |

Add a new category by writing it as a kebab-case string; no central registry needed.

---

## F-N entry template

Pass this block as `append_entry`'s `body` (without the `## F-N — <title>`
line — the server writes the heading from `title`). Add the matching Index
row afterwards, using the id the call returned. Do not allocate the id
yourself; see *How to use* above.

```markdown
## F-N — <one-line title>

**Observed:** <date, session task>

**When:** <what you were trying to do>

**Expected:** <what plan / docs / prior session said>

**Got:** <actual observed reality>

**Probable cause:** <one sentence>

**Workaround:** <what you did to proceed>

**Severity:** low | med | high

**Status:** open | wontfix-false-alarm | fixed-verified | mitigated | promoted-to-bug-tracker | pinned-as-eval-baseline

**Valid:** invariant | dated YYYY-MM-DD | conditional — <the event that ends it>

**Rests on:** <one durable sentence — an ADR, a decision, or the principle this
instantiates>

**Fix idea / Pointer:** <issue # in formal tracker, plan task ID, or "TBD">

---
```

## W-N entry template

Pass this block as `append_entry`'s `body`, with `id_prefix="W"` — F-N and
W-N have separate counters. A win without a **Counterfactual** is marketing
— name what would have happened without the pattern, with at least one
piece of evidence.

```markdown
## W-N — <one-line title>

**Observed:** <date, session task>

**Pattern:** <the practice that worked>

**Counterfactual:** <what would have happened without the pattern, with evidence>

**Confirming data points:** <list of session moments validating the pattern; aim for ≥2>

**Impact:** low | med | high

**Promote-when:** <criterion for graduating into permanent docs (CLAUDE.md, ADR, etc.)>

**Promoted-to:** <surface + section, one per line, line-start — omit until it lands>

**Status:** validated | promotion-due | promoted-to-permanent-docs | archived

**Valid:** invariant | dated YYYY-MM-DD | conditional — <the event that ends it>

**Rests on:** <one durable sentence — an ADR, a decision, or the principle this
instantiates>

---
```

---

## Status vocabulary

Codified so the Index column means the same thing across sessions.

### Friction statuses

| Status | Meaning |
|---|---|
| `open` | Observed, not yet resolved. Default for new entries. |
| `wontfix-false-alarm` | Initial observation was wrong; documented for transparency rather than deleted. |
| `mitigated` | Workaround in place; root cause not fully resolved. |
| `fixed-verified` | Code / process fix landed AND empirically confirmed. (`fixed` alone is too weak — verification is part of the status.) |
| `promoted-to-bug-tracker` | Moved to a formal tracker (`docs/issues/*`, `docs/TODO-*`, GitHub issue). The session log keeps the pointer; the formal tracker owns the lifecycle. |
| `pinned-as-eval-baseline` | Kept verbatim as a reference point for measuring later improvements. Do NOT close — its job is to remain comparable. |

### Win statuses

| Status | Meaning |
|---|---|
| `validated` | Pattern confirmed by ≥1 counterfactual data point. Default for entries with evidence. |
| `promotion-due` | `Promote-when` has **fired** and the text is not yet on the target surface. An action item, not a resting state. Exists because `validated` cannot distinguish "criterion not yet met" from "criterion met, nobody harvested it" — and both read as healthy, which is how a lesson sits unpromoted while the failure it describes recurs. |
| `promoted-to-permanent-docs` | Moved into CLAUDE.md, an ADR, a skill, or another permanent surface. Session log keeps the pointer — and, for a multi-instance target, names every instance it landed in. |
| `archived` | Pattern no longer load-bearing — either the underlying system changed or the discipline became automatic. |

---

## F-1 — Asserted VRAM-contention root cause before measuring; real cause was a host reboot

**Observed:** 2026-08-29, diagnosing why `semantic_search` failed with "dense embedder unreachable" on `127.0.0.1:48081` despite the stored Qdrant index being fully intact (`index(action="verify")` → complete, 1704/1704 files, 0 missing).

**When:** After finding `docker compose --profile gpu ps -a` showed `codescout-dense-gpu` and `codescout-sparse-gpu` both `Exited (255)` "7 hours ago" while `qdrant` and `reranker-gpu` stayed healthy, and after the user volunteered "this is the laptop not the desktop so we don't have so many resources."

**Expected (my claim):** I asserted, without measuring, that the crash was GPU-VRAM contention — three concurrent CUDA services (`dense-gpu`, `sparse-gpu`, `reranker-gpu`) overcommitting a small laptop GPU's VRAM budget — and wrote a whole "★ Insight" block presenting this as the explanation.

**Got (scouted reality):** Every measurement contradicted it. `nvidia-smi` showed a GTX 1660 Ti with 6GB total VRAM and 5.4GB free even *before* restarting anything. The actual model files are tiny (`CodeRankEmbed-Q4_K_M.gguf` = 90MB, `bge-reranker-v2-m3-Q4_K_M.gguf` = 438MB) — nowhere near a 6GB ceiling. Docker's own `.State.OOMKilled` on the still-crashed `sparse-gpu` container read `false`. No kernel OOM-killer line anywhere in `dmesg`/`journalctl -k` for the whole day. The real cause, found only after the user pushed back and asked me to measure: `journalctl --list-boots` showed the machine had rebooted multiple times that day, and `sparse-gpu`'s own `FinishedAt` (`2026-08-29T08:52:32Z`) landed within 10 seconds of a boot boundary (`08:52:22Z`) in the boot list. Both GPU containers' logs showed no shutdown/crash message at all — consistent with the whole machine going down mid-flight, not a targeted resource kill. Post-reboot, `qdrant`/`reranker-gpu` came back automatically but `dense-gpu`/`sparse-gpu` did not (unconfirmed why — never resolved, treated as out of scope for this session).

**Probable cause:** Host RAM alone (`free -h`: 1.6GB free, 5.4GB swap in use) looked like plausible corroborating evidence for a memory-pressure story, and the coincidence of "small GPU" + "laptop" + "two of four GPU containers down" was enough to write a confident narrative without running `nvidia-smi`, checking model file sizes, or checking `OOMKilled`/`journalctl --list-boots` first. This is exactly the failure CLAUDE.md's "Do not hypothesise but ALWAYS VERIFY" line exists to block, and it happened in a session working inside a project whose own memory (`gotchas`) already documents a near-identical class of mistake (trusting a plausible-sounding config/profile explanation over a runtime probe).

**Workaround:** User explicitly said "I don't think so, memory is quite low, we use small models. check, start gpu stack or measure first if you can" — the only reason the correction happened was direct pushback, not self-catch.

**Severity:** high — the wrong root cause was stated as fact, with a fabricated-sounding "Insight" box giving it false authority, in a codebase whose own operator will act on infra claims like this. Had the user not pushed back, "run bigger/fewer GPU services on the laptop" would have been the takeaway, when the actual fix (`docker compose up -d dense-gpu`, and separately, understanding why it doesn't survive a reboot) has nothing to do with sizing.

**Status:** fixed-verified — corrected in the same session once measurements were run; `dense-gpu` confirmed healthy and `semantic_search` confirmed working end-to-end afterward.

**Valid:** dated 2026-08-29

True of this machine's boot history and container state that day; the underlying "why don't dense-gpu/sparse-gpu restart automatically" question was left open.

**Rests on:** direct measurement — `nvidia-smi`, model file sizes via `ls -la models/`, `docker inspect --format '{{.State.OOMKilled}}'`, `journalctl --list-boots`, and container `FinishedAt` timestamps — not on the plausibility of the VRAM-contention story.

**Fix idea / Pointer:** Before asserting an infra root cause involving resource limits (memory, VRAM, disk), the ordering should be: measure first (`nvidia-smi`/`free -h`/`docker inspect OOMKilled`/`journalctl --list-boots`), THEN hypothesize — never the reverse, even when a plausible narrative is sitting right there (laptop vs desktop, small VRAM budget). Candidate for promotion to the reconnaissance skill's "claim about current state" bullet if this pattern recurs in a second session.

## F-2 — Docker reported the whole GPU stack healthy for 15 hours while every inference request hung

**Observed:** 2026-08-29 morning, earlier in the day than `F-1` and a **different
incident with the same symptom surface**. Read both before diagnosing a third.

**When:** `cargo test` ran past 900s with no test binary completing. Exactly 16
slow tests, all in `tools::memory::tests`, nothing else affected.

**Expected:** a dead or overloaded embedder, i.e. the shape `F-1` describes —
containers `Exited`, fixed by `docker compose up -d`.

**Got (measured):** the opposite. Every container read `Up 4 days (healthy)`.

- `curl -m 75` against `127.0.0.1:48081/v1/embeddings` → `HTTP 000`, zero bytes,
  full 75s elapsed. It accepted the TCP connection and never answered.
- `/health` on the same server → `HTTP 200 in 0.6ms`.
- `nvidia-smi` → 745 MiB / 6144 MiB used, 0% utilisation. Not memory pressure.
- `ps` → `1602 Zsl llama-server <defunct>` — the dense server was a **zombie**,
  ppid 1459 (its containerd shim), still holding 394 MiB of VRAM that the kernel
  could not reclaim.
- `docker restart` → *"container … PID 1602 is zombie and can not be killed."*
  `docker stop` on the sparse container → *"tried to kill container, but did not
  receive an exit event."* Both unkillable.
- `dmesg` → `NVRM: memmgrRestorePowerMgmtState_KERNEL: !!!!! Calling Resume on an
  active GPU or the previous Suspend call might have failed !!!!!` and
  `NV_ERR_INVALID_STATE from gpuStateLoad(...) @ gpu_suspend.c:281`, timestamped
  **2026-08-28 19:10:49**, preceded by three `NV_ERR_NO_MEMORY` faults and an
  assertion in `fbsr_gm107.c` (framebuffer save/restore).

The host suspended, the GPU's framebuffer save failed, and the driver refused to
restore state on resume. New CUDA allocations then hang inside the driver, which
is why `llama.cpp` stalls at `common_params_fit_impl: getting device memory data`
and why the processes become unkillable rather than crashing.

**The finding worth keeping is not the driver bug — it is that nothing said so
for 15 hours.** Docker's healthcheck hits `/health`, which returns a static
string and never touches CUDA, so it stayed green from 19:10 on 2026-08-28 until
10:24 the next morning. The reranker was checked only as a control and was
**also** dead (`HTTP 000` in 15s) while equally green. This is a self-validating
gate in the classic shape: it cannot fail in the broken world, and it does not
merely fail to fire — it actively reassures.

**Probable cause of the blind spot:** `/health` was chosen because it is cheap and
always available, which is exactly what makes it uninformative. A healthcheck that
would have caught this has to traverse the failing subsystem.

**Workaround:** none available without a reboot — the zombie's VRAM was
unreclaimable, so a module reload would also have failed with the device busy.
Stood up a CPU-only container on a free port (`--init`, `-ngl 0`,
`CUDA_VISIBLE_DEVICES=` empty) serving the *same* `CodeRankEmbed-Q4_K_M.gguf`, so
dim stayed 768 and the Qdrant index stayed valid. ~0.2s for a 16-input batch vs
~0.07s on GPU. Retired 2026-08-29 18:16 when the host rebooted; config restored to
`:48081` and the container removed.

**Severity:** high — every embedding-backed surface was silently dead for 15
hours, and the monitoring said otherwise.

**Status:** fixed-verified — resolved by the reboot; `:48081` re-measured healthy
at 0.07s for 16 inputs, dim 768.

**Valid:** dated 2026-08-29

True of that incident. The driver fault is cleared; the healthcheck blind spot is
**not** — nothing in `docker-compose.yml` changed.

**Rests on:** the `dmesg` NVRM lines and the zombie's `ps` state, not on the
container status, which was wrong throughout.

**Fix idea / Pointer:** ~~Two unclaimed, both one-line, both recorded in `ET-8`
Phase E~~ — **both DONE**, verified 2026-08-30 by re-reading `docker-compose.yml`
directly (not from this entry's own claim): `init: true` is present on
`dense-gpu`, `sparse-gpu` and `reranker-gpu`, each commented with a direct
reference to this entry; `dense-gpu`'s and `reranker-gpu`'s healthchecks now
POST to `/v1/embeddings` / `/v1/rerank` respectively (a 29x-slower real forward
pass vs `/health`'s 0.8ms latch-read, measured and commented inline). Landed in
`9360be99` ("fix(compose): healthcheck the inference path, not /health (T13,
T14)"), patch-id `47ca28a05d9e5b5fa962b4ba43b9b16d68b52a9d`
(`git show 9360be99 | git patch-id --stable`). A live `curl -X POST
.../v1/embeddings` re-check the same day returned a real 768-dim vector in
26ms — the fix is not just present in the file, it is currently doing its job.

1. ~~`init: true` on the llama.cpp compose services.~~ Done.
2. ~~Point the healthchecks at `/v1/embeddings`...~~ Done.

**Distinguishing this from `F-1`:** same surface, opposite state and opposite
remedy. `F-1` is containers **`Exited`** after a host reboot, fixed by
`docker compose up -d`. This is containers **`Up (healthy)`** and unkillable,
fixable only by a reboot. If a future session sees `Exited`, `F-1` applies. If it
sees `healthy` and requests hang, this does — and `docker ps` will be actively
misleading, so go to `curl` and `dmesg` first.

## W-1 — Boot-time systemd unit's clean environment surfaced a masked .env config bug

**Observed:** 2026-08-29, immediately after adding `~/.config/systemd/user/codescout-retrieval-stack.service` (to auto-start the GPU embedder profile on login/boot) and testing it live via `systemctl --user start`.

**Pattern:** Bring up infrastructure via the exact mechanism that will run it in production (a clean systemd unit environment), not by re-running the same manual shell command that has been "working" all session — an interactive shell's accumulated ambient environment variables can silently paper over a real config bug that a clean environment immediately exposes.

**Counterfactual:** Without testing through the actual unit (vs. just trusting my earlier manual `docker compose --profile gpu up -d dense-gpu` success), the project-root `.env`'s stale `CODESCOUT_MODEL_DIR=/home/marius/models` would have stayed invisible indefinitely — every interactive debugging session this machine has ever had was shielded by an ambient `CODESCOUT_MODEL_DIR=./models` export that happens to override `.env` (Docker Compose precedence: process env > `.env` file > inline default). The bug would have first bitten in some future genuinely-clean context (CI, a fresh terminal, another machine, or exactly this systemd unit on the very first real reboot after enabling it) — at a moment with far less context loaded than right now, likely reading as a fresh, confusing GPU/VRAM-looking crash-loop (dense-gpu AND reranker-gpu both failing simultaneously) rather than the one-line fix it actually was.

**Confirming data points:**
1. This session (2026-08-29) — `dense-gpu`/`reranker-gpu` crash-looped only via the systemd unit path, not via my earlier manual restart, and the root cause (`docs/issues/archive/2026-08-29-stale-model-dir-env-masked-by-shell.md`) was found and fixed within minutes once the clean-env log evidence was read directly (`No such file or directory` + an empty root-owned host directory with a matching mtime) rather than re-guessing a resource-contention story.
2. Pending: any future "add automation for X" task on this machine that previously only ran interactively.

**Impact:** high — this was a real, previously-invisible defect in a live config file that would have blocked the exact automation just built, on its very first real trigger (next reboot), with no advance warning.

**Promote-when:** A second instance of "testing new automation via its real invocation path (not a manual shell re-run) surfaces a bug that manual testing had been silently masking." At 2 datapoints, promote to CLAUDE.md / the reconnaissance skill: "When adding a boot/CI/cron-triggered automation for an existing manual workflow, test it via that exact clean-environment mechanism before declaring it done — a manual shell re-run inherits ambient state the automation will not have."

**Status:** validated — single datapoint, drift caught and fixed before the unit's first real unattended boot.

**Valid:** dated 2026-08-29

One confirmed datapoint; promote-when threshold (2 datapoints) not yet reached.

**Rests on:** `docs/issues/archive/2026-08-29-stale-model-dir-env-masked-by-shell.md` — the bug file this win's counterfactual is built on.

## Template for new entries

<!-- New F-N / W-N entries land above this line. This heading is the anchor:

     artifact(action="append_entry", id="<artifact id>", id_prefix="F",
              anchor_heading="## Template for new entries",
              title="<one-line title>", body="**Observed:** ...")

     The server allocates the id, writes `## F-N — <title>` at the ledger's
     own level, records the high-water mark and stamps `**Valid:** dated
     <today>` — one write. Then add the Index / Wins Index row with the id
     it returned. Do not hand-allocate; do not pre-write the row. -->
