---
kind: tracker
status: active
title: Session Log — Embedder Stack Ops
owners: ["marius"]
tags: ["embeddings", "retrieval", "docker", "gpu"]
topic: embedder stack ops
entry_prefix: ["F", "W"]
entry_high_water_F: 11
entry_high_water_W: 6
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
> doc(action="append_entry", id="<artifact id>", id_prefix="F",
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
> **`edit_file` is not the append path**, though it works at first.
> This template ships without frontmatter, so a fresh copy is directly
> editable — but once you declare `entry_prefix` to make the ledger
> guarded (which `get_guide("tracker-conventions")` tells you to do), the
> librarian guard refuses direct edits and only `append_entry` writes.
> Reach for `edit_file` for the prose sections and the index tables,
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
| F-3 | 2026-09-15 | high | tooling | open | `/code-review` given a PR number reviewed the local working tree instead, returning 14 findings about peers' uncommitted code and none about the PR |
| F-4 | 2026-09-15 | med | shared-checkout | open | The foreign-index guard's remedy ("re-stage by explicit path") is a git no-op after a blanket add, and only the action it warns against clears it |
| F-5 | 2026-09-15 | high | ledger-integrity | open | Merging a ledger-touching PR leaves the local id allocator stale, so the next append mints a colliding id rather than erroring |
| F-6 | 2026-09-15 | high | build-provenance | open | A rebuild is not a rebuild of what you merged — `cargo rb` on a tree behind origin exits 0, updates mtime, and ships the old binary |
| F-7 | 2026-09-16 | high | claim-scope | open | A value that is CORRECT about a question you were not asked — re-verification returns the same correct value, so "verify harder" is a no-op |
| F-8 | 2026-09-16 | med | observer-blindness | open | Authored a class's tell and tripped it 3h 25m later — no instrument ran, so there was nothing to re-derive and no control to fire |
| F-9 | 2026-09-16 | high | claim-scope | open | Every wrong claim in a five-instance day asserted an ABSENCE — the first testing law (monotone-under-removal) applied to claims, not tests |
| F-10 | 2026-09-17 | high | data-loss | open | A heading-only `replace` silently wipes the section body — the shrink guard is whole-file, so a single-section wipe is 7.7% and invisible |
| F-11 | 2026-09-18 | med | test-isolation | fixed-verified | `temp_env`'s lock protects only its participants — a plain ambient-env reader in the same test binary raced two new env-mutating tests and reproduced in the real gate |
| W-5 | 2026-09-17 | — | shared-checkout | validated | A peer's in-flight doc edit red the gate; positive attribution (HEAD-vs-worktree + file-provenance) beat reading my own diff, and the red cleared itself |
| W-6 | 2026-09-18 | — | test-design | validated | A differential assertion for one function surfaced an unrelated, previously-unfiled bug (a malformed project.toml silently resolving the default model) as a side effect |

## Wins Index

| ID | Date | Impact | Pattern | Counterfactual | Status |
|----|------|-------:|---------|----------------|--------|
| W-1 | 2026-08-29 | high | test-via-real-invocation-path | .env's stale CODESCOUT_MODEL_DIR would have stayed masked indefinitely, first breaking on the unit's own first real boot | validated |
| W-2 | 2026-09-14 | high | call-graph-before-recommending-removal | Would have recommended mirroring v1's dimension-migration machinery for v2 (doubling it) instead of retargeting and deleting v1 | validated |
| W-3 | 2026-09-15 | high | measure-the-remedy-not-only-the-diagnosis | The approved fix (`wal_checkpoint`) would have shipped the same stale backup while reading as diligence; measuring it inverted the remedy to `VACUUM INTO` | validated |
| W-4 | 2026-09-16 | high | publish-figures-specific-enough-to-be-refuted | Five instruments returned plausible unsupported values in one session; the detector was a second party's independent figure, not care — and only a contradictable report can collide | validated |

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

## F-3 — `/code-review` given a PR number reviewed the local working tree instead

**Valid:** dated 2026-09-15

**Observed:** 2026-09-15, invoked `/code-review` with args `20 high` to review pull request #20. The skill reported its own scope as `git diff @{upstream}...HEAD` (14 commits) + `git diff HEAD` + untracked files — i.e. the LOCAL branch and peers' uncommitted work — not the pull request. Its 14 findings were in `src/tools/output_buffer.rs`, `src/tools/run_command/*`, `scripts/pre-push-foreign-session-guard.sh` and an untracked `AGENTS.md`. PR #20 touches **none** of those files; it is entirely under `src/librarian/`.

**Impact:** the pass consumed 203k subagent tokens / 35 tool calls / 618s and returned **zero** findings about the target. That is worse than an empty result, because the findings are *real defects in other sessions' in-flight code* — so the output reads as a substantive review and invites editing a peer's uncommitted Rust, which this repo files as its own defect (`docs/issues/archive/2026-09-03-the-gates-first-step-reformats-every-peers-uncommitted-rust.md`).

**Cost had it been trusted:** PR #20 carried a load-bearing defect — `std::fs::copy` of a WAL-mode catalog as the sole backup before a destructive vector rebuild, measured at 51 committed rows live / 1 in the copy. A reviewer reporting "review found nothing in the PR" on the strength of this pass would have merged it unreviewed. It was caught by a manual read running in parallel, not by the skill.

**Why it is hard to notice:** the skill *does* print its scope, but as a statement of fact rather than as an echo of the argument it was given. A caller who passes `20` and reads back `@{upstream}...HEAD` must spot that the two disagree; nothing errors, and a PR number is not rejected as unsupported. Same shape as this corpus's recurring *plausible answer rather than an error*.

**Status:** open. `/code-review` is a Claude Code built-in, not a project surface, so the repair is upstream and not ours to make. Recorded here so the next session reviewing a PR does not spend a full pass rediscovering it — and, if using it on a PR, reads the findings' file paths against the PR's own file list before drawing any conclusion.

## F-4 — The foreign-index guard's remedy is a git no-op in exactly the case it is printed for

**Valid:** dated 2026-09-15

**Observed:** 2026-09-15, in an isolated worktree with its **own** index and **no peer involvement at all** (verified: `git rev-parse --git-path index` resolved to `.git/worktrees/pr20-review/index`, and zero other sessions had that worktree as cwd). Staged four of my own files with `git add -A`, then committed. `pre-commit-foreign-index.sh` refused, listing three of the four under `theirs:` with cause *"blanket add — the staging command did not NAME these paths"*, and printing the remedy: **"Re-stage by explicit path and the bare commit passes."**

Ran exactly that — `git add` naming all four paths. **The refusal was identical.** Re-staging changed nothing.

**Mechanism, read rather than inferred:** `scripts/post-index-change-stage-log.sh`'s own header states that ownership is keyed on the pair *(staged blob, path)*, so *"a later index write that does not change the content … introduces no new pair and reassigns nothing."* A blanket add stamps `-` deliberately (`names_path` refuses directory/`-A` forms on purpose — claiming a subtree would hand a session its peers' files). So after a blanket add the rows are unowned, and the one action the guard names to reclaim them is, by the recorder's own design, a git no-op.

**Only `git reset` clears it** — and the same guard's text says *"Do NOT `git checkout` or `git stash`"*, with the surrounding sequence warning that `git reset` on a shared checkout takes a peer's work out of the index. Correct advice for the shared index; in a private worktree it is the sole route, and nothing distinguishes the two cases for the reader.

**Not a rediscovery, and the distinction is the point.** `docs/issues/archive/2026-09-02-a-refused-pathspec-commit-stamps-your-own-content-unowned.md` names this mechanism verbatim — *"they cannot reclaim them, because re-`git add`ing byte-identical content is not an index write, so `post-index-change` never fires"* — and is `status: fixed` (`cd1b138e`, patch-id `c374900d02eb47a131fc18c5e802e321ebf3dca4`). But that fix closed the **temp-index** route: the recorder now exits early when `GIT_INDEX_FILE` is not the shared index. The **blanket-add** route still produces `-` rows by design and reaches the identical unreclaimable state. A residual of a fixed bug via a second route, not a re-file.

**Class:** this is a REMEDY-TEXT defect, which CLAUDE.md § *Testing Discipline* names as untested by construction — every assertion is about *who is refused*, none about *where the refusal sends you*. The predicate here is right (a blanket add genuinely is the capture case). The remedy names an action that cannot produce the state it promises. The shape test would pass: the message names an addressee and an action.

**Cost:** two refused commits and a full re-stage cycle. Low in isolation; the concern is that a session reading the remedy and seeing it fail has no reason to suspect the remedy rather than themselves, and `--no-verify` is one keystroke away and explicitly the wrong habit.

**Status:** open. Owed a bug file against the remedy text — a candidate wording is to branch on whether the index is shared: private worktree → say `git reset` then re-add by name; shared index → say a pathspec commit, and say that a blanket add cannot be reclaimed in place.

## F-5 — Merging a PR that touches a ledger leaves the local id allocator stale, and it mints a colliding id

**Valid:** dated 2026-09-15

**Observed:** 2026-09-15, immediately after merging PR #20 (which added `W-2` to **this ledger**) into `origin/experiments`. Preparing to append a `W` entry here, `doc(action="get")` reported `entry_high_water_W: 1`. Verified at the bytes:

| | local working tree | `origin/experiments` |
|---|---|---|
| `entry_high_water_W` | **1** | **2** |
| `W-` index rows | 1 | 2 |

The local checkout was 5 commits behind origin (`8c217e1a..17c9a338`, derived at that instant) and carried peers' uncommitted work, so it had not taken the merge. `append_entry` allocates from the **local** file — `max(frontmatter high-water, body max + 1)` — so a `W` append at that moment would have minted **`W-2` a second time**, colliding with the entry already merged.

**Why it is this corpus's recurring shape:** the allocator would have returned a *plausible id, not an error*. Two `## W-2 — …` sections are both valid markdown; `link_scan` would bind the token to two definers and report Ambiguous; nothing fires at write time. The condition is invisible from inside the allocator, which is reading its file correctly — the file is simply older than the fact.

**The trigger is SUCCESS, which is what makes it easy to walk into.** Merging the PR is what desynchronised the allocator from its own ledger. The riskiest moment to append to a ledger is right after landing a change that touched it — precisely when a session is most likely to be writing up that work. Nothing in the merge path, the ledger, or `append_entry` notes the staleness.

**Detected only because the scout read the high-water mark before appending** rather than after. Had the `W` been written first and verified after, the collision would have been durable and would have surfaced as an Ambiguous citation later, far from its cause.

**Scope, stated rather than assumed:** `F` was unaffected here — both sides sat at 2, so `F-3` was safe either way, and F-3/F-4/F-5 were written on that basis. This is not a property of `F`; it is a coincidence of this ledger's two namespaces advancing independently. A merge touching the `F` half would poison `F` identically.

**Narrowest seam for a mechanism:** `append_entry` already knows the artifact's path and the repo. Comparing the ledger's blob against its upstream counterpart (or simply refusing when `git rev-list --count HEAD..@{upstream} -- <ledger path>` is non-zero) would turn a silent collision into a refusal naming the pull that clears it. Cheaper alternative with no git dependency: have the merge path mark touched ledgers stale in the catalog.

**Status:** open. The `W` entry this scout was written for is deliberately **unwritten** and held until the local checkout is reconciled — reconciling a shared tree carrying peers' live edits is not a call this session should make unilaterally.

## W-2 — Pre-decision scout confirmed artifact_vec (v1) is dead in production before recommending its removal

**Valid:** dated 2026-09-14

**Observed:** 2026-09-14, asked "can we remove v1 (`artifact_vec`)? is it still needed?" —
prompted by docs/issues/2026-09-14-artifact-vec-v2-hardcoded-768-dim-no-migration-path.md's own
closing suggestion ("consider whether artifact_vec is still read by anything").

**Pattern:** Before answering an architecture removal question from a hunch or from the bug
file's own speculation, scouted the actual call graph: `call_graph(write_embeddings,
direction="callers")` and `call_graph(write_embeddings_with, direction="callers")` in
`src/librarian/indexer.rs` — both return **zero non-test callers** (3 and 8 edges respectively,
every one inside `#[test]` functions). Read `SqliteVecArtifactStore::knn`'s body directly
(`src/librarian/artifact_store.rs:530-554`) — its SQL targets `artifact_vec_v2` only, despite the
method's own doc comment two lines above still describing the schema in terms of `artifact_vec`
(v1) — a live instance of this project's own `cluster/doc-contradicted-by-code` tag. The
project's own regression test `the_sqlite_store_writes_a_chunk_id_into_v2_and_never_into_v1`
(same file) independently asserts the same thing. Cross-checked against
`docs/adrs/2026-07-20-artifact-vec-shared-catalog-boundary.md` (predates chunk-grain retrieval;
decided to keep the *sqlite-vec backend* as a permanent no-Qdrant escape hatch — a decision this
finding does not disturb) and `docs/superpowers/specs/2026-09-02-artifact-chunk-grain-retrieval-design.md`
(the migration that introduced v2 and the "keeping both alive avoids a dark window" bridge
comment at `catalog/mod.rs:262` — a comment describing a cutover period, not a permanent
coexistence).

**Counterfactual:** Without the call-graph scout, the natural answer to the 2026-09-14 bug's own
"mirror v1's mechanism for v2" suggested fix would have been to build a **second**,
near-duplicate `rebuild_artifact_vec_v2_at_dim` alongside v1's existing
`rebuild_artifact_vec_at_dim` — permanently maintaining two dimension-migration paths for a table
(v1) that turns out to have no production writer or reader at all. The scout changes the fix
shape entirely: retarget the one existing migration path at v2 and delete v1's copy (table,
trigger, `write_embeddings`/`write_embeddings_with`, `gc::migrate_vec_id`, and the three
per-catalog-open orphan-`DELETE`s in `catalog/mod.rs`) in the same change, rather than doubling
the machinery.

**Confirming data points:**
1. `write_embeddings` — 3 callers, all in `#[test] fn`s (`embeds_artifact_into_vec_table`,
   `write_embeddings_is_idempotent_on_same_id` ×2).
2. `write_embeddings_with` — 8 direct callers, all `#[test] fn`s, plus the `write_embeddings`
   wrapper (itself test-only).
3. `SqliteVecArtifactStore::{upsert,delete,refile,knn}` — all four target `artifact_vec_v2`
   exclusively; `upsert`'s own comment says so explicitly ("`artifact_vec_v2` is keyed by
   `chunk_id`").
4. `gc::migrate_vec_id` (v1's id-rehome helper) still has one real production caller (the
   catalog-level artifact-rehome path, `catalog/gc.rs`), but is a guaranteed no-op there today
   since nothing ever writes a row into v1 under any id.

**Impact:** high — prevented recommending a parallel-implementation fix for an open, filed bug
(the 2026-09-14 hardcoded-dimension issue) that would have doubled long-lived migration machinery
around a table with zero production traffic in either direction.

**Promote-when:** N/A — single-decision scout, not a recurring pattern proposal.

## W-3 — Measuring an already-approved remedy inverted it — `wal_checkpoint` reports busy in a row callers discard

**Valid:** dated 2026-09-15

**Observed:** 2026-09-15, reviewing PR #20. Found that `rebuild_artifact_vec_v2_at_dim` backed the catalog up with `std::fs::copy` of a `journal_mode = WAL` database, which omits the `-wal` sidecar. Proposed the obvious remedy to the operator — `PRAGMA wal_checkpoint(TRUNCATE)` before the copy — **and they approved it.** Then measured it before writing it.

**Pattern:** treat an approved remedy as a hypothesis until it has been run. The measurement took two minutes in a scratch SQLite database and inverted the fix:

| remedy | with ONE concurrent reader holding a transaction |
|---|---|
| `PRAGMA wal_checkpoint(TRUNCATE)` | returns `(busy=1, log=100, checkpointed=100)` — **incomplete** |
| `VACUUM INTO` | complete snapshot, **51/51 rows** |

`wal_checkpoint` reports `busy = 1` *in its result row* rather than failing. `catalog.db` is shared by every codescout process on the machine — 6 sessions in this checkout at the time — so busy is the ORDINARY case here, not the edge one. Shipped `VACUUM INTO`, whose failure is an error rather than a discarded row.

**Counterfactual — and it is worse than "the fix would not have worked".** A caller runs `wal_checkpoint` through `execute_batch`, which **discards result rows**. The approved fix would therefore have produced the *same stale backup* as the bare `fs::copy`, while adding a step that reads as diligence — a `PRAGMA` call sitting above the copy, which any later reader would take as the WAL problem already being handled. The bug file for this defect is tagged `cluster/record-asserts-an-unchecked-completion` (IC-8), so the approved remedy would have reproduced the exact class it was fixing, one level down, inside its own fix. That is recorded on IC-8's `**Members:**` field, because the next person will reach for `wal_checkpoint` too.

**Confirming data points:**
1. The backup staleness itself was measured, not reasoned: 51 rows visible to the live connection, **1** row in the `fs::copy`. The live catalog at the time was 420 MB with a 4.2 MB `-wal` written seconds earlier, so the exposure was real rather than theoretical.
2. The mutation is the discriminator: reverting the production path to `std::fs::copy` reds the new test (`left: 0, right: 1`) while the pre-existing `write_embeddings_v2_migration_backs_up_file_backed_catalog` stays **green** — it asserts a file whose NAME matches and never opens it.
3. `VACUUM INTO`'s one real constraint was checked too: it refuses inside an open transaction. That error is propagated rather than falling back to a copy, because a rebuild that destroys vectors must not proceed on a backup that cannot restore.

**Why this is a win and not merely a fix:** the operator had already approved the wrong remedy, and nothing downstream would have caught it — not the gate, not CI, not the existing test. The only thing between the approval and shipping it was running it once.

**Promote-when:** pairs with `F-1` in this same ledger (*asserted root cause before measuring*). Two datapoints now for one discipline in opposite directions — `F-1` is measuring before **diagnosing**, this is measuring before **remedying**. A third would justify promoting *"measure the remedy, not only the diagnosis"* into CLAUDE.md.

## F-6 — A rebuild is not a rebuild of what you merged, and every signal except behaviour says it is

**Valid:** dated 2026-09-15

**Observed:** 2026-09-15. After PR #20 merged to `origin/experiments` at 13:22, the operator ran `cargo rb` at 14:08 and `/mcp` to pick up the fix. **The binary did not contain it.** At 14:08 local `HEAD` was `8c217e1a`, and `git merge-base --is-ancestor cbbfb7be 8c217e1a` is false — the local checkout was 5 commits behind origin at that instant (`8c217e1a..17c9a338`; the figure read **15** until 2026-09-15 and was simply wrong, caught by sessionId `f0b1a4c7-e991-4478-bf22-b088483b6821`, who also measured **7** against `506924f2` — a different comparand, and correct for its own question. A bare count with no comparand is what made two right answers look like a disagreement), carrying peers' uncommitted work, so `cargo rb` compiled a tree three minutes short of the reconciling push that landed at 14:11.

**Every available signal said the rebuild was current.** `cargo rb` exited 0. The binary's mtime updated. `~/.cargo/bin/codescout` resolved correctly through the symlink. `codescout --version` returned `0.15.0` — unchanged by the merge, so it discriminated nothing. `codescout doc --help` succeeded, which only proves the librarian is compiled in, not which schema it carries. Nothing anywhere reported "you built a tree that does not contain what you merged."

**What actually settled it — a BEHAVIOURAL probe, not an inspection.** Point the binary at a scratch catalog and read what schema it writes:

```
LIBRARIAN_DB=<scratch> codescout doc find --kind tracker
-> schema version written: 12; creates v1 artifact_vec: YES  => pre-v13 binary
```

After a second `cargo rb` on the reconciled tree (`HEAD` = `506924f2`): `13`, no v1 table. That is a one-command discriminator and it is the only check that answered the question.

**The inspection route failed, and its failure is the instructive half.** `strings ~/.cargo/bin/codescout | grep -c 'DROP TABLE IF EXISTS artifact_vec'` returned **0** — the correct verdict, reached by a method that did not establish it. The control proves that much: `LIBRARIAN_ARTIFACT_VEC_MIGRATE` also returned **0** on that binary, and that constant exists in *both* pre- and post-PR source, so the probe was under-reporting regardless of which answer it gave.

**NARROWED 2026-09-15 by sessionId `f0b1a4c7-e991-4478-bf22-b088483b6821`, who ran the control I did not.** This entry first explained the 0 as *"Rust merges literals into large `.rodata` blobs, so a line-oriented search over a release binary cannot express the question."* That is **wrong as a general claim**. On the post-rebuild binary both `LIBRARIAN_ARTIFACT_VEC_MIGRATE` and `rebuilding artifact_vec_v2 at new dimension` are **found, 1 hit each** — verified here independently, and identical under `strings`, `strings -a` and `strings -n 6` (`-a` and the default emit the same 372,081 lines on this file, so the flag is not the variable either).

**Why the difference cannot be attributed, which is itself the finding.** The two binaries differ in content *and* in feature set — 65,061,280 bytes with `local-embed` then, 42,560,784 without it now — and **the 62 MB binary no longer exists**, overwritten by the rebuild. So the original 0 is **not reproducible**, and "the build merged those literals differently" and "my probe was flawed in a way I can no longer reconstruct" are both live explanations. Recording that rather than picking the flattering one.

**The correct claim is the peer's, and it is sharper than the original:** a method whose reliability varies per build is **worse than one reliably broken**, because nothing tells you which regime you are in — a **0 is uninterpretable, a 1 is sound**. Asymmetric, not absent. The standing instruction is unchanged and is the reason this entry exists: **ask the binary what it does, not what it is.**

**Same shape one step later — and the second half of this paragraph was WRONG until 2026-09-15, which makes it the third instance rather than the second.** Verifying `local-embed` had actually left a subsequent build, `strings` reported `onnxruntime: 7` — merged-blob hits saying nothing about linkage. I then ran `ldd`, called it *"the decisive test"*, read `no onnxruntime in the link map`, and concluded the feature was gone.

**`ldd` cannot answer that question.** `crates/codescout-embed/Cargo.toml:18` declares `local-embed` as a **static prebuilt ONNX Runtime** (`fastembed/ort-download-binaries-native-tls`); the dynamic variant is a *separate* feature, `local-embed-dynamic`, used only on windows-gnu. A statically linked runtime appears in no link map, so `ldd` returns the same empty result whether the feature is in or out — a **one-directional instrument presented as decisive**, which is precisely the defect the `strings` half of this entry is about. I could not even compare against the previous binary, since it had already been overwritten.

The conclusion survives, on evidence that does discriminate: the **62 MB -> 40 MB** drop, which is exactly what removing a statically linked runtime costs. The right answer, reached by an instrument that could not reach it, rescued by one that could.

So: three probes in one session (`strings` twice, `ldd` once), each returning a plausible result none of them established, and each corrected only by a control or a second instrument. Found while auditing a *peer's* bug file about the `strings` half — not by re-reading my own entry, which I had already revised twice.

**The instrument-level defect is filed separately and is NOT this entry**, because the two have different readers: [`docs/issues/2026-09-15-probes-recommends-strings-to-settle-a-binarys-contents-and-a-miss-proves-nothing.md`](../issues/2026-09-15-probes-recommends-strings-to-settle-a-binarys-contents-and-a-miss-proves-nothing.md) (`ded9143998081efc`) covers `docs/PROBES.md`:180 offering inspection as co-equal to a behavioural probe. **That is the recommendation site; this is the incident.** A reader deciding how to verify a binary consults PROBES.md and never opens a session log, which is why recording the lesson here would not have reached them — § *Observer Blindness* position 3.

**Two sub-kinds inside that family, and the distinction is worth carrying:** `strings`' blindness is **contingent** — measured per-BUILD, 0 on the 65 MB binary and 1 on the 42 MB one for a control string present in both source versions, so you cannot know which regime you are in. `ldd`'s is **structural** — a statically linked runtime appears in no link map, by design, always. The second is worse in that it is never right for this question, and better in that it is knowable in advance from `Cargo.toml` alone. A reader told only *"strings is unreliable"* reaches for `ldd` and lands on the worse one.

**Cost:** one wasted rebuild plus a reconnect, and a window in which the operator believed the WAL-backup fix was live when it was not — so any dimension migration firing in that window would still have taken the broken `fs::copy` backup. Nothing indicates one did (`artifact_vec_v2` intact at 768 dims, 61,489 vectors).

**Why it belongs beside `F-5`, not folded into it.** `F-5` is a *ledger* allocator reading a stale local tree; this is a *compiler* reading one. Same class — an instrument correctly reading a tree that is older than the fact, and reporting success — reached through two unrelated subsystems. Both were caused by the same underlying condition (local behind origin on a shared checkout) and neither names it.

**Narrowest seam for a mechanism:** `cargo rb` is an alias in `.cargo/config.toml` and cannot run a precondition. A `scripts/rb.sh` wrapper could refuse, or warn, when `git rev-list --count HEAD..@{upstream}` is non-zero — the same shape as `gate.sh` wrapping the four gate commands, and the same argument for it (§ *Observer Blindness* position 3: make the correct path end in a safe state). Until then, the standing instruction is the probe above: **after any rebuild you are relying on, ask the binary what it does, not what it is.**

**SHARPENED 2026-09-15, same day, by the first use of this entry — and it corrects a reading this entry invites.** The text above lists "the binary's mtime updated" among the signals that misled, which a reader can easily invert into *check that mtime moved*. That check produces a FALSE ALARM. Asked to verify a later rebuild, mtime was **unchanged** (`14:54:32`, read at `15:25`) — and the binary was **correct**: `git log --since=@<binary mtime> -- '*.rs' 'Cargo.toml' 'Cargo.lock'` returned **0** commits and the tree held no uncommitted Rust, so `cargo` had nothing to relink and skipped the write. The behavioural probe confirmed schema 13.

So mtime is uninformative in **both** directions, and that is strictly stronger than the original claim:

| mtime | content | when |
|---|---|---|
| **updated** | **stale** | built from a tree behind origin (the 14:08 case above) |
| **unchanged** | **current** | nothing to rebuild; cargo correctly no-ops (the 15:25 case) |

A session trusting mtime gets a false negative in the first row and a false positive in the second. The two checks that *do* discriminate are the ones to run: `git log --since=@<binary mtime>` over `*.rs` + manifests for whether a rebuild was even owed, and the scratch-`LIBRARIAN_DB` schema probe for what the binary actually does. Neither is about the file's metadata.

**Status:** open.

## W-4 — Five plausible-but-unsupported values in one session, and the detector was a peer's independent figure

**Valid:** dated 2026-09-16

**Observed:** 2026-09-15/16, across one PR review and the cross-session thread it produced. **Five** instruments in one session returned a plausible value none of them established. None failed with an error; every one returned a well-formed result of the expected *shape*.

| instrument | returned | actually |
|---|---|---|
| `strings \| grep -c 'DROP TABLE…'` | `0` | correct verdict, method could not support it — and the control (`LIBRARIAN_ARTIFACT_VEC_MIGRATE`, present in both source versions) also returned `0` |
| `ldd \| grep onnxruntime` | empty | non-discriminating: `local-embed` links ONNX **statically**, so absence is identical either way |
| a hand-rolled mutation | `PASS` | the patch **never applied** — the anchor had moved in a refactor |
| *"does the test still red?"* | `FAILED` | it did — on an `unwrap`, not the assertion. The discriminator (`left:`/`right:` absent) was in my own output |
| `… \| tr -d '[:space:]' \| sort -u \| grep -c .` | `1` session | **6** — `tr -d '[:space:]'` strips newlines, concatenating every sid into one record |

**Pattern:** the detector that worked was **two parties deriving the same figure separately and disagreeing** — not care, not re-reading, not a checklist. Of the five, **two** were caught that way and the rest by a control run *because* a peer had asked for one.

**Counterfactual, and it is not hypothetical — it is the two that nothing else caught.** The `ldd` error survived **two deliberate revisions of `F-6` by its own author**; I read past it both times and found it only while auditing a *different* session's bug file on a different framing. The `tr -d` error I published as `1` and re-derived only because sessionId `9403d62d-116b-46ea-ac9b-004acff2b1cb` had independently said `6`. Neither catch came from either session looking harder. Both came from a second derivation existing at all.

**The refinement that makes this actionable is `aa272bed-7d33-4e5e-bcbf-2ccf3b4c4c66`'s, relayed by `9403d62d`: the value is in being CONTRADICTABLE.** A report that says *"looks fine"* collides with nothing and cannot be the detector for anything. *"28 commits, 6 sessions, 4 mine, tip `43fdc0ea`"* can be **wrong**, and that is precisely what makes it load-bearing. This reframes the practice from *publish your conclusions* to **publish figures specific enough to be refuted** — which is the same instruction CLAUDE.md § *Testing Discipline* gives for counts (*"arrive with its unit or not at all"*), arriving here from the cross-session direction rather than the measurement one.

**Why it is a win and not simply five mistakes:** the mistakes are the denominator. Publishing the confirmations *and* the refutations is what made the population countable at all — CLAUDE.md § *Testing Discipline* asks for exactly that (*"instrument the DOUBT, not the correction"*), and this is the first entry in this ledger with enough instances to show the shape. The corpus of five is the finding; no single one of them would have been.

**Confirming data points:**
1. Both directions observed by this session in one day — my `28` was the control that caught `9403d62d`'s heading-scan; their `6` was the control that caught my `tr -d`.
2. `9403d62d` independently reproduced the broken pipeline on the real range (`1` vs `6`) rather than accepting my explanation, and added the sum-to-28 check that neither count alone provides.
3. A third instance in the same thread, theirs: they attributed `c3135fa3` to `29420e72` from **thread memory** rather than the `Session-Id` trailer — adjacency, the one method CLAUDE.md names as *anti*-evidence, inside a message about an attribution fix. Caught by the party misattributed.

**Promote-when:** at a third independent work stream. This is two (the PR-review stream and the cross-session verification thread), and the instances are correlated — same day, same participants. Do **not** promote on this entry alone; the population is small and the sessions were talking to each other, which is exactly the shared-blind-spot condition CLAUDE.md warns makes agreement worthless.

## F-7 — A value that is CORRECT about a question you were not asked — and re-verifying it returns the same correct value

**Valid:** dated 2026-09-16

**Status:** open · **Severity:** high · **Category:** claim-scope

**Observed:** Four times in one session — the first two inside a single turn — I published
a figure that was fully supported and still wrong, because it answered a *different
question* than the claim it was attached to. None was a guess; all four survive
re-verification.

| what I published | the value is CORRECT for | the claim attached it to |
|---|---|---|
| "`a3579710` went to origin in your push range" | a real commit by sid `9403d62d`, already at origin | which of their commits *my push* carried — actually `55cbf9a3` |
| "CI on the range: 23 success, 0 failed, 1 cancelled" | run `35055091243` (`43fdc0ea`) exactly | the **range** of three pushes — `64411fe0`'s run had **7** cancelled |
| "the nine staged paths are `9e022ef0`'s" | eight of the nine, exactly | the ninth, `src/tools/output_buffer.rs`, had **two** live writers in flight — one author reported for a set with two |
| "no tracker, plan, or roadmap entry for it — I checked" | `docs/trackers/` and `ROADMAP.md`, where it is genuinely absent | **the project** — an `active` spec decides BOTH questions, in `docs/superpowers/specs/`, which that search never reaches |

**The fourth is the worst of the four and the only one published outside this repo.** It
went on GitHub #16, to a reporter who had audited the code correctly and asked two
questions. *"Nobody has considered this"* is the single answer that makes someone stop
looking, and both questions in fact had decided answers with rationales — § *Architecture*
declares dense-only *"an error, not a downgrade"*, and § *Decision* lists their exact case,
*"add a second weights source"*, among the absorbed change scenarios. Corrected publicly
and filed into that spec's § *Deferred* in `606e2df0`.

**Why the standard remedy is a no-op here.** "Verify before asserting" resolves to
*re-check the value*, and the value is right. `git log a3579710` confirms a real
commit by that session. `gh run view 35055091243` confirms 23 success. Every
instrument aimed at the number agrees, every time, and agreement is exactly what
a correct answer to the wrong question produces. The defect is not in the value
but in the **binding** between the value and the claim's scope word — *"in my
push"*, *"the range"* — and no instrument pointed at the value can read that word.

**Mechanism: holding a value suppresses the question that would scope it.** In
both cases the correct instrument was one command away and no more expensive than
what I did: `git rev-list 64411fe0..02ff86ee` (2 lines) instead of recalling a sha
from the thread; `gh run list --branch experiments` instead of reading one run.
Cost was never the barrier. The instrument went unreached because I already had a
number in hand, and a number in hand does not feel like an open question.

**The sha had no source but me, which is the sharpest part.**
`scripts/pre-push-foreign-session-guard.sh:369` is
`printf '%s\n' "$ack_matched" | tr ',' '\n'` — the guard prints **sids and never
shas**. So no surface handed me `a3579710`; I produced it from thread salience.
That is `F-165`'s own mechanism — attribution by thread adjacency — committed
inside the notification about the commit that *is* `F-165`.

**Detector, both times: a peer holding the other end of the binding.** The session
that authored the commit knew which of theirs was outstanding. The session watching
all three CI runs knew what the other two cost. Neither detection was available to
a more careful re-measurement on my side, and both arrived as corrections rather
than as alarms.

**Distinct from `W-4`, and the distinction is the point.** `W-4` catalogues
*plausible but unsupported* values — a broken `tr`, a per-build `strings` blindness,
a non-discriminating `ldd`. Those fail under re-derivation. These do not: they are
**supported and mis-scoped**, so the `W-4` discipline (derive it, don't cite it)
runs clean over them and returns the same wrong claim. Two different failure modes
wearing one word, "wrong number".

**Remedy, and it is about the sentence rather than the check.** Re-derive the
**scope word**, not the value: when writing *"X went out in my push"*, produce X
from the push range at write time; when writing *"N green"*, name the run id in the
same breath, because a figure that names its run cannot silently be read as a
range. This is § *Testing Discipline*'s unit rule pushed one notch — the unit here
was not missing, it was **implicit and wrong**, and the value was right for the
unit it actually had.

**Open question, PARTLY answered by the third instance.** The first two were caught
by a peer — § *Observer Blindness* position 2, a reviewer who does not share the
author's context — with no position-3 mechanism proposed. The third produced one.
Run with its **default** window, `scripts/file-provenance.py` returns `UNKNOWN` for
`src/tools/output_buffer.rs` and refuses to let that be read as a finding:

> no record of any session writing this path in the window. That is a statement about
> coverage, NOT about ownership — Bash writes this tool's heuristics miss look
> identical. Do not read it as 'not mine'.

Had it printed a bare `0`, the file would have been reported **unowned** — this class
in its most expensive direction, since the next step is to edit it. So a negative
result that **names the question it cannot answer** does reach the class: not by
catching a mis-scoped value, but by declining to supply one. That is the shape
position 3 asks for, and it is an **existence proof, not a general remedy** — it works
because the instrument knows its own window, and nothing equivalent guards a sha
recalled from a thread.

**And the third instance shows the scope word need not be a WORD.** Instances 1 and 2
turned on *"in my push"* and *"the range"* — phrases a writer can re-read. This one
turned on an unnamed **window**: the same instrument, same path, same minute returns
`UNKNOWN` (default), `2` (in flight), or `8 writers, 3 live` (`--all`), and the
disambiguating flag appeared in neither party's message. A figure whose scope lives in
a **flag** rather than in a phrase leaves nothing in the sentence to re-derive, which
is why the remedy above (*re-derive the scope word*) does not reach it.

Not promoted to an `OB` class: four instances, one session, one day, and both detectors
are peers I had been in continuous conversation with all evening. A further instance from
inside the same thread does not relieve the shared-blind-spot condition — it is the same
population sampled again.

**A FIFTH INSTANCE, NOT MINE, AND IT MOVES THE DETECTION EARLIER.** sessionId `9e022ef0`
reported that this push would make `scripts/rb.sh` *"fire on trees that were fine ten
minutes ago"*, reasoning from *"origin advanced 36 commits"* to *"therefore local is
behind"*. Measured instead of accepted: `HEAD..origin/experiments` is **0** and `rb.sh`
exits **0**, because those 36 commits came FROM this checkout — origin moved to MATCH local
HEAD, not past it. **"Origin advanced" and "you are behind" are different relations, and a
push that publishes local work produces only the first.** Their statement is exactly right
about a *different clone*, which did gain 36 commits it lacks.

Two things this one adds that the four above do not:

- **It was caught BEFORE it reached a record**, at transmission, by the recipient. The
  other four were all corrected after publication. Same detector — § *Observer Blindness*
  position 2, a party not sharing the author's context — applied at a different point in
  the lifecycle. Still an instrument that happened to be present, still not a mechanism.
- **Its cost would have been distrust of a guard rather than a wrong fact.** A false
  operational note filed beside a new guard is how the next reader learns to route around
  it, and a guard readers route around is worse than no guard. The author's own summary,
  which is sharper than mine: *"the tell is that I had no measurement, only a direction."*

That tell generalises past this entry and is the closest thing to a usable check it has
produced: **a claim built from a DIRECTION rather than a reading is where this class
lives.** "Origin moved forward", "the search came back empty", "that commit is theirs" are
all directions. None of them is a number, and each becomes a claim about a situation only
when someone supplies the situation from context.

## F-8 — I authored the check for a class and produced an instance of it 3h 25m later

**Valid:** dated 2026-09-16

**Status:** open · **Severity:** med · **Category:** observer-blindness

**Observed:** I authored a check for a defect class and produced an instance of
that class **3h 25m later**, in the same session, having cited the check twice in
between.

| | |
|---|---|
| tell authored | `16575b1a`, 2026-09-16 **10:52:32Z** — F-7 gains *"the tell is that I had no measurement, only a direction"* |
| tell tripped | ~**14:15Z** — I told a peer that `UNKNOWN`-with-no-hint is *"what the default floor produces most often"* |
| measured by that peer | 12 files, **7** `UNKNOWN`, **5 hinted, 2 silent** — the hinted case is the common one |

I had no measurement. I had a direction: *"a clean file has no writes, therefore no
hint."* That is the tell verbatim, applied to a frequency claim, by the session that
wrote it down.

**This is not `F-7` and not `W-4`, and the separation is the entry's reason to exist.**
`F-7` is a value **correct** about another question; re-verification returns it
unchanged. `W-4` is a value **unsupported** but produced by an instrument. This was
neither — **there was no instrument at all.** Nothing was run, so there was nothing to
re-derive and nothing to control. A claim with no measurement behind it cannot fail
under re-derivation, because re-derivation was never the step that was skipped.

**Why it belongs in the ledger rather than in a message.** It reached the peer who
measured it and nowhere else. Twice today a judgement that existed only in a socket
message was lost — `f0b1a4c7`'s classification died with their session, and the peer who
held the unsent answer said so explicitly. They declined to write this up for me, which
was right: it is my error and my ledger, and filing another session's self-report is the
thing they had just refused to do for a dead peer.

**The interval is the evidence, and the interval itself was first stated unmeasured.** I
reported it as *"four hours"* to two parties before deriving it. It is 3h 25m. An
unmeasured number, inside the account of making unmeasured claims, in the correction of
them. That is not an embellishment of the finding — it is the finding's third occurrence
in one thread, and it is why the entry claims no remedy of the form *"be more careful
with frequencies."*

**What this instantiates.** `CLAUDE.md` § *Observer Blindness* opens with four instances
of one class in one evening, **every one committed by an author actively writing about
that class**, and concludes that knowing the class prevented none of them while a
standing policy caught one. This is a fifth, and better evidenced than some: the gap
between authoring the check and tripping it is a measured interval rather than a
recollection.

**No mechanism proposed, deliberately.** Position 3 asks for a check that runs when
nobody is worried, and I have none for *"is this sentence carrying a number I never
derived?"* The one thing that did fire was a **second party running the measurement** —
position 2, an instrument that happened to be present. Recording the absence rather than
filling it with a resolution to look harder, which is what the four instances above
already falsified.

**Residue kept from the same exchange, because it is independent of the above and neither
claim subsumes the other:**

- *Mine:* a wrong count erring toward **more caution** is not thereby harmless — which
  side of a threshold it lands on is luck, not a property of the error. My `8 foreign
  sessions` against an actual `4` straddled a threshold of 3.
- *The peer's:* an error can be wrong, or wrong **and self-confirming**. `grep -cv '^$'`
  without `sort -u` is too large by a factor and re-derivation fixes it; `pgrep | head -N`
  over `/proc` glob order takes the N **oldest** and re-derivation returns the same wrong
  answer with the same confidence. **Theirs decides whether re-derivation is a remedy;
  mine says the error is not excused by its direction.**

## F-9 — A published absence is monotone under its own falsity — the first testing law, pointed at claims

**Valid:** dated 2026-09-16

**Status:** open · **Severity:** high · **Category:** claim-scope

**Observed:** Five wrong claims across two sessions in one day, and **every one asserted
an absence.** Neither session got a positive badly wrong in the same way. The direction
is the finding.

| claim | reality | reached |
|---|---|---|
| *"There is no tracker, plan, or roadmap entry for it — I checked"* | an `active` spec decides **both** the reporter's questions, with rationales | **GitHub #16 — outside this machine** |
| *"The false record was never public"* | it was; `grep -c` with mangled backtick escaping returned `0` for a line `grep -n` printed two commands later | nearly my operator's decision |
| *"`src/agent/build_check.rs` does not exist"* | a 49 KB wired module, one `ls` away | `9e022ef0`, internal |
| *"We have not corresponded today"* | we had, twice, and the record was in the asserter's own context | `9e022ef0`, to me |
| *(third, `9e022ef0`'s own)* | — | internal |

**THIS IS NOT A NEW MECHANISM. It is `CLAUDE.md` § *Testing Discipline*'s FIRST law,
applied to a population it was never pointed at.** Verified at `CLAUDE.md:151` rather
than recalled:

> Absence assertions (`is_empty()`, `!exists()`) are monotone under **removal** — a dead
> mechanism produces exactly the silence they assert.

An agent asserting *"there is no tracker entry"*, *"no mechanism exists"*, *"we have not
corresponded"* is making an **absence assertion**, and it is monotone under **its own
falsity**: the world where the thing exists and the world where it does not produce an
*identical experience for the asserter* — silence, no contradiction, nothing downstream.
The law explains why a **test** cannot catch it. These five say the same structure
governs the **claims we publish**, where there is no suite at all and therefore not even
a red to miss.

**The asymmetry, which is why this is worth separating from the wrong-value classes.** A
wrong **positive** fails loudly because the world objects — you go to use the thing and
it is not as claimed. A wrong **negative** has nothing downstream depending on it, so it
simply sits there being wrong. That is not a difference in care; it is a difference in
what the error is coupled to.

**Distinct from the three already in this ledger, and the separation decides the
remedy:**

| entry | the claim was | why it survived |
|---|---|---|
| `F-7` | **correct**, about another question | re-verification returns it unchanged |
| `W-4` | unsupported, but **an instrument ran** | a control could have fired and did not |
| `F-8` | asserted with **no instrument at all** | nothing to re-derive |
| **this** | an **absence**, no instrument | narrower than `F-8`: it supplies *why* nothing running is specifically undetectable |

`F-8` records *that* nothing ran. This records *why that is fatal for a negative
specifically*, which `F-8` cannot say.

**REMEDY, and the law already prescribes its shape — this is the one entry today that
ships a runnable one.** *"Ask which direction each test is monotone under, and mutate the
other way."* For a claim: **before publishing an absence, run the instrument that would
return the thing if it existed.** `ls` the module. `grep` the transcript. `git show` the
other ref. Not *"check harder"* — a specific, opposite-direction query, chosen because it
is the one that **can** come back non-empty.

**CORRECTED 2026-09-16, same day, by `9403d62d` — THE TRIGGER IS NOT "I GOT ZERO".** The
remedy above was written over a population that happens to be all zeros: both of my
instances returned `0`, so *"run the query that can come back non-empty"* reads as
something you reach for **after seeing an empty result**. Their instance breaks that, and
it is the more dangerous shape.

They ran `git grep 'docs/issues/2026-09-13-architecture-probe' 215a5cad` to check a peer's
claim that a commit had stranded citations, **got 1 hit**, and read it as *"the peer is
wrong"*. The hit was a fenced quote in their own bug file. Every real citation in `docs/`
is written **relative** — verified here rather than taken on report: **4** files use
`../issues/`, **0** use the repo-rooted form — so the pattern was structurally incapable
of returning the object it was looking for. Re-run unscoped: **7 lines, immediately.**

**A zero at least looks like it might be nothing. A small positive reads as *"I looked,
and here is what is there"* — and invites no second look at all.** Their words, and the
correction is theirs: the trigger cannot be the count. It has to be **"I am about to
assert an absence"**, whatever came back. A wrong negative that returns `1` has already
passed the only check the original wording would have prompted.

So the remedy stands and its *precondition* was wrong. Run the opposite-direction query
because you are about to publish an absence — not because the first query looked empty.

**Their population is three, not one**, and counting the incident rather than the
population is its own instance of this entry: the `git grep` above; a whole-file grep for
a quoted phrase returning `1`, where the `1` was their own prose citing the quote; and a
blank stage-log probe read as *"owners are unknowable"* when their `awk` was simply wrong.
All three self-reported against their own interest, unprompted.

**Blast radius is not symmetric, and the tally must not read as though it were.**
Contributed by `9e022ef0` against their own interest: their three are internal and were
noticed same-day. Mine on GitHub #16 is worse than all three together — public, to a
reporter who had audited the code correctly, and *"nobody has considered this"* is the
single answer that makes someone stop looking. It is also the only one of the five that
left this machine.

**Attribution, split because the halves were earned differently.** The monotonicity
framing — that this is the first testing law pointed at claims — is `9e022ef0`'s, and it
is what makes the entry cheap: it inherits a derivation instead of needing one, and lands
on a section every session already reads. They declined credit for the asymmetry itself,
saying they noticed three instances and that *"a wrong positive fails loudly because the
world objects; a wrong negative has nothing downstream depending on it"* was mine, named
back at them in a usable form. Recorded their way.

**Filed late and deliberately.** Both of us held it rather than write it: each had told
our own operator the correspondence had reached falling marginal value, and filing
unilaterally would have been deciding an attention budget already declined. It exists
because one operator said continue — not because two sessions agreed it was good, which
is the shared-blind-spot condition this ledger keeps refusing to treat as corroboration.

**WHERE THIS DOES NOT REACH — contributed by `e5691fad`, who filed their own entry as a
SIBLING rather than a member and gave the reason.** `bug-fix-session-log:F-172` is a count
derived from a real command and published under a label naming a different **scope**. The
instrument *was* run and returned a **correct** answer — to a different question. There is
no absence to probe for, and the opposite-direction query surfaces nothing, because
`origin/experiments..HEAD` answered its own question perfectly.

And the asymmetry above does not transfer: **a mislabelled count is a wrong POSITIVE that
still fails silently, because the world has no opinion about which noun you attached to a
true number.** The coupling argument says a wrong positive fails loudly *when something
downstream uses the thing* — a label has no downstream. So absence-probing is the remedy
for this entry and **naming the window** is the remedy for that one, and neither
substitutes. That boundary is what justifies both entries existing rather than one
swallowing the other; `F-7` is the nearer neighbour of theirs, not this.

**AND THIS ENTRY'S OWN TOKEN IS AN INSTANCE OF THE ADJACENT DEFECT** — raised by
`e5691fad` against this entry, which is the right place to raise it. Re-derived here:
`## F-9` is defined in **14** ledgers under `docs/trackers/`; `## F-8` in **16**; `## F-7`
in **18**. I have cited all three **bare** — in commit messages, to three peer sessions,
and to my operator. Every one of those resolves to 14–18 candidates with nothing keeping
them apart: `IC-6`'s no-disambiguator half, one prefix over many ledgers. `e5691fad`
resolved this entry only from the SHA I happened to include; without `4006e9c6` they had
14 candidates and no way to choose, and they cited it as
`embedder-stack-ops-session-log:F-9`.

**Qualify the prefix or give the SHA.** The parallel is exact and unflattering: an entry
whose subject is *a true value published under an under-specified name* was itself
published under one.

## F-10 — Renaming a section heading with `replace` deleted its body; the shrink guard is whole-file and cannot see a single-section wipe

**Valid:** dated 2026-09-17

**Observed:** Renaming a plan section's heading from `**NOT STARTED**` to
`**LANDED**` via `doc(action="update", patch={body_edits: [{heading: "<old>",
action: "replace", content: "### <new heading line>\n"}]})` **deleted the
section's 34-line body.** The file went 442 → 428 lines. No error, no warning,
no `replaced_subsections` notice — the call returned `{"updated": true}`.

**Cause:** `replace`'s `content` is the section's new body, and a body whose
FIRST LINE parses as a heading is taken as the replacement heading rather than
placed under the existing one. So content that is *only* a heading line yields a
section that is only a heading line. This is `CLAUDE.md` § *Parsers Over a
Namespace* holding exactly as written — *"content whose first line looks like a
heading deleting the heading it was replacing"* — which I had read in this same
session, three hours earlier, in the reload payload.

**Cost:** recovered from `git show HEAD:<path>` only because the section had
been committed 40 minutes earlier as part of `70ec79d5`. Had the plan been
uncommitted — the normal state of a plan being written — the 34 lines were gone,
and the catalog is not in git either.

**Why no guard fired:** the body-shrink guard compares **whole-file** totals.
A 34-line deletion inside a 442-line file is 7.7%, nowhere near the 50%
threshold, so the guard is structurally unable to see a single-section wipe. It
is an aggregate assertion about a per-member claim — § *Testing Discipline*'s
scope law, in a guard rather than a test.

**What to do instead:** there is no rename action. To change a heading, `replace`
with the new heading line **followed by the full existing body**, which means
reading the section first. `doc(get, id=…, heading=…)` then paste. The two-step
is not optional and nothing in the tool's surface says so.

**Rests on:** `doc` tool behaviour at `6d5cc540`; the shrink-guard threshold
documented in `get_guide("librarian")` § *The shrink guard*.

## W-5 — A gate red owned by a live peer's uncommitted markdown — attributed positively, not by adjacency, and gone on re-run

**Valid:** dated 2026-09-17

**Observed:** `./scripts/gate.sh` came back `FMT=0 CLIPPY=0 LEAN=101 DEFAULT=101`.
The single failure was `tests/doc_tool_refs.rs::a_documented_call_names_a_live_tool`,
reporting `src/prompts/guides/librarian.md:199` — *"`link_scan(…)` names no
registered tool."* My diff that session was `src/hardware.rs`,
`src/tools/onboarding.rs`, `src/retrieval/config.rs` and five docs. Nothing I
touched could reach that test.

**What the pattern bought:** two checks, neither of which is "read my own diff":

1. `git show HEAD:src/prompts/guides/librarian.md | grep -c "link_scan(write=true)"`
   → **0**; working tree → **1**. The offending text exists only uncommitted.
2. `python3 scripts/file-provenance.py src/prompts/guides/librarian.md src/hardware.rs`
   → `PEER` / `MINE`, naming sessionId `a3bf229c-…`, name
   `rekey-prefix-entry-id-namespace`, profile `.claude-kat`, **[LIVE]**, with the
   socket to reach it.

**Counterfactual.** The failing line is prose about `link_scan`, sitting in a
guide, one plausible edit away from "fix the stale tool name". Doing that would
have rewritten a live peer's uncommitted markdown mid-edit — the defect
`fmt-mine.sh` exists to prevent, arrived at through a different door. The other
branch is cheaper but still wrong: reading my own 350-line diff for a cause that
was never in it.

**And the red cleared on its own.** A `--no-fail-fast` re-run ~12 minutes later
was **green, zero failures**, and the offending text was gone from the working
tree (the peer's file now staged). So the window was minutes, and CLAUDE.md's
`cli_doc` prescription generalises past `cli_doc`: *re-run before reading your
own diff* — the second run is both the cheapest discriminator and, here, the
whole repair.

**The generalisation worth keeping:** the shared-tree trap CLAUDE.md documents is
stated for a *binary* (`target/debug/codescout` left librarian-less by a lean
lane). This instance had no binary in it. Any test that reads a **working-tree
file** a peer is editing has the same shape — `doc_tool_refs`, `audit_doc_refs`,
`issue_clusters`, `committed_paths` and `claude_md_*` all scan tracked files, so
the population is larger than the binary case and nothing marks it.

**Rests on:** gate run at 2026-09-17T12:2x; re-run at 12:4x, both this session
(`458a8a26`). Attribution re-derived at use, per § *Reaching a Peer Session*.

## W-6 — A differential test for chunk-budget wiring surfaced an unrelated silent-fallback bug in project.toml parse-error handling

**Valid:** dated 2026-09-18

**Observed:** Writing a differential test for `resolved_chunk_budget` (Task 8,
2026-09-18), the first fixture — `[embeddings]\nmodel = "..."\n` with no `[project]`
table — produced the SAME budget (652) for two roots configured with two
DIFFERENT models. The assertion `assert_ne!` **caught it** (red, not a silent
false-pass), but the cause was one layer removed from what the assertion named:
`ProjectSection.name` is required, so `ProjectConfig::load_or_default` returned
`Err("missing field project")` for both fixtures, and
`resolve_embed_fields_with`'s `root.and_then(|r| ...load_or_default(r).ok())`
turned that into `None` → both roots silently fell through to the built-in
default model.

**Cost:** one probe cycle (a throwaway `cargo run --example` reading
`ProjectConfig::load_or_default` and `RetrievalConfig::from_env_and_project`
directly) to separate "my test fixture is wrong" from "the function under test
is wrong" — resolved in favour of the fixture, but only after checking, not by
assuming. Filed the masking defect itself as `bcb1d662862dbfc9` (archived; scoped out of
Task 8, fixed independently 2026-09-19 in `e635d4dab44a4b`) rather than fixing
it inline, since it's a separate, pre-existing
call site from the sibling bug `f73130523241a666` already fixed for the
workspace-pin memory-read path.

**Why this is a WIN, not just a friction:** the differential-assertion
discipline (two configured values, one `assert_ne!`, established this session
for the hardware-ranking work) is what surfaced a REAL, previously-unfiled bug
as a side effect of testing something else entirely. A single-value assertion
("budget == 1305") would have needed the SAME fix to the fixture but would have
just read as "test passed" once corrected — nothing would have pointed at the
`.ok()` swallow at all.

**Rests on:** `src/tools/memory/tests.rs::resolved_chunk_budget_reflects_the_projects_configured_model`;
bug `bcb1d662862dbfc9` (fixed); the archived sibling `f73130523241a666`.

## F-11 — temp_env's lock only protects its participants — an unguarded ambient-env reader raced two new tests and red the real gate

**Valid:** dated 2026-09-18

**Observed:** The first gate run for Task 8 (`workspace(status)` reporting the
resolved embedding settings) red on `status_reports_the_live_backend_and_what_is_compiled_in`
— a test I hadn't touched's logic, only its ground-truth derivation (fixed a
separate staleness earlier in the same edit). `left: "remote-http" right:
"local-onnx"`: its own real response said remote-http; its own independently-
computed ground truth said local-onnx.

**Cause:** two NEW tests I added (`project_status_embeddings_model_reflects_an_
env_override_not_project_toml`, using `temp_env::async_with_vars` to set
`CODESCOUT_EMBEDDING_MODEL`) ran CONCURRENTLY with the pre-existing test, which
reads the SAME ambient env directly (`embedding_env::read`) with NO
serialization at all. `temp_env`'s mutex only protects its own participants —
an untagged reader in the SAME test binary sees the transient value mid-flight.
This is the EXACT hazard `tests/retrieval_unit.rs`'s own header comment already
names for `set_var` vs `temp_env`, generalized one level: it also applies to
`temp_env` (a participant) vs a plain `std::env::var`/`embedding_env::read`
call (a non-participant) — the lock only ever protects who takes it.

**Fix:** `#[serial_test::serial]` (bare, this crate's existing default group —
no new group name introduced) on the two new tests AND the three pre-existing
tests in `src/tools/config/tests.rs` that read this same ambient env as ground
truth. Verified by running the affected module 5× consecutively post-fix
(all green) and then the real full gate (green).

**Why this counts as a WIN too, not purely friction:** the race was caught by
the ACTUAL GATE, not merely inferred — "run the reproduction" applied to my own
new code, not just to a bug report. And fixing it surfaced a second, unrelated
staleness in the same test (`status_reports_the_live_backend_...`'s own
ground-truth derivation only checked the two PRE-Task-5b deprecated env names,
missing the canonical `CODESCOUT_EMBEDDING_*` family entirely) — fixed
alongside, using the shared `embedding_env::read` utility rather than
re-hardcoding the (now three-name) list a second time.

**Rests on:** `src/tools/config/tests.rs`; `tests/retrieval_unit.rs`'s own
`set_var`-vs-`temp_env` comment, generalized.

## Template for new entries

<!-- New F-N / W-N entries land above this line. This heading is the anchor:

     doc(action="append_entry", id="<artifact id>", id_prefix="F",
              anchor_heading="## Template for new entries",
              title="<one-line title>", body="**Observed:** ...")

     The server allocates the id, writes `## F-N — <title>` at the ledger's
     own level, records the high-water mark and stamps `**Valid:** dated
     <today>` — one write. Then add the Index / Wins Index row with the id
     it returned. Do not hand-allocate; do not pre-write the row. -->
