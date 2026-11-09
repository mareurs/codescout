"""Sequential runner for the setup A/B study: one Claude Code run at a time, never a retry.

    run_batch(plan, budget, data, dry_run)       plan: [RunSpec], budget: Budget, data: a dir
    make_order(tasks, models, seed)              per task and model: bare x2, cs x1, full x1,
                                                 the arms of each (task, model) block shuffled
    stage1_order(seed)                           spec section 7 Stage 1: 2 development tasks x
                                                 3 arms x 2 models, one run each = 12 specs

SPEND SAFETY. `run_batch(..., dry_run=False, registration=<file>)` refuses to start (Refused,
before any docker call) unless the registration file is a regular JSON file whose
`registration_sha256` is the digest of the rest of it and which registers the plan's hash
(load_registration), and `<data>/GO.json` is a regular file holding `cap_usd`, `per_call_usd`,
`reserve_usd` and `spent_usd` equal to the batch's Budget, `plan_sha256` equal to
`plan_sha256(plan)`, `registration_sha256` equal to the registration's, and
`authorized_by: "operator"`. The file is the operator's (Task 10); nothing in this module writes
it. STOPS LATCH: every batch ends with a `stop` event in `<data>/batch.jsonl` carrying a
`stopId`; when the latest one is not "plan complete" (or a batch never wrote its stop), the next
batch is refused until GO.json `acknowledged_stops` lists that id. `dry_run=True` touches no
docker object, no CLI and no token: it writes `<data>/dry-run.json` (the specs, the plan hash
and which reps table the plan follows) and returns.

RUNBOOK (final review I-5 ruling 2026-10-29; Task 12, 2026-10-30). Stage 3 (pilot and
extension, in either order) runs in ONE data dir, setup-ab-data/study, the one Stage 1 ran in:
its journal holds Stage 1's rows, and spent_usd() adds every journal row, so Stage 1's spend
is counted there, once. `--spent` is the spend OUTSIDE this journal: the archived pilot
attempt 1, i.e. the registration's `carried_spend_usd`, and GO.json `spent_usd` == `--spent` ==
`carried_spend_usd` (check_go binds the first pair; run_batch refuses any other `--spent` under
a Stage 2 registration). Admission: journal spend + carried + reserve + per-call <= cap. Budget:
`--cap 200 --per-call 5 --reserve 5` (STAGE3_*). The printed `run.py run` commands of
freeze_stage2 carry all of it. A fresh data dir would forget the journal's spend: run_batch
refuses any data dir other than the registered run_data. Before the second part, move the
first part's GO.json aside (the `start` event keeps its budget and plan hash) and write the
second's.

JOURNAL. `<data>/rows.jsonl`, append-only, every line fsynced. A run writes a `pending` row
BEFORE the CLI call and its final row after; the latest row of a runId wins. At batch start every
runId whose latest row is still `pending` (the runner was killed) gets an `unknown-spend` row. A
runId that has any row is never run again by any batch: there is no automatic retry after any
outcome. Row fields are exactly ROW_FIELDS: the Task 8 brief's list plus ruling R6's
bashCommands, editedFiles, inputTokens and outputTokens, plus `stage` ("dev" for the spec's
seven development tasks, "main" otherwise) and `capHit` (final review I-6: the result record
says `--max-budget-usd` ended the run; None when there is no result record, and for Stage 1's
rows, written before the field). Final outcomes are OUTCOMES. ONE DATA DIR PER STUDY:
the lock, the ledger and the spend total are per data dir (deferred M3).

A spec that failed BEFORE its pending row (bad bundle, uid check, a foreign name) has a
`runs/<runId>/` dir with run-record.json (`error` ... "before the CLI call") and no row; the next
batch stops at its `mkdir`. Recovery is by hand: confirm no row and no `call` event for that
runId, move the dir aside (`runs/<runId>.failed-<date>`), then acknowledge the stop.

SPEND. spent = budget.spent_usd (spend outside this journal) + for the latest row of every runId
its `costUsd`, or, when the cost is unknown (pending, unknown-spend, a run with no result
record), the `per_call_usd` of the batch that made that call (its `call` event's `start` event;
the largest per_call of any start event when there is none): conservative. ADMISSION: a call starts only when
spent + reserve_usd + per_call_usd <= cap_usd (per_call_usd is the call's `--max-budget-usd`, its
worst case; the reserve is held for the next run, spec section 7). `cap_usd` may not exceed the
study's $200 (TOTAL_CAP_USD).

OUTCOMES, decided in this order for each run (the first that applies):
  plan-limit     the vendored plan-limit.mjs (through node) finds a refusal in the result record,
                 stdout, stderr or the session transcript. Neutral: not scored, not a failure of
                 the arm. STOPS the batch. (Ruling M2: on a stream-json stdout the stdout channel
                 is inert, as the detector anchors at index 0 with a 600-char cap; only the
                 envelope and transcript channels detect. With no retry, a false positive, an
                 answer that starts like a limit message, loses that one paid run: accepted.)
  infra-oom      the agent container has State.OOMKilled=true, or its cgroup memory.events
                 counts an oom_kill (fix round 2, N-5: a killed rust-analyzer or rustc is not
                 PID 1), or the grader's container was OOM-killed.
                 Not scored, not a failure. The batch goes on when the cost is known. On OOM
                 and on a CLI timeout the container is `docker kill`ed at once, before usage
                 and plan-limit are read, so a runaway CLI cannot keep spending.
  infra-other    no result record (timeout, kill, crash), or no usable cost, or the run did no
                 model work (is_error with zero api time, tokens, cost and calls: a credential or
                 infrastructure failure), or the grader raised or reported infra or an anomaly
                 (grade_outcome). Not scored. EVERY infra-other STOPS the batch (spec section
                 7: stop on missing cost, credential error, grader contamination).
  invalid-arm    integrity.check is not ok, or a tool_use block is malformed. Charged, not scored.
  scored         graded by the task's grader on the image the agent ran in (main tasks).
  dev-scored     the same for a development task: never a scored outcome.
  unknown-spend  the CLI was called and the runner did not learn the cost (interruption).
After a graded row, EFFORT: the graded rows of each (task, model) must share one effectiveEffort
(a non-empty string). A different or unknown value marks that task+model invalid in
`<data>/task-validity.json` and STOPS the batch.

CONTAINERS. Every arm starts from bundle.json's `arm_image` through `tasks.bundle_run_image`
(the exec-only layer, Task 7 Ruling A), never `run_image`, with arms.docker_run_args (cap-drop
ALL, no-new-privileges); the agent must answer `id -u` with 1000 before the CLI starts. The
plugin dirs and the full arm's config come from the registration's frozen copy (final review
I-7, frozen.py): run_batch re-hashes every frozen tree before the lock and refuses on any
difference, and hands arms a manifest that names only the frozen paths. The CLI
argv is arms.arm_invocation's, with `--output-format json` replaced by `stream-json --verbose
--include-hook-events` (the input integrity.check reads, as live_control.step_r10 built it) and
`--max-budget-usd` set to `budget.per_call_usd`; no `--effort` (spec: CLI default effort). The env
file adds the token, HTTPS_PROXY (the runner's proxy) and NO_PROXY/no_proxy covering 127.0.0.1,
localhost and every study host of the run (Task 4 carry); it is deleted once the container runs.
Patches exclude `.codescout/`, `.buddy/` and `.superpowers/` at any depth (Task 4 ruling R2-F3);
the excluded paths are listed in the run's run-record.json.

OWNERSHIP. The runner creates only `sab-ab-*` objects plus the per-run Qdrant objects stack.py
names (`sab-qdrant-ab<key>`, `sab-qv-ab<key>`, `sab-copy-sab-qv-ab<key>`). Each name is written
to `<data>/owned.jsonl` BEFORE it is created, and a name that already exists is refused, never
adopted. Cleanup removes exactly the ledger's names (stack.run_down / stack._remove_exact, exact
names, never a prefix sweep and never stack.stack_down), so the background index job's
`sab-prep-*`, `sab-dense`, `sab-sparse`, `sab-qdrant`, the `sab-tpl-*` templates, `codescout-*`,
`pbr27-*` and `rae-*` are never touched.

Stdlib only. Never uses pilot's PREFIX/NETWORK/IMAGE/DATA/ORDER constants (ruling R1); token
accounting and plan-limit detection run the vendored copies under vendor/ (ruling R7,
PROVENANCE.json "vendored").
"""
import argparse
import hashlib
import json
import os
import random
import re
import subprocess
import time
import uuid
from pathlib import Path, PurePosixPath
from typing import Callable, NamedTuple
from urllib.parse import urlsplit

import arms
import evidence
import frozen
import integrity
import pilot
import stack
import tasks
from evidence import nonnegative

HERE = Path(__file__).resolve().parent
VENDOR = HERE / "vendor/claude-plugins/effort-steering/spikes"
PLAN_LIMIT_MJS = VENDOR / "plan-limit.mjs"
USAGE_MJS = VENDOR / "s5/usage.mjs"
PROXY_TEMPLATE = VENDOR / "s5/images/tinyproxy.conf.tmpl"

MODELS = ("claude-sonnet-5-5", "claude-opus-5-5")
STAGE3_REPS = (("bare", 2), ("cs", 1), ("full", 1))
STAGE1_REPS = (("bare", 1), ("cs", 1), ("full", 1))
# Provisional: spec section 2 lists the development tasks; Task 10 confirms the two (and builds
# their bundles) before the operator go. ripgrep-2576 is the Codex pilot's task, ruff-15309
# the development image of ruling R4.
STAGE1_DEV_TASKS = ("burntsushi__ripgrep-2576", "astral-sh__ruff-15309")
PLAN_COMPLETE = "plan complete"
TOTAL_CAP_USD = 200.0
STAGE1_CAP_USD = 25.0
# Final review I-5/I-6 rulings (2026-10-29): Stage 3 (pilot and extension) runs in ONE data dir,
# setup-ab-data/study, whose journal already holds Stage 1's spend; the cap is the study's whole
# $200, the per-call cap $5.00 and the reserve one call's worst case.
STAGE3_CAP_USD = TOTAL_CAP_USD
STAGE3_PER_CALL_USD = 5.00
STAGE3_RESERVE_USD = STAGE3_PER_CALL_USD
RUN_TIMEOUT_S = 3600  # Task 12 (operator 2026-10-30): pilot attempt 1 lost a run at 1800 s
AGENT_UID = "1000"

# Fix round 1 (M5): a graded run of a development task is `dev-scored`, never `scored` (spec
# section 2 and 7: development outcomes are never scored), so an analysis that counts `scored`
# cannot include one even if it ignores the row's `stage`.
OUTCOMES = ("scored", "dev-scored", "invalid-arm", "plan-limit", "infra-oom", "infra-other",
            "unknown-spend")
GRADED = ("scored", "dev-scored")
PENDING = "pending"  # journal-only state; never a final outcome
# The brief's 14 fields, R6's 4, `stage` (fix round 1, M5), `capHit` (final review I-6) and
# `oomKills`/`memoryPeakBytes` (fix round 2, N-5: the agent container cgroup's memory.events
# oom_kill count and memory.peak, read before the container is removed).
ROW_FIELDS = ("runId", "taskId", "model", "arm", "rep", "sessionId", "costUsd", "wallMs",
              "numTurns", "usage", "effectiveEffort", "grade", "integrity", "outcome",
              "bashCommands", "editedFiles", "inputTokens", "outputTokens", "stage", "capHit",
              "oomKills", "memoryPeakBytes")
LATER_ROW_FIELDS = ("capHit", "oomKills", "memoryPeakBytes")
# Stage 1's rows (development tasks only) were written before LATER_ROW_FIELDS existed;
# read_rows gives such a row None (unknown) for each, and only for stage "dev".
STAGE1_ROW_FIELDS = tuple(f for f in ROW_FIELDS if f not in LATER_ROW_FIELDS)
# Final review I-6: the CLI's result record when `--max-budget-usd` stopped the run (CLI
# 2.1.309: a `budget_exhausted` stop becomes {"type": "result", "subtype":
# "error_max_budget_usd", "is_error": true, ...}; read from the CLI bundle, as no Stage 1 run
# hit the cap: all 12 results are subtype "success", terminal_reason "completed").
CAP_HIT_SUBTYPE = "error_max_budget_usd"
CAP_HIT_TERMINAL_REASON = "budget_exhausted"
# Spec section 2: the seven Codex development tasks; plumbing and calibration only, never scored.
DEVELOPMENT_TASKS = ("burntsushi__ripgrep-2576", "tokio-rs__axum-734", "tokio-rs__axum-1119",
                     "burntsushi__ripgrep-2209", "tokio-rs__tokio-6551", "astral-sh__ruff-15443",
                     "astral-sh__ruff-15309")

PATCH_EXCLUDED_DIRS = (".codescout", ".buddy", ".superpowers")
PATCH_PATHSPEC = (".", *(f":(exclude,glob)**/{d}/**" for d in PATCH_EXCLUDED_DIRS))

# Runner-owned names (study namespace `sab-`, runner sub-namespace `sab-ab-`).
NET = "sab-ab-net"
PROXY = "sab-ab-proxy"
PROXY_PORT = 8888
DENSE = "sab-ab-dense"
SPARSE = "sab-ab-sparse"
NETPROBE = "sab-ab-netprobe"
AGENT_PREFIX = "sab-ab-agent-"
QDRANT_RUN_PREFIX = "ab"  # stack.qdrant_for_run(run=ab<key>) -> sab-qdrant-ab<key>, sab-qv-ab<key>
PROXY_IMAGE = "s5/proxy:latest"
PROBE_IMAGE = "python:3.12-slim"
ALLOWED_HOST = "api.anthropic.com"
PROXY_CONF_MOUNT = "/etc/s5-proxy"

GO_FILE = "GO.json"
ROWS = "rows.jsonl"
OWNED = "owned.jsonl"
EVENTS = "batch.jsonl"
LOCK = "runner.lock"
VALIDITY = "task-validity.json"

_ID = re.compile(r"[A-Za-z0-9][A-Za-z0-9_.-]*")
_SUBNET = re.compile(r"\d{1,3}(\.\d{1,3}){3}/\d{1,2}")
_HOST = re.compile(r"(?=.{1,253}$)[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?(\.[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?)*")


class Refused(RuntimeError):
    """The runner will not start: no operator authorization, a foreign object, a held lock."""


class RunSpec(NamedTuple):
    task_id: str
    model: str
    arm: str
    rep: int


class Budget(NamedTuple):
    cap_usd: float
    spent_usd: float
    per_call_usd: float
    reserve_usd: float


class TaskInfo(NamedTuple):
    image: str                 # the verified arm image (tasks.bundle_run_image)
    prompt: str
    template: str              # the task's Qdrant template volume, sab-tpl-<id>
    grader: Callable           # grader(gdir) -> grade(patch, name) -> dict
    kind: str = ""             # the bundle's kind; `live` tasks run the bare arm only


# --- plan, budget, authorization -----------------------------------------------------------------

def make_order(tasks_, models, seed, reps=STAGE3_REPS):
    """Per task, then per model (in the given orders), one block of run specs whose arm order is
    shuffled by one random.Random(seed): deterministic for a seed (spec section 7: arm order per
    task randomised, to spread prompt-cache effects)."""
    if isinstance(seed, bool) or not isinstance(seed, int):
        raise ValueError(f"seed must be an int, got {seed!r}")
    tasks_, models = list(tasks_), list(models)
    if not tasks_ or len(set(tasks_)) != len(tasks_) or not models or len(set(models)) != len(models):
        raise ValueError("tasks and models must be non-empty and without duplicates")
    for arm, n in reps:
        if arm not in arms.ARMS or not isinstance(n, int) or n < 1:
            raise ValueError(f"bad reps entry {(arm, n)!r}")
    rng = random.Random(seed)
    order = []
    for task_id in tasks_:
        for model in models:
            block = [RunSpec(task_id, model, arm, rep) for arm, n in reps for rep in range(n)]
            rng.shuffle(block)
            order += block
    return check_plan(order)


def stage1_order(seed, dev_tasks=STAGE1_DEV_TASKS):
    """Stage 1 (spec section 7): two development tasks x 3 arms x 2 models, one run each."""
    return make_order(dev_tasks, MODELS, seed, STAGE1_REPS)


def check_plan(plan):
    out, seen = [], set()
    for s in plan:
        s = RunSpec(*s)
        if not (isinstance(s.task_id, str) and _ID.fullmatch(s.task_id)):
            raise ValueError(f"bad task id {s.task_id!r}")
        if s.model not in MODELS:
            raise ValueError(f"model {s.model!r} is not one of {MODELS}")
        if s.arm not in arms.ARMS:
            raise ValueError(f"arm {s.arm!r} is not one of {arms.ARMS}")
        if isinstance(s.rep, bool) or not isinstance(s.rep, int) or s.rep < 0:
            raise ValueError(f"rep must be an int >= 0, got {s.rep!r}")
        rid = run_id(s)
        if rid in seen:
            raise ValueError(f"duplicate run spec {rid}")
        seen.add(rid)
        out.append(s)
    if not out:
        raise ValueError("empty plan")
    return out


def plan_sha256(plan):
    """sha256 of the canonical JSON of the ordered spec list: [[task, model, arm, rep], ...]."""
    canon = json.dumps([list(RunSpec(*s)) for s in plan], separators=(",", ":"), ensure_ascii=True)
    return hashlib.sha256(canon.encode()).hexdigest()


def run_id(spec):
    s = RunSpec(*spec)
    return f"{s.task_id}.{s.model}.{s.arm}.{s.rep}"


def _key(rid):
    return hashlib.sha256(rid.encode()).hexdigest()[:12]


def check_budget(budget):
    b = Budget(*budget)
    for field in b._fields:
        nonnegative(getattr(b, field), field)
    if b.cap_usd > TOTAL_CAP_USD:
        raise ValueError(f"cap_usd {b.cap_usd} exceeds the study's ${TOTAL_CAP_USD:.0f} total")
    if b.per_call_usd <= 0:
        raise ValueError("per_call_usd must be > 0 (it is the call's --max-budget-usd)")
    return b


def admit(budget, spent):
    """A call may start only when its worst case and the held reserve still fit under the cap."""
    return spent + budget.reserve_usd + budget.per_call_usd <= budget.cap_usd


def check_go(data, plan, budget, registration_sha256):
    """The operator's authorization, or Refused. Read only; the runner never writes GO.json.

    Binds cap_usd, per_call_usd and reserve_usd (fix round 1, I1b: the per-call cap is what an
    unknown cost is charged at), spent_usd (final review I-5; Task 12: the spend outside this
    data dir's journal, which under a Stage 2 registration is its `carried_spend_usd`), the plan
    hash and `registration_sha256` (final review
    I-5 / T10b M6: the registration the batch runs under). `acknowledged_stops`, optional, is the
    list of stop ids (batch.jsonl) the operator has inspected; see check_latch."""
    path = Path(data) / GO_FILE
    if path.is_symlink() or not path.is_file():
        raise Refused(f"{path} is missing (or not a regular file): spending needs the "
                      f"operator's go (Task 10); the runner never creates it")
    try:
        go = json.loads(path.read_text())
    except (OSError, ValueError) as e:
        raise Refused(f"{path} is unreadable: {e}")
    if not isinstance(go, dict) or go.get("authorized_by") != "operator":
        raise Refused(f"{path} is not authorized_by the operator")
    for field in ("cap_usd", "per_call_usd", "reserve_usd", "spent_usd"):
        value = go.get(field)
        if (isinstance(value, bool) or not isinstance(value, (int, float))
                or float(value) != float(getattr(budget, field))):
            raise Refused(f"{path} authorizes {field} {value!r}, the batch has "
                          f"{getattr(budget, field)}")
    if go.get("plan_sha256") != plan_sha256(plan):
        raise Refused(f"{path} authorizes plan {go.get('plan_sha256')!r}, the batch's plan is "
                      f"{plan_sha256(plan)}")
    if not registration_sha256 or go.get("registration_sha256") != registration_sha256:
        raise Refused(f"{path} authorizes registration {go.get('registration_sha256')!r}, the "
                      f"batch runs under {registration_sha256!r}")
    acks = go.get("acknowledged_stops", [])
    if not isinstance(acks, list) or not all(isinstance(a, str) for a in acks):
        raise Refused(f"{path}: acknowledged_stops must be a list of stop ids")
    return go


def registered_arm_profile(reg):
    """The arm profile a registration pins: its `arm_profile` key, "v1" for a registration that has
    none (every one frozen before profiles existed), `Refused` for a name `arms.PROFILES` lacks.
    The key is part of the hashed body, so a swapped profile no longer matches the digest."""
    profile = reg.get("arm_profile", "v1")
    if profile not in arms.PROFILES:
        raise Refused(f"the registration pins arm profile {profile!r}; expected one of {arms.PROFILES}")
    return profile



def registration_digest(body):
    """sha256 of the canonical JSON of a registration body (freeze_stage1._digest's form)."""
    return hashlib.sha256(json.dumps(body, sort_keys=True, separators=(",", ":"),
                                     ensure_ascii=True).encode()).hexdigest()


def registered_plan_hashes(reg):
    """Every plan hash a registration registers: a top-level `plan_sha256` (Stage 1's form) and
    `plans.<part>.plan_sha256` (the Stage 3 parts, stage3.registration_pieces)."""
    out = set()
    if isinstance(reg.get("plan_sha256"), str):
        out.add(reg["plan_sha256"])
    for part in (reg.get("plans") or {}).values():
        if isinstance(part, dict) and isinstance(part.get("plan_sha256"), str):
            out.add(part["plan_sha256"])
    return out


def load_registration(path, plan):
    """The registration a batch runs under, or Refused: a regular JSON file whose
    `registration_sha256` is the digest of the rest of it and which registers `plan`."""
    path = Path(path)
    if path.is_symlink() or not path.is_file():
        raise Refused(f"registration {path} is missing (or not a regular file)")
    try:
        reg = json.loads(path.read_text())
    except (OSError, ValueError) as e:
        raise Refused(f"registration {path} is unreadable: {e}")
    if not isinstance(reg, dict):
        raise Refused(f"registration {path} is not a JSON object")
    body = {k: v for k, v in reg.items() if k != "registration_sha256"}
    if reg.get("registration_sha256") != registration_digest(body):
        raise Refused(f"registration {path}: registration_sha256 does not match its content "
                      f"(edited after the freeze?)")
    if plan_sha256(plan) not in registered_plan_hashes(reg):
        raise Refused(f"registration {path} does not register the batch's plan "
                      f"{plan_sha256(plan)}")
    return reg
def registered_budget(reg, plan):
    """The budget a registration records for `plan`: Stage 1's top-level `budget` when its
    `plan_sha256` is the plan's, else the `budget` of the `plans.<part>` whose hash it is."""
    h = plan_sha256(plan)
    if reg.get("plan_sha256") == h:
        return reg.get("budget")
    for part in (reg.get("plans") or {}).values():
        if isinstance(part, dict) and part.get("plan_sha256") == h:
            return part.get("budget")
    return None


def check_registered_budget(reg, plan, budget):
    """Fix round 2 (N-2): the batch runs under the cap, per-call cap and reserve its registered
    part records, or Refused (the per-call cap is outcome-affecting, I-6)."""
    want = registered_budget(reg, plan)
    if not isinstance(want, dict):
        raise Refused("the registration records no budget for the batch's plan")
    for field in ("cap_usd", "per_call_usd", "reserve_usd"):
        value = want.get(field)
        if (isinstance(value, bool) or not isinstance(value, (int, float))
                or float(value) != float(getattr(budget, field))):
            raise Refused(f"the registration's {field} for this plan is {value!r}, the batch "
                          f"has {getattr(budget, field)}")


def last_stop(events):
    """(stop id, reason) of the latest batch that did not end in PLAN_COMPLETE, else None.

    A batch whose `start` has no later `stop` (the runner was killed) is latched as
    `unterminated-<start seq>-<hash of the start event>` (deterministic: the same journal always
    names the same id, a restored or truncated journal names another)."""
    starts = [e for e in events if e.get("event") == "start"]
    stops = [e for e in events if e.get("event") == "stop"]
    if starts and (not stops or starts[-1]["seq"] > stops[-1]["seq"]):
        digest = hashlib.sha256(json.dumps(starts[-1], sort_keys=True).encode()).hexdigest()[:8]
        return (f"unterminated-{starts[-1]['seq']}-{digest}",
                "the batch never wrote its stop (killed)")
    if stops and stops[-1].get("reason") != PLAN_COMPLETE:
        return stops[-1]["stopId"], stops[-1].get("reason")
    return None


def check_latch(data, go):
    """Fix round 1 (I3): a stop is latched. A new batch is refused until the operator lists the
    latest stop's id in GO.json `acknowledged_stops`. Every stop reason other than PLAN_COMPLETE
    latches, admission included (re-running the same budget would only stop again)."""
    latched = last_stop(read_events(data))
    if latched and latched[0] not in go.get("acknowledged_stops", []):
        raise Refused(f"the previous batch stopped ({latched[1]}, stop id {latched[0]}); inspect "
                      f"it, then list {latched[0]!r} in {GO_FILE} acknowledged_stops")
    return latched


# --- journal -------------------------------------------------------------------------------------

def _fsync_dir(d):
    fd = os.open(d, os.O_RDONLY)
    try:
        os.fsync(fd)
    finally:
        os.close(fd)


def _append(path, obj):
    path = Path(path)
    new = not path.exists()
    with open(path, "a", encoding="utf-8") as f:
        f.write(json.dumps(obj, allow_nan=False) + "\n")
        f.flush()
        os.fsync(f.fileno())
    if new:
        _fsync_dir(path.parent)


def _event(data, event, **fields):
    """Append one batch.jsonl event with its sequence number (1-based line number, stable: the
    file is append-only); a `stop` also gets `stopId` = `stop-<seq>-<uuid8>`, stored in the event
    (the uuid keeps a truncated or restored journal from reusing a stale acknowledged id).
    Returns the event."""
    seq = len(read_events(data)) + 1
    rec = {"event": event, "seq": seq, "at": time.strftime("%Y-%m-%dT%H:%M:%S%z"), **fields}
    if event == "stop":
        rec["stopId"] = f"stop-{seq}-{uuid.uuid4().hex[:8]}"
    _append(Path(data) / EVENTS, rec)
    return rec


def read_events(data):
    path = Path(data) / EVENTS
    if not path.exists():
        return []
    events = []
    for n, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        if not line.strip():
            continue
        e = json.loads(line)
        if not isinstance(e, dict) or isinstance(e.get("seq"), bool) or not isinstance(
                e.get("seq"), int):
            raise Refused(f"{path}:{n}: journal line has no integer `seq` (old format?); the "
                          f"latch cannot be read; inspect it before any spend")
        if e.get("event") == "stop" and not isinstance(e.get("stopId"), str):
            raise Refused(f"{path}:{n}: stop event has no `stopId` (old format?); inspect it "
                          f"before any spend")
        events.append(e)
    return events


def check_journal(data):
    """batch.jsonl is the one point of trust for the latch and the per-call charge. Refuse (before
    any side effect) when rows.jsonl has rows but batch.jsonl does not exist or has no `start`
    event, or when a non-pending row's runId has no `call` event (every row this code writes has
    one: a deleted or truncated journal would clear the latch and under-count the spend)."""
    rows = read_rows(data)
    if not rows:
        return
    events = read_events(data)
    if not (Path(data) / EVENTS).exists():
        raise Refused(f"{Path(data) / ROWS} has {len(rows)} row(s) but {Path(data) / EVENTS} does "
                      f"not exist; the journal was deleted or never written; inspect before any "
                      f"spend")
    if not any(e.get("event") == "start" for e in events):
        raise Refused(f"{Path(data) / ROWS} has rows but {EVENTS} has no `start` event; the "
                      f"journal was truncated; inspect before any spend")
    called = {e.get("runId") for e in events if e.get("event") == "call"}
    for row in rows:
        if row["outcome"] != PENDING and row["runId"] not in called:
            raise Refused(f"row {row['runId']} ({row['outcome']}) has no `call` event in "
                          f"{EVENTS}; the journal does not cover it; inspect before any spend")


def stage_of(task_id):
    return "dev" if task_id in DEVELOPMENT_TASKS else "main"


def make_row(spec, session, **fields):
    s = RunSpec(*spec)
    unknown = set(fields) - set(ROW_FIELDS)
    if unknown:
        raise ValueError(f"not row fields: {sorted(unknown)}")
    row = dict.fromkeys(ROW_FIELDS)
    row.update(runId=run_id(s), taskId=s.task_id, model=s.model, arm=s.arm, rep=s.rep,
               sessionId=session, stage=stage_of(s.task_id))
    row.update(fields)
    if row["outcome"] not in OUTCOMES + (PENDING,):
        raise ValueError(f"bad outcome {row['outcome']!r}")
    if row["outcome"] == "scored" and row["stage"] == "dev":
        raise ValueError(f"{row['runId']}: a development task is never `scored` (dev-scored)")
    return row


def read_rows(data):
    """Every journal row in order; a torn or foreign line raises (inspect before any spend).
    A Stage 1 row (stage "dev", written before `capHit` existed) gets capHit None."""
    path = Path(data) / ROWS
    if not path.exists():
        return []
    rows = []
    for n, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        if not line.strip():
            continue
        try:
            row = json.loads(line)
        except ValueError:
            raise ValueError(f"{path}:{n}: torn or malformed journal line; inspect it before "
                             f"any further spend")
        if (isinstance(row, dict) and set(row) == set(STAGE1_ROW_FIELDS)
                and row.get("stage") == "dev"):
            row = dict(row, **dict.fromkeys(LATER_ROW_FIELDS))
        if (not isinstance(row, dict) or set(row) != set(ROW_FIELDS)
                or row["outcome"] not in OUTCOMES + (PENDING,)):
            raise ValueError(f"{path}:{n}: not a runner row")
        rows.append(row)
    return rows


def latest_rows(rows):
    """{runId: its latest row}, in first-seen order."""
    out = {}
    for row in rows:
        out[row["runId"]] = row
    return out


def call_per_call(events, budget):
    """{runId: the per_call_usd of the batch that made its CLI call} and the fallback charge.

    Fix round 1 (I1a): an unknown cost is charged at the cap of the call that spent it, read from
    the `budget` of the `start` event preceding the run's `call` event in batch.jsonl. A run with
    no `call` event is charged the MAX per_call over every start event (and the current one)."""
    per_run, current = {}, None
    for e in events:
        if e.get("event") == "start":
            current = float(e["budget"]["per_call_usd"])
        elif e.get("event") == "call" and current is not None:
            per_run[e["runId"]] = current
    starts = [float(e["budget"]["per_call_usd"]) for e in events if e.get("event") == "start"]
    return per_run, max(starts + [budget.per_call_usd])


def spent_usd(rows, budget, events):
    """budget.spent_usd + the latest row's cost per runId; an unknown cost (pending,
    unknown-spend, no result) is charged at its own call's per_call (call_per_call)."""
    per_run, fallback = call_per_call(events, budget)
    total = budget.spent_usd
    for rid, row in latest_rows(rows).items():
        total += row["costUsd"] if row["costUsd"] is not None else per_run.get(rid, fallback)
    return total


def recover_pending(data):
    """A pending latest row means the runner died around a CLI call: append unknown-spend."""
    out = []
    for row in latest_rows(read_rows(data)).values():
        if row["outcome"] == PENDING:
            fixed = dict(row, outcome="unknown-spend", costUsd=None)
            _append(Path(data) / ROWS, fixed)
            out.append(fixed)
    return out


def effective_effort(usage):
    """The one effort the main transcript's counted assistant records carry, else None."""
    efforts = (usage or {}).get("efforts")
    if isinstance(efforts, list) and len(efforts) == 1 and isinstance(efforts[0], str) and efforts[0]:
        return efforts[0]
    return None


def task_validity(rows):
    """{"<task>|<model>": {valid, efforts, reason}} over the latest GRADED rows (scored and
    dev-scored). Valid iff every graded row of the task and model has the same non-empty
    effectiveEffort (an unknown effort cannot be shown equal). Rows of other outcomes are not
    compared."""
    groups = {}
    for row in latest_rows(rows).values():
        if row["outcome"] in GRADED:
            groups.setdefault((row["taskId"], row["model"]), []).append(row)
    out = {}
    for (task_id, model), rs in groups.items():
        values = [r["effectiveEffort"] for r in rs]
        ok = all(isinstance(v, str) and v for v in values) and len(set(values)) == 1
        out[f"{task_id}|{model}"] = {
            "taskId": task_id, "model": model, "valid": ok,
            "efforts": {r["runId"]: r["effectiveEffort"] for r in rs},
            "reason": None if ok else "effective effort differs or is unknown across the arms"}
    return out


# --- node: the vendored S5 readers ---------------------------------------------------------------

_PLAN_LIMIT_JS = ("import {detectPlanLimit} from %s; import {readFileSync} from 'node:fs';"
                  "console.log(JSON.stringify(detectPlanLimit(JSON.parse(readFileSync(0,'utf8')))));")
_USAGE_JS = ("import {readRunUsage} from %s; import {readFileSync} from 'node:fs';"
             "const a=JSON.parse(readFileSync(0,'utf8'));"
             "console.log(JSON.stringify(readRunUsage(a.config,a.session)));")


def _node(script, payload, timeout=120):
    p = subprocess.run(["node", "--input-type=module", "-e", script], input=json.dumps(payload),
                       capture_output=True, text=True, timeout=timeout)
    if p.returncode:
        raise RuntimeError(f"node failed (exit {p.returncode}): {p.stderr[-2000:]}")
    return json.loads(p.stdout)


def plan_limit(envelope=None, stdout="", stderr="", transcript=""):
    """The vendored plan-limit.mjs verdict (a hit dict) or None. Never re-implemented here."""
    return _node(_PLAN_LIMIT_JS % json.dumps(PLAN_LIMIT_MJS.as_uri()),
                 {"envelope": envelope, "stdout": stdout, "stderr": stderr,
                  "transcript": transcript})


def run_usage(config, session):
    """The vendored usage.mjs readRunUsage(config, session) (tokens, efforts, transcriptPath)."""
    return _node(_USAGE_JS % json.dumps(USAGE_MJS.as_uri()),
                 {"config": str(config), "session": session})


# --- stream --------------------------------------------------------------------------------------

def dedupe_tool_uses(lines):
    """(lines, duplicates, shape_errors). A tool_use block whose id an earlier block had is dropped
    (Task 6 carry: stream-json may repeat an assistant message); a block without a string id and
    name and an object input is kept and reported. Unparseable lines are kept verbatim, so
    integrity.check still reports them."""
    seen, dups, errors, out = set(), 0, [], []
    for n, line in enumerate(lines, 1):
        try:
            rec = json.loads(line)
        except ValueError:
            out.append(line)
            continue
        msg = rec.get("message") if isinstance(rec, dict) and rec.get("type") == "assistant" else None
        content = msg.get("content") if isinstance(msg, dict) else None
        if not isinstance(content, list):
            out.append(line)
            continue
        kept = []
        for block in content:
            if isinstance(block, dict) and block.get("type") == "tool_use":
                tid, name, inp = block.get("id"), block.get("name"), block.get("input")
                if not (isinstance(tid, str) and tid and isinstance(name, str) and name
                        and isinstance(inp, dict)):
                    errors.append(f"line {n}: tool_use without a string id/name or object input")
                elif tid in seen:
                    dups += 1
                    continue
                else:
                    seen.add(tid)
            kept.append(block)
        out.append(json.dumps(dict(rec, message=dict(msg, content=kept))))
    return out, dups, errors


def _records(lines):
    out = []
    for line in lines:
        try:
            rec = json.loads(line)
        except ValueError:
            continue
        if isinstance(rec, dict):
            out.append(rec)
    return out


def bash_commands(records):
    """input.command of every Bash tool_use (ruling R6), subagent calls included."""
    out = []
    for rec in records:
        msg = rec.get("message") if rec.get("type") == "assistant" else None
        for block in (msg.get("content") if isinstance(msg, dict) else None) or []:
            if (isinstance(block, dict) and block.get("type") == "tool_use"
                    and block.get("name") == "Bash" and isinstance(block.get("input"), dict)
                    and isinstance(block["input"].get("command"), str)):
                out.append(block["input"]["command"])
    return out


def _result_record(records):
    results = [r for r in records if r.get("type") == "result"]
    return results[-1] if results else None


def num_turns(records):
    """The run's turn count: `num_turns` summed over every result record (bug 02c3f2630dcd7ae8).

    The CLI writes one result record per segment, and a queued task-notification turn after the
    run adds a segment, so the last record alone counts only that turn. None when there is no
    result record, or when one of them has no integer count: a partial sum would be a wrong
    number under a whole-run name."""
    counts = [r.get("num_turns") for r in records if r.get("type") == "result"]
    if not counts or not all(type(c) is int for c in counts):
        return None
    return sum(counts)


def _cost(result):
    if not isinstance(result, dict):
        return None
    try:
        return nonnegative(result.get("total_cost_usd"), "total_cost_usd")
    except ValueError:
        return None
def cap_hit(result):
    """Final review I-6: True when the result record says `--max-budget-usd` ended the run
    (subtype error_max_budget_usd, or terminal_reason budget_exhausted), False for any other
    result record, None when there is none (unknown)."""
    if not isinstance(result, dict):
        return None
    return (result.get("subtype") == CAP_HIT_SUBTYPE
            or result.get("terminal_reason") == CAP_HIT_TERMINAL_REASON)


# --- container inputs ----------------------------------------------------------------------------

def cli_argv(args, per_call_usd):
    """arms.arm_invocation's argv with stream-json output and the per-call cap; flags are looked
    up before the `--` separator only, so the prompt can never be mistaken for one."""
    args = list(args)
    sep = args.index("--")
    flags = args[:sep]
    if "--effort" in flags:
        raise ValueError("--effort in an arm argv: every arm runs at the CLI default effort")
    if flags.count("--output-format") != 1 or flags.count("--max-budget-usd") != 1:
        raise ValueError("expected exactly one --output-format and one --max-budget-usd")
    i = flags.index("--output-format")
    if flags[i + 1] != "json":
        raise ValueError(f"expected arms' --output-format json, got {flags[i + 1]!r}")
    flags[i:i + 2] = ["--output-format", "stream-json", "--verbose", "--include-hook-events"]
    j = flags.index("--max-budget-usd")
    flags[j + 1] = repr(float(nonnegative(per_call_usd, "per_call_usd")))
    return flags + args[sep:]


def url_host(url):
    return urlsplit(url).hostname


def study_urls(key, retrieval_urls):
    """The codescout env URLs of run <key>: its own Qdrant plus the runner's dense/sparse."""
    urls = dict(retrieval_urls)
    urls.update(stack._urls("qdrant", f"sab-qdrant-{QDRANT_RUN_PREFIX}{key}"))
    return urls


def no_proxy(urls):
    hosts = sorted({url_host(v) for v in urls.values()})
    bad = [h for h in hosts if not (h and h.startswith(stack.STUDY_PREFIX))]
    if bad:
        raise ValueError(f"study URLs name non-study hosts: {bad}")
    return ",".join(["127.0.0.1", "localhost", *hosts])


def _check_token(token):
    if not isinstance(token, str) or not token or any(c.isspace() for c in token):
        raise ValueError("invalid token")
    return token


def env_file_text(spec, token, urls):
    np = no_proxy(urls)
    return (arms.env_file_lines(spec) + f"CLAUDE_CODE_OAUTH_TOKEN={_check_token(token)}\n"
            f"HTTPS_PROXY=http://{PROXY}:{PROXY_PORT}\nNO_PROXY={np}\nno_proxy={np}\n")


def read_token_file():
    """The default token reader: SAB_TOKEN_FILE, mode 0600, a regular file (pilot's checks). There
    is deliberately no default path: Task 10 names the account that pays."""
    path = os.environ.get("SAB_TOKEN_FILE")
    if not path:
        raise Refused("SAB_TOKEN_FILE is not set; the operator names the token file at Task 10")
    p = Path(path)
    st = p.lstat()
    if p.is_symlink() or not p.is_file() or st.st_mode & 0o077:
        raise Refused(f"{p}: not a regular 0600 file")
    return _check_token(p.read_text().strip())


def is_excluded_path(name):
    """True for a path under a .codescout/, .buddy/ or .superpowers/ directory at any depth."""
    return any(part in PATCH_EXCLUDED_DIRS for part in PurePosixPath(name).parts[:-1])


def extract_patch(git, base):
    """(patch, edited, excluded) of the work tree against `base`; `git(*args) -> stdout`.

    pilot.extract_patch's `git add -A; git diff --cached --binary <base>` with PATCH_PATHSPEC, so
    tool session state never enters a patch (R2-F3); `excluded` lists what was left out."""
    git("add", "-A")
    changed = [n for n in git("diff", "--cached", "--name-only", "-z", base).split("\0") if n]
    edited = [n for n in git("diff", "--cached", "--name-only", "-z", base, "--",
                             *PATCH_PATHSPEC).split("\0") if n]
    patch = git("diff", "--cached", "--binary", base, "--", *PATCH_PATHSPEC)
    kept = set(edited)
    excluded = [n for n in changed if n not in kept]
    if any(is_excluded_path(n) for n in edited) or not all(is_excluded_path(n) for n in excluded):
        raise RuntimeError(f"pathspec and path rule disagree: edited {edited}, excluded {excluded}")
    return patch, edited, excluded


def render_proxy_config(template, subnets, allowlist=(ALLOWED_HOST,)):
    """Python port of S5 proxy.mjs renderProxyConfig (claude-plugins 223b56abe,
    effort-steering/spikes/s5/proxy.mjs:92-124), allowlist mode, over the vendored template."""
    if not subnets or not all(isinstance(s, str) and _SUBNET.fullmatch(s) for s in subnets):
        raise ValueError(f"proxy subnets must be IPv4 CIDRs, got {subnets!r}")
    if not allowlist or not all(isinstance(h, str) and _HOST.fullmatch(h) for h in allowlist):
        raise ValueError(f"proxy allowlist must be plain lower-case host names: {allowlist!r}")
    filt = "".join(f"^{h.replace('.', chr(92) + '.')}$\n" for h in dict.fromkeys(allowlist))
    filter_lines = ["FilterDefaultDeny Yes", "FilterType ere", "FilterURLs Off",
                    "FilterCaseSensitive Off", f'Filter "{PROXY_CONF_MOUNT}/filter"']
    out, seen = [], {"allow": 0, "filter": 0}
    for line in template.split("\n"):
        if line == "@@ALLOW@@":
            seen["allow"] += 1
            out += [f"Allow {s}" for s in subnets]
        elif line == "@@FILTER@@":
            seen["filter"] += 1
            out += filter_lines
        else:
            out.append(line)
    if seen != {"allow": 1, "filter": 1}:
        raise ValueError(f"template must hold one @@ALLOW@@ and one @@FILTER@@ line: {seen}")
    return "\n".join(out), filt


# Fail-closed egress probe (pilot.start_proxy), from a named sab- container on the run network.
NET_PROBE_PY = f'''import socket
for host, expected in [("github.com",b"403"),("huggingface.co",b"403"),("{ALLOWED_HOST}",b"200")]:
    s=socket.create_connection(("{PROXY}",{PROXY_PORT}),10)
    s.sendall(("CONNECT "+host+":443 HTTP/1.1\\r\\nHost: "+host+":443\\r\\n\\r\\n").encode())
    assert expected in s.recv(4096), host
    s.close()
s=socket.socket();s.settimeout(3)
assert s.connect_ex(("1.1.1.1",443)) != 0
'''


# --- ownership -----------------------------------------------------------------------------------

class Owned:
    """The ledger of objects this runner created: claimed before creation, removed by exact name."""

    def __init__(self, data, docker):
        self.path = Path(data) / OWNED
        self.docker = docker

    def entries(self):
        if not self.path.exists():
            return []
        return [json.loads(l) for l in self.path.read_text().splitlines() if l.strip()]

    def live(self):
        """[(kind, name)] claimed and not yet removed, latest claim last."""
        state = {}
        for e in self.entries():
            state[(e["kind"], e["name"])] = e["event"]
        return [k for k, v in state.items() if v == "claim"]

    def exists(self, kind, name):
        if kind == "container":
            return name in self.docker("ps", "-a", "--format", "{{.Names}}").stdout.split()
        if kind in ("volume", "network"):
            return self.docker(kind, "inspect", name, check=False).returncode == 0
        raise ValueError(kind)

    def claim(self, kind, name):
        if not name.startswith(stack.STUDY_PREFIX):
            raise ValueError(f"{name}: the runner creates only sab- objects")
        if self.exists(kind, name):
            raise Refused(f"{kind} {name} already exists and this runner's ledger does not hold "
                          f"it as live: never adopted, never removed")
        _append(self.path, {"event": "claim", "kind": kind, "name": name})

    def removed(self, kind, name):
        _append(self.path, {"event": "removed", "kind": kind, "name": name})

    def release(self, kind, name):
        if (kind, name) not in self.live():
            raise RuntimeError(f"{kind} {name} is not owned by this runner")
        if kind == "network":
            if self.exists("network", name):
                self.docker("network", "rm", name)
        elif kind == "container":
            stack._remove_exact(self.docker, containers=[name])
        else:
            stack._remove_exact(self.docker, volumes=[name])
        self.removed(kind, name)

    def cleanup(self):
        """Remove every live ledger object: containers, then volumes, then networks."""
        errors = []
        live = self.live()
        for kind in ("container", "volume", "network"):
            for k, name in reversed(live):
                if k == kind:
                    try:
                        self.release(kind, name)
                    except Exception as e:  # keep going; report every leftover
                        errors.append(f"{kind} {name}: {e}")
        if errors:
            raise RuntimeError("cleanup left objects behind: " + "; ".join(errors))


# --- tasks ---------------------------------------------------------------------------------------

def load_task_from_bundle(task_id, tasks_root=None, *, inspect=None, grader_for=None):
    """A task's TaskInfo from its bundle.json. The image is tasks.bundle_run_image's (the verified
    exec-only arm image, Task 7 carry); the grader runs on that same image."""
    root = Path(tasks_root) if tasks_root is not None else tasks.DATA / "tasks"
    bundle_json = root / task_id / "bundle.json"
    image = tasks.bundle_run_image(bundle_json, inspect or tasks._inspect_image)
    rec = json.loads(bundle_json.read_text())
    if rec.get("kind") == "private":
        prompt = Path(rec["out"]) / "s5" / "tasks" / task_id / "prompt.txt"
    elif rec.get("kind") == "foreign":
        prompt = Path(rec["out"]) / "prompt.txt"
    elif rec.get("kind") == "live":
        prompt = Path(rec["out"]) / "prompt.txt"
    else:
        raise RuntimeError(f"{bundle_json}: unknown kind {rec.get('kind')!r}")
    template = rec.get("template") or ""
    if rec.get("kind") == "live":
        template = ""  # a Live bundle has no index, so no Qdrant template: bare arm only
    elif not template.startswith(stack.TEMPLATE_PREFIX):
        raise RuntimeError(f"{bundle_json}: no Qdrant template recorded")
    make = grader_for or tasks.grader_for
    return TaskInfo(image, prompt.read_text(), template, lambda gdir: make(rec, image, gdir),
                    rec["kind"])


def docker_exec_cli(container, argv, timeout):
    """(stdout, stderr, exit code or None on timeout) of the CLI in the agent container."""
    try:
        p = subprocess.run(["docker", "exec", "-w", tasks.WORKDIR, container, *argv],
                           capture_output=True, text=True, timeout=timeout)
        return p.stdout, p.stderr, p.returncode
    except subprocess.TimeoutExpired as e:
        text = lambda b: b.decode(errors="replace") if isinstance(b, bytes) else (b or "")
        return text(e.stdout), text(e.stderr), None


def _redact_tree(root, token):
    """Replace the token in every regular file under `root` (symlinks are never followed)."""
    if not token:
        return
    needle = token.encode()
    for path in Path(root).rglob("*"):
        if path.is_symlink() or not path.is_file():
            continue
        raw = path.read_bytes()
        if needle in raw:
            path.write_bytes(raw.replace(needle, b"[REDACTED]"))


# --- infrastructure ------------------------------------------------------------------------------

def _infra_up(docker, owned, data, manifest, need_retrieval):
    """The run network (internal), the egress proxy (api.anthropic.com only, probed fail-closed)
    and, for cs/full, the runner's own dense and sparse embedders. Returns (urls, record)."""
    owned.claim("network", NET)
    docker("network", "create", "--internal", "--label", "sab.owner=setup-ab-run", NET)
    stack._require_internal(docker, NET)
    subnets = docker("network", "inspect", "-f", "{{range .IPAM.Config}}{{.Subnet}} {{end}}",
                     NET).stdout.split()
    conf, filt = render_proxy_config(PROXY_TEMPLATE.read_text(), subnets)
    pdir = Path(data) / "proxy"
    pdir.mkdir(exist_ok=True)
    (pdir / "tinyproxy.conf").write_text(conf)
    (pdir / "filter").write_text(filt)
    record = {"network": NET, "subnets": subnets, "proxy": PROXY,
              "proxy_image_id": docker("image", "inspect", PROXY_IMAGE, "--format",
                                       "{{.Id}}").stdout.strip()}
    owned.claim("container", PROXY)
    docker("run", "-d", "--name", PROXY, "--network", NET, "--read-only", "--cap-drop", "ALL",
           "--security-opt", "no-new-privileges", "-v", f"{pdir}:{PROXY_CONF_MOUNT}:ro",
           PROXY_IMAGE)
    docker("network", "connect", "bridge", PROXY)
    owned.claim("container", NETPROBE)
    probe = docker("run", "--rm", "--name", NETPROBE, "--network", NET, "--cap-drop", "ALL",
                   "--security-opt", "no-new-privileges", PROBE_IMAGE, "python", "-c",
                   NET_PROBE_PY, check=False, timeout=120)
    if probe.returncode:
        raise RuntimeError(f"egress probe failed: {probe.stdout[-2000:]}")
    urls = {}
    if need_retrieval:
        retrieval = manifest["retrieval"]
        if retrieval.get("reranker"):
            raise RuntimeError("manifest marks a reranker present; the study has no copy of it")
        if not retrieval.get("qdrant"):
            raise RuntimeError("manifest marks no Qdrant; cs/full cannot run as in daily use")
        services = [s for s in ("dense", "sparse") if retrieval.get(s)]
        if "sparse" in services and docker("volume", "inspect", stack.MODEL_CACHE,
                                           check=False).returncode:
            raise RuntimeError(f"{stack.MODEL_CACHE} is missing; prepare creates it")
        names = {"dense": DENSE, "sparse": SPARSE}
        builders = {"dense": stack._dense_args, "sparse": stack._sparse_args}
        for s in services:
            args = builders[s]()
            args[args.index("--name") + 1] = names[s]
            owned.claim("container", names[s])
            docker("run", "-d", "--network", NET, *args)
            urls.update(stack._urls(s, names[s]))
        stack.wait_ready({names[s]: stack.PROBES[s] for s in services}, docker=docker)
    record["retrieval_urls"] = urls
    (Path(data) / "infra.json").write_text(json.dumps(record, indent=2) + "\n")
    return urls, record


# --- one run -------------------------------------------------------------------------------------

def read_cgroup_memory(docker, name):
    """{"oom_kill": int, "peak": int} of a running container's cgroup v2 memory (memory.events
    `oom_kill`, memory.peak in bytes); None for a value that cannot be read."""
    def read(path):
        r = docker("exec", name, "cat", path, check=False)
        return r.stdout if r.returncode == 0 else None

    events, peak = read("/sys/fs/cgroup/memory.events"), read("/sys/fs/cgroup/memory.peak")
    oom_kill = None
    for line in (events or "").splitlines():
        parts = line.split()
        if len(parts) == 2 and parts[0] == "oom_kill" and parts[1].isdigit():
            oom_kill = int(parts[1])
    peak = peak.strip() if peak else ""
    return {"oom_kill": oom_kill, "peak": int(peak) if peak.isdigit() else None}

def grade_outcome(grade):
    """(outcome, stop, why) of one grader result. Fails closed; fix round 1 (I4).

    Spec section 7 stops on "grader contamination". The signals the graders actually expose:
      - S5 grade.mjs `infra` (create_failed, oom, inspect_failed, rm_failed, timeout,
        incomplete) and tasks.private_grader's `oom_killed` / `infra: "oom"`;
      - tasks.foreign_grader's `error` (baseline.grade_output refused the log: an unexplained
        grader exit, a missing required test, a duplicate test identity);
      - evidence.checked_pass on an S5-shaped grade (a `build` key): a pass that build, infra or
        the test lists contradict;
      - no boolean `pass`.
    Each one stops the batch. NOT detectable here: tampering that still yields normal-looking
    test output (an in-source #[cfg(test)] module the agent edited, which neither grader restores;
    git attribute or textconv tricks, grade.mjs:345)."""
    if not isinstance(grade, dict) or type(grade.get("pass")) is not bool:
        return "infra-other", "grader anomaly", "grade has no boolean pass"
    if grade.get("oom_killed") or grade.get("infra") == "oom":
        return "infra-oom", "grader infra", "the grader's container was OOM-killed"
    if grade.get("infra") is not None:
        return "infra-other", "grader infra", f"grader infra: {grade['infra']}"
    if grade.get("error"):
        return "infra-other", "grader anomaly", f"grader output refused: {grade['error']}"
    if "build" in grade:
        try:
            evidence.checked_pass(grade)
        except ValueError as e:
            return "infra-other", "grader anomaly", str(e)
    return "scored", None, None


def _run_one(ctx, spec):
    """Run one spec; returns (final row, stop reason or None).

    The final row is always written before anything that can still fail: an exception after the
    pending row writes `unknown-spend` (or `infra-other` with the cost when it is known) and is
    re-raised; teardown and redaction errors are collected and raised only after the row and
    run-record.json exist (fix round 1, I2); an interrupt during grading still writes the row."""
    rid = run_id(spec)
    key = _key(rid)
    qrun = QDRANT_RUN_PREFIX + key
    name = AGENT_PREFIX + key
    has_qdrant = spec.arm != "bare"
    info = ctx.load_task(spec.task_id)
    if info.kind == "live" and spec.arm != "bare":
        raise RuntimeError(f"task {spec.task_id} is a live task (bare arm only, no index); "
                           f"arm {spec.arm!r} is refused before any container starts")
    run_dir = ctx.data / "runs" / rid
    run_dir.mkdir(parents=True)  # exists -> raise: a run dir is never reused (see M6 in run.py)
    session = str(uuid.uuid4())
    urls = study_urls(key, ctx.retrieval_urls)
    record = {"runId": rid, "spec": list(spec), "stage": stage_of(spec.task_id),
              "image": info.image, "sessionId": session, "container": name,
              "qdrant_run": qrun if has_qdrant else None}
    rows_path = ctx.data / ROWS
    state = {"pending": False, "cost": None}
    docker = ctx.docker
    errors = []
    patch = edited = excluded = None
    try:
        if has_qdrant:
            volume = stack.RUN_VOLUME_PREFIX + qrun
            for kind, n in (("volume", volume), ("container", stack.COPY_HELPER_PREFIX + volume),
                            ("container", f"sab-qdrant-{qrun}")):
                ctx.owned.claim(kind, n)
            got = stack.qdrant_for_run(info.template, qrun, network=NET, docker=docker)
            if got != urls["CODESCOUT_QDRANT_URL"]:
                raise RuntimeError(f"per-run Qdrant URL {got} != {urls['CODESCOUT_QDRANT_URL']}")
        arm_spec = arms.arm_invocation(spec.arm, spec.model, info.prompt, ctx.manifest, urls,
                                       run_dir, session_id=session, profile=ctx.profile)
        argv = cli_argv(arm_spec.cli_args, ctx.budget.per_call_usd)
        record["argv_flags"] = argv[:argv.index("--")]
        env_file = run_dir / "private.env"
        fd = os.open(env_file, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
        with os.fdopen(fd, "w") as f:
            f.write(env_file_text(arm_spec, ctx.token, urls))
        ctx.owned.claim("container", name)
        try:
            docker(*arms.docker_run_args(name, info.image, NET, arm_spec, env_file))
        finally:
            env_file.unlink(missing_ok=True)
        uid = docker("exec", name, "id", "-u").stdout.strip()
        if uid != AGENT_UID:
            raise RuntimeError(f"agent container runs as uid {uid!r}, not {AGENT_UID}; the CLI "
                               f"is not started")
        base = docker("exec", "-w", tasks.WORKDIR, name, "git", "rev-parse", "HEAD").stdout.strip()
        _event(ctx.data, "call", runId=rid)  # ties the call to this batch's per_call (I1)
        _append(rows_path, make_row(spec, session, outcome=PENDING))
        state["pending"] = True
        started = ctx.clock()
        stdout, stderr, rc = ctx.exec_cli(name, argv, ctx.timeout)
        wall = round((ctx.clock() - started) * 1000)
        killed = killed_rc = None
        if rc is None:  # M1: the docker exec client timed out; the CLI inside may still spend
            killed_rc = docker("kill", name, check=False).returncode
            killed = "timeout"
        stdout = (stdout or "").replace(ctx.token, "[REDACTED]")
        stderr = (stderr or "").replace(ctx.token, "[REDACTED]")
        (run_dir / "stream.jsonl").write_text(stdout)
        (run_dir / "stderr.txt").write_text(stderr)
        # N-5: the cgroup's own count, read while the container still exists (after a timeout
        # kill it is gone: unknown). A kernel OOM kill of a non-init process (rust-analyzer, a
        # rustc) need not set State.OOMKilled.
        mem = read_cgroup_memory(docker, name) if killed is None else {"oom_kill": None,
                                                                       "peak": None}
        record["cgroupMemory"] = mem
        oom_flag = docker("inspect", "-f", "{{.State.OOMKilled}}", name,
                          check=False).stdout.strip() == "true"
        record["stateOomKilled"] = oom_flag
        oom = oom_flag or bool(mem["oom_kill"])
        if oom and killed is None:  # M1: nothing of an OOM-hit container keeps running
            killed_rc = docker("kill", name, check=False).returncode
            killed = "oom"
        record["killed"] = killed
        record["killedRc"] = killed_rc
        lines, dups, shape_errors = dedupe_tool_uses(stdout.splitlines())
        (run_dir / "stream.dedup.jsonl").write_text("".join(l + "\n" for l in lines))
        records = _records(lines)
        result = _result_record(records)
        cost = _cost(result)
        state["cost"] = cost
        verdict = integrity.check(spec.arm, run_dir / "stream.dedup.jsonl",
                                  result if result is not None else {})
        integ = dict(verdict._asdict(), duplicateToolUses=dups, toolUseShapeErrors=shape_errors)
        if shape_errors:
            integ["ok"] = False
            integ["reasons"] = list(verdict.reasons) + [f"malformed tool_use ({e})"
                                                        for e in shape_errors]
        pilot.safe_config_files(run_dir / "config")  # no symlink is followed into the host
        usage = run_usage(run_dir / "config", session)
        transcript = (Path(usage["transcriptPath"]).read_text(errors="replace")
                      if usage.get("transcriptPath") else "")
        hit = plan_limit(envelope=result, stdout=stdout, stderr=stderr, transcript=transcript)
        calls = list(integrity._tool_uses(records))
        outcome, stop, why = None, None, None
        if hit:
            outcome, stop = "plan-limit", "plan-limit"
        elif oom:
            outcome, stop = "infra-oom", ("missing cost" if cost is None else None)
        elif killed is None and mem["oom_kill"] is None:
            # Pre-freeze M-1: an unreadable OOM count fails CLOSED (a child OOM on a daemon that
            # does not set State.OOMKilled would otherwise be scored as a normal attempt).
            outcome, stop = "infra-other", "oom reading unknown"
            why = "cgroup memory.events oom_kill unreadable after the CLI returned"
        elif result is None:
            outcome, stop = "infra-other", "missing cost"
            why = "timeout" if rc is None else f"no result record (exit {rc})"
        elif cost is None:
            outcome, stop, why = "infra-other", "missing cost", "result has no usable total_cost_usd"
        elif result.get("is_error") and integrity._did_no_model_work(result, calls):
            outcome, stop = "infra-other", "no model work"
            why = "is_error with zero api time, tokens, cost and calls (credential or infra)"
        elif not integ["ok"]:
            outcome = "invalid-arm"
        if outcome in (None, "invalid-arm"):
            git = lambda *a: docker("exec", "-w", tasks.WORKDIR, name, "git", *a).stdout
            patch, edited, excluded = extract_patch(git, base)
            (run_dir / "agent.patch").write_text(patch)
        fields = dict(costUsd=cost, wallMs=wall, capHit=cap_hit(result),
                      oomKills=mem["oom_kill"], memoryPeakBytes=mem["peak"],
                      numTurns=num_turns(records), usage=usage,
                      effectiveEffort=effective_effort(usage), integrity=integ,
                      bashCommands=bash_commands(records), editedFiles=edited,
                      inputTokens=usage["total"]["input_total"] if usage.get("reason") is None else None,
                      outputTokens=usage["total"]["output"] if usage.get("reason") is None else None)
        record.update(exitCode=rc, oomKilled=oom, planLimit=hit, why=why, patchExcluded=excluded)
    except BaseException as e:
        if state["pending"]:
            final = (("infra-other", state["cost"]) if state["cost"] is not None
                     else ("unknown-spend", None))
            _append(rows_path, make_row(spec, session, outcome=final[0], costUsd=final[1]))
            record.update(outcome=final[0], error=f"{type(e).__name__}: {e}")
        else:
            record.update(outcome=None, error=f"{type(e).__name__}: {e} (before the CLI call)")
        _cleanup_run(ctx, name, qrun, has_qdrant, run_dir, errors)
        record["teardownErrors"] = errors
        _write_record(run_dir, record)
        raise
    _cleanup_run(ctx, name, qrun, has_qdrant, run_dir, errors)
    grade, interrupted = None, None
    if outcome is None:
        try:
            grade = info.grader(run_dir / "grades")(patch, rid)
        except BaseException as e:  # an interrupt too: the row with the known cost comes first
            outcome, stop, why = "infra-other", "grader error", f"{type(e).__name__}: {e}"
            if not isinstance(e, Exception):
                interrupted = e
        else:
            outcome, stop, why = grade_outcome(grade)
    if outcome == "infra-other" and stop is None:
        stop = "infra-other"  # every infra-other stops the batch (I4)
    if outcome == "scored" and stage_of(spec.task_id) == "dev":
        outcome = "dev-scored"
    row = make_row(spec, session, outcome=outcome, grade=grade if outcome in GRADED else None,
                   **fields)
    _append(rows_path, row)
    record.update(outcome=outcome, stop=stop, why=why, graderResult=grade, teardownErrors=errors)
    _write_record(run_dir, record)
    if interrupted is not None:
        raise interrupted
    if errors:
        raise RuntimeError(f"{rid}: teardown failed after its row was written: {errors}")
    return row, stop


def _write_record(run_dir, record):
    (run_dir / "run-record.json").write_text(json.dumps(record, indent=2, default=str) + "\n")
def _cleanup_run(ctx, name, qrun, has_qdrant, run_dir, errors):
    """The run's agent container and Qdrant objects, by exact name through the ledger, then the
    token redaction of the run dir. Never raises: each failure is appended to `errors`."""
    try:
        live = set(ctx.owned.live())
    except Exception as e:
        errors.append(f"ledger: {type(e).__name__}: {e}")
        live = set()
    if ("container", name) in live:
        try:
            ctx.owned.release("container", name)
        except Exception as e:
            errors.append(f"container {name}: {type(e).__name__}: {e}")
    if has_qdrant:
        volume = stack.RUN_VOLUME_PREFIX + qrun
        objs = [("container", f"sab-qdrant-{qrun}"),
                ("container", stack.COPY_HELPER_PREFIX + volume), ("volume", volume)]
        if any(o in live for o in objs):
            try:
                stack.run_down(qrun, docker=ctx.docker)
                for o in objs:
                    if o in live:
                        ctx.owned.removed(*o)
            except Exception as e:
                errors.append(f"qdrant run {qrun}: {type(e).__name__}: {e}")
    try:
        _redact_tree(run_dir, ctx.token)
    except Exception as e:
        errors.append(f"redaction: {type(e).__name__}: {e}")




# --- the batch -----------------------------------------------------------------------------------

def _check_cli(manifest):
    cli = manifest["cli"]
    got = hashlib.sha256(Path(cli["path"]).read_bytes()).hexdigest()
    if got != cli["sha256"]:
        raise Refused(f"CLI {cli['path']} changed after freeze ({got} != {cli['sha256']})")
def _frozen_inputs(reg, manifest):
    """Final review I-7: the registration's frozen arm inputs (`frozen.root`, `frozen.trees`)
    must hash as frozen, or Refused; returns the manifest that names only the frozen copies."""
    fz = reg.get("frozen")
    if not isinstance(fz, dict) or not isinstance(fz.get("root"), str) or not fz["root"]:
        raise Refused("the registration records no frozen arm inputs (frozen.root, "
                      "frozen.trees); see frozen.py")
    try:
        frozen.check_trees(fz["root"], fz.get("trees"))
        names = set(frozen.entry_names(manifest))
        if names != set(fz["trees"]):
            raise frozen.Mismatch(f"the registration freezes {sorted(fz['trees'])}, the "
                                  f"manifest's arms read {sorted(names)}")
        return frozen.frozen_manifest(manifest, fz["root"])
    except frozen.Mismatch as e:
        raise Refused(str(e))


def plan_reps(plan):
    """"stage3" or "stage1" when every (task, model) block of `plan` holds exactly the runs of
    STAGE3_REPS or STAGE1_REPS, else None."""
    blocks = {}
    for s in check_plan(plan):
        blocks.setdefault((s.task_id, s.model), []).append((s.arm, s.rep))
    for name, reps in (("stage3", STAGE3_REPS), ("stage1", STAGE1_REPS)):
        want = sorted((arm, r) for arm, n in reps for r in range(n))
        if all(sorted(b) == want for b in blocks.values()):
            return name
    return None


def _dry_run(plan, budget, data):
    data.mkdir(parents=True, exist_ok=True)
    out = {"dry_run": True, "count": len(plan), "plan_sha256": plan_sha256(plan),
           "reps": plan_reps(plan),
           "specs": [list(s) for s in plan],
           "run_ids": [run_id(s) for s in plan], "budget": budget._asdict(),
           "worst_case_usd": round(len(plan) * budget.per_call_usd, 6),
           "first_call_admitted": admit(budget, budget.spent_usd),
           "go_file": str(data / GO_FILE),
           "note": "no docker object, CLI call or token was touched"}
    (data / "dry-run.json").write_text(json.dumps(out, indent=2) + "\n")
    return out


def run_batch(plan, budget, data, dry_run, *, registration=None, docker=None, exec_cli=None,
              load_task=None, manifest=None, token_reader=None, clock=time.monotonic,
              timeout=RUN_TIMEOUT_S, inspect=None, tool_dir=None, s5_dir=None, tasks_root=None,
              node_version=None):
    """Run `plan` in order, one spec at a time. Keyword-only additions to the brief's signature
    are the registration the batch runs under (required to spend: load_registration, and GO.json
    must name its registration_sha256; a Stage 2 registration is also re-checked against the
    host by registration.check: tool files, data dir, manifest, images, grader inputs), the
    injection points for tests (fake docker, fake CLI, inspect, tool_dir, s5_dir) and the frozen
    manifest (default `<data>/setup-manifest.json`)."""
    plan = check_plan(plan)
    budget = check_budget(budget)
    data = Path(data)
    if dry_run:
        _dry_run(plan, budget, data)
        return None
    if registration is None:
        raise Refused("a batch that spends runs under a registration: pass --registration")
    reg = load_registration(registration, plan)  # before any side effect
    profile = registered_arm_profile(reg)  # a name the arms do not know is refused before docker
    check_registered_budget(reg, plan, budget)  # N-2
    if plan_reps(plan) == "stage3" or reg.get("stage") == 2:
        # Task 11 rounds 1-2 (I-1, I-2, I-3, N-1a, m-2, m-4..m-6): for every Stage 3 plan,
        # whatever the registration says it is, the registration is the committed one and the
        # host is what was frozen; a registration without those keys is refused.
        import registration as regcheck  # lazy: registration imports freeze_stage1 -> run
        try:
            regcheck.check_committed(registration, tool_dir or HERE)
            regcheck.check(reg, data=data, tool_dir=tool_dir or HERE,
                           inspect=inspect or tasks._inspect_image, s5_dir=s5_dir,
                           runner_tasks_root=tasks_root, node_version=node_version)
        except regcheck.Mismatch as e:
            raise Refused(f"the registration no longer matches the host: {e}")
        # Task 12: --spent (GO.json spent_usd) is the registered carried spend of the archived
        # pilot attempt 1; the study journal counts Stage 1's rows itself.
        if float(budget.spent_usd) != float(reg["carried_spend_usd"]):
            raise Refused(f"--spent {budget.spent_usd} is not the registered carried spend "
                          f"{reg['carried_spend_usd']}")
    go = check_go(data, plan, budget, reg["registration_sha256"])
    check_journal(data)  # rows without the journal that covers them are refused
    acknowledged = check_latch(data, go)  # I3: a stop holds until the operator acknowledges it
    docker = docker or pilot.docker
    exec_cli = exec_cli or docker_exec_cli
    load_task = load_task or load_task_from_bundle
    token_reader = token_reader or read_token_file
    if manifest is None:
        manifest = json.loads((data / "setup-manifest.json").read_text())
    _check_cli(manifest)
    manifest = _frozen_inputs(reg, manifest)  # I-7: re-hashed at every batch start
    try:
        lock = os.open(data / LOCK, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    except FileExistsError:
        raise Refused(f"{data / LOCK} exists: another batch runs, or one died; check before "
                      f"removing it")
    os.write(lock, f"{os.getpid()}\n".encode())
    os.close(lock)
    stop = None
    try:
        _event(data, "start", plan_sha256=plan_sha256(plan), budget=budget._asdict(),
               count=len(plan), acknowledged=acknowledged[0] if acknowledged else None,
               registration_sha256=reg["registration_sha256"])
        for row in recover_pending(data):
            _event(data, "recovered", runId=row["runId"], outcome="unknown-spend")
        owned = Owned(data, docker)
        owned.cleanup()  # leftovers of an earlier batch of THIS runner (ledger-live names only)
        ctx = None
        try:
            for spec in plan:
                rid = run_id(spec)
                rows = read_rows(data)
                if rid in latest_rows(rows):
                    _event(data, "skip", runId=rid, reason="journaled; no automatic retry")
                    continue
                spent = spent_usd(rows, budget, read_events(data))
                if not admit(budget, spent):
                    stop = "admission"
                    _event(data, "refused", runId=rid, spent=spent)
                    break
                if ctx is None:
                    token = _check_token(token_reader())
                    urls, _ = _infra_up(docker, owned, data, manifest,
                                        any(s.arm != "bare" for s in plan))
                    ctx = _Ctx(docker, exec_cli, load_task, manifest, token, data, budget, owned,
                               urls, timeout, clock, profile)
                row, stop = _run_one(ctx, spec)
                _event(data, "row", runId=rid, outcome=row["outcome"], costUsd=row["costUsd"])
                validity = task_validity(read_rows(data))
                (data / VALIDITY).write_text(json.dumps(validity, indent=2) + "\n")
                v = validity.get(f"{spec.task_id}|{spec.model}")
                if stop is None and v is not None and not v["valid"]:
                    stop = "effort mismatch"
                if stop:
                    break
        except BaseException as e:
            stop = f"error: {type(e).__name__}: {e}"
            raise
        finally:
            try:
                owned.cleanup()
            finally:
                _event(data, "stop", reason=stop or PLAN_COMPLETE,
                       spent=spent_usd(read_rows(data), budget, read_events(data)))
    finally:
        (data / LOCK).unlink(missing_ok=True)
    return None


class _Ctx(NamedTuple):
    docker: Callable
    exec_cli: Callable
    load_task: Callable
    manifest: dict
    token: str
    data: Path
    budget: Budget
    owned: Owned
    retrieval_urls: dict
    timeout: float
    clock: Callable
    profile: str = "v1"  # the registration's arm profile (registered_arm_profile)

def load_plan(path):
    """A plan file: JSON [[task, model, arm, rep], ...] (stage3.py writes them)."""
    return check_plan([RunSpec(*s) for s in json.loads(Path(path).read_text())])


def main(argv=None):
    p = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    sub = p.add_subparsers(dest="action", required=True)
    d = sub.add_parser("dry-run", help="a plan's specs, hash and worst case; no docker, no CLI, "
                                       "no token")
    d.add_argument("--plan", type=Path, default=None,
                   help="a plan file (JSON [[task, model, arm, rep], ...]); else --tasks/--seed")
    d.add_argument("--stage3", action="store_true",
                   help="Stage 3: STAGE3_REPS for --tasks, and the Stage 3 budget defaults "
                        f"(cap {STAGE3_CAP_USD}, per-call {STAGE3_PER_CALL_USD}, reserve "
                        f"{STAGE3_RESERVE_USD})")
    d.add_argument("--seed", type=int, default=None)
    d.add_argument("--tasks", nargs="*", default=list(STAGE1_DEV_TASKS))
    d.add_argument("--data", type=Path, default=tasks.DATA / "stage1-dry-run")
    d.add_argument("--cap", type=float, default=None)
    d.add_argument("--spent", type=float, default=0.0)
    d.add_argument("--per-call", type=float, default=None)
    d.add_argument("--reserve", type=float, default=None)
    r = sub.add_parser("run", help="spend: refused without <data>/GO.json and its registration")
    r.add_argument("--plan", type=Path, required=True, help="JSON [[task, model, arm, rep], ...]")
    r.add_argument("--data", type=Path, required=True)
    r.add_argument("--registration", type=Path, required=True,
                   help="the frozen registration; GO.json names its registration_sha256")
    for flag in ("--cap", "--spent", "--per-call", "--reserve"):
        r.add_argument(flag, type=float, required=True)
    a = p.parse_args(argv)
    if a.action == "dry-run":
        if a.plan is not None:
            plan = load_plan(a.plan)
        elif a.seed is None:
            p.error("dry-run needs --plan or --seed")
        else:
            plan = make_order(a.tasks, MODELS, a.seed, STAGE3_REPS if a.stage3 else STAGE1_REPS)
        if a.stage3:
            cap = STAGE3_CAP_USD if a.cap is None else a.cap
            per_call = STAGE3_PER_CALL_USD if a.per_call is None else a.per_call
        else:
            cap = STAGE1_CAP_USD if a.cap is None else a.cap
            per_call = float(arms.MAX_BUDGET_USD) if a.per_call is None else a.per_call
        budget = Budget(cap, a.spent, per_call, per_call if a.reserve is None else a.reserve)
        run_batch(plan, budget, a.data, dry_run=True)
        print(json.dumps(json.loads((a.data / "dry-run.json").read_text()), indent=2))
    else:
        run_batch(load_plan(a.plan), Budget(a.cap, a.spent, a.per_call, a.reserve), a.data,
                  dry_run=False, registration=a.registration)


if __name__ == "__main__":
    main()
