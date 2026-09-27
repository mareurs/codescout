"""Phase 1b Stage 2, Step 5: the gate, for one checkpoint, on its own final menu and on the common menu.

Implements docs/evals/phase1b-local-classifier-preregistration.md § Step 5 with § Stage 2's changes,
through an adapter over scripts/phase1-span-selector.py, as phase 1's Stage-4 amendment did with
scripts/phase1-local-trained.py. The gate, the span gate and their pass criteria are that file's code,
unchanged; this script supplies `judge_rule`, `JUDGED`, `GATE` and `SPAN_GATE`.
- judge_rule: the checkpoint's heads at the temperatures and precision thresholds Step 4 fit over own
  plus admitted cross cells (step4-<arm>-<seed>.json), not the run's own-cell files. Per (text, rule),
  over the text's segment() units at least MIN_SPAN long, P = sigmoid(z / T) of the unit with the
  highest logit; the rule fires when P >= t, and that unit is the claim, verbatim.
  Phase 1's adapter took the first unit of highest P. The two agree wherever P values differ, and
  sigmoid is monotone, so P and every fire decision are the same. They differ only where float
  saturation ties P at 1.0 (z / T above about 36.8, so any z above about 9.2 at T = 0.25): there the
  first unit won whatever its logit (review of 2026-09-27, finding 1). Each row records how many
  candidates tie at its P.
- JUDGED: the menu being read, the checkpoint's own final menu or the common menu (§ Stage 2: every
  checkpoint's gate is reported on both).
- GATE: phase 1's 10 texts, then the new clean texts that survived Step 1's labellers
  (audit/result.json), each expecting no rule.
- SPAN_GATE: phase 1's span texts whose rule is on the menu.
A menu that is not gate-able (§ Step 4) fails whatever the gate prints.

Before the gate, and refusing to run it otherwise:
- the checkpoint is the one its name says (arm and seed against Step 4's run directory), its sha256 is
  the one Step 4 scored, and the scored file is the one Step 4 read;
- the common menu is step4.py --common's over nine checkpoints, this one among them;
- every file a verdict depends on is committed, unchanged since HEAD, and recorded by sha256;
- the device is the RTX A5000 on CUDA, the registered hardware;
- determinism: a neutral text scored twice moves no logit by 1e-4 or more (phase 1's rule), and no
  logit is non-finite;
- parity: every frozen val row's own-cell logit and admitted cross-cell logits, recomputed through the
  judge's own scoring path, equal Step 4's scored values exactly.
A refusal at the last two is appended to refusals.jsonl.

Each checkpoint's gate runs once: two files are created exclusively just before it, the result in
--out-dir and a marker beside the checkpoint, so a second invocation is refused whatever paths it is
given. Every gate and span text is then scored in this thread, as parity was, before the gate's thread
pool reads the scores. A failure after that writes the result as crashed, with its traceback and every
row and error so far, and the reservation stands.

Run:
  ~/work/claude/jevk5/.venv/bin/python -u step5_gate.py --step4 stage2/step4-nc-20260935.json \
      --scored <scored-nc-20260935.json> --common stage2/common.json --out-dir stage2/gate
  ~/work/claude/jevk5/.venv/bin/python step5_gate.py --summary stage2/gate --out stage2/gate/summary.json
"""
import argparse
import contextlib
import datetime
import hashlib
import importlib.util
import io
import json
import math
import os
import re
import subprocess
import sys
import traceback
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[4]
sys.path.insert(0, str(HERE))
import clean_texts as ct  # noqa: E402


def _load(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


sel = _load("span_selector", ROOT / "scripts/phase1-span-selector.py")
s4 = _load("step4", HERE / "step4.py")
ta = s4.ta

# Taken before this script rebinds sel's globals for a menu.
PHASE1_GATE = list(sel.GATE)
PHASE1_SPAN = list(sel.SPAN_GATE)
PHASE1_TEXTS = 10
DEVICE_NAME = "NVIDIA RTX A5000"
DRIFT_MAX = 1e-4
NEUTRAL = ("The build script copies the assets into dist/. It then runs the bundler with the production "
           "flag and writes a manifest.")      # phase1-local-trained.py's determinism text
GATE_LINE = re.compile(r"^gate: (\d+)/(\d+)   errored rows: (\d+)$", re.M)
SPAN_LINE = re.compile(r"^span gate: (\d+)/(\d+)   errored runs: (\d+)$", re.M)
ARMS = ("b", "n", "nc")
SEEDS = ("20260935", "20260937", "20260940")
SHIP_SEED = "20260935"
RUN_NAMES = {"b": "s1-r1-{seed}", "n": "s2-n-{seed}", "nc": "s2-nc-{seed}"}   # each arm's run directories
RESERVED = "step5-gate.reserved"      # written beside best.pt when that checkpoint's gate starts
_D = "docs/evals/data/2026-09-24-rule-tell"
# Everything a verdict can depend on, which must be committed and unchanged when the gate runs.
CODE = [f"{_D}/phase1b/step5_gate.py", "scripts/phase1-span-selector.py", "scripts/phase1-rule-selection.py",
        f"{_D}/stage3/train_arm.py", f"{_D}/stage2/segment.py", f"{_D}/phase1b/step4.py",
        f"{_D}/phase1b/clean_texts.py", f"{_D}/phase1b/codex-clean-texts.jsonl", f"{_D}/phase1b/audit/result.json",
        f"{_D}/stage2/frozen", "docs/evals/phase1b-local-classifier-preregistration.md"]


def gate_texts(phase1: list[tuple[str, str, str]], survived: list[str],
               new: list[tuple[str, str]]) -> list[tuple[str, str, str]]:
    """Phase 1's gate texts, then each new clean text that survived Step 1, in registration order."""
    if len(phase1) != PHASE1_TEXTS:
        raise SystemExit(f"phase 1's gate has {len(phase1)} texts, not {PHASE1_TEXTS}")
    unknown = sorted(set(survived) - {cid for cid, _ in new})
    if unknown:
        raise SystemExit(f"audit/result.json names clean texts that do not exist: {unknown}")
    out = list(phase1) + [(cid, text, "none") for cid, text in new if cid in survived]
    if len({c for c, _, _ in out}) != len(out) or len({t for _, t, _ in out}) != len(out):
        raise SystemExit("gate texts must have distinct ids and distinct texts")
    return out


class Judge:
    """judge_rule for sel's gate: one checkpoint's heads, at Step 4's temperatures and precision
    thresholds, over the menu being read. `unit_logits(units)` returns [n_units][n_heads] floats."""

    def __init__(self, unit_logits, heads: list[str], temps: dict, thresholds: dict):
        self.unit_logits = unit_logits
        self.index = {h: i for i, h in enumerate(heads)}
        self.T = {h: v["T"] for h, v in temps.items()}
        self.t = {h: v["precision_t"] for h, v in thresholds.items()}
        self.menu: list[str] = []
        self.menu_name = None
        self.cache: dict[str, tuple] = {}
        self.log: list[dict] = []
        self.errors: list[dict] = []

    def set_menu(self, name: str, menu: list[str]) -> None:
        missing = sorted(h for h in menu if h not in self.index or h not in self.T or h not in self.t)
        if missing:
            raise SystemExit(f"menu {name}: no head, temperature or threshold for {missing}")
        self.menu, self.menu_name = list(menu), name

    def scored(self, text: str) -> tuple[list[str], list[int], list[list[float]]]:
        """The text's units, the candidate claims among them, and every unit's logits, once per text."""
        if text not in self.cache:
            units = ta.segment(text)
            cand = [i for i, u in enumerate(units) if len(u) >= sel.MIN_SPAN]
            self.cache[text] = (units, cand, self.unit_logits(units) if units else [])
        return self.cache[text]

    def judge_rule(self, text: str, rule: str) -> dict:
        """sel's hook. A raise is logged with its message first, since sel keeps only a count of errors."""
        try:
            return self._judge(text, rule)
        except Exception as e:
            self.errors.append(dict(menu=self.menu_name, text=text, rule=rule, error=f"{type(e).__name__}: {e}"))
            raise

    def _judge(self, text: str, rule: str) -> dict:
        if rule not in self.menu:
            raise ValueError(f"{rule} is not on the menu being read")
        units, cand, z = self.scored(text)
        j, T, t = self.index[rule], self.T[rule], self.t[rule]
        row = dict(menu=self.menu_name, text=text, rule=rule, threshold=t)
        if not cand:
            self.log.append({**row, "p": None, "verdict": "NO"})
            return {"rule": rule, "verdict": "NO"}
        zs = [z[i][j] for i in cand]
        if not all(math.isfinite(v) for v in zs):
            raise ValueError(f"a non-finite logit for {rule}: {zs}")
        k = max(range(len(zs)), key=zs.__getitem__)    # the highest logit: see the module docstring
        p = s4.sig(zs[k] / T)
        row.update(p=p, z=zs[k], ties_at_p=sum(s4.sig(v / T) == p for v in zs))
        if p < t:
            self.log.append({**row, "verdict": "NO"})
            return {"rule": rule, "verdict": "NO"}
        claim = units[cand[k]]
        span = sel.verify_span(claim, text)
        if not span:   # a segmenter unit of the text; a failure is a bug, raised, never downgraded
            raise ValueError(f"argmax unit failed verify_span: {claim[:80]!r}")
        self.log.append({**row, "verdict": "YES", "claim": span})
        return {"rule": rule, "verdict": "YES", "claim": span}


def determinism(unit_logits) -> dict:
    """Phase 1's check: the neutral text scored twice, uncached. A non-finite logit fails it."""
    units = ta.segment(NEUTRAL)
    a, b = unit_logits(units), unit_logits(units)
    finite = all(math.isfinite(x) for r in a + b for x in r)
    drift = max(abs(x - y) for ra, rb in zip(a, b) for x, y in zip(ra, rb)) if finite else None
    return dict(text="neutral", cells=sum(len(r) for r in a), finite=finite, max_abs_dz=drift,
                ok=finite and drift < DRIFT_MAX)


def parity(judge: Judge, rows: list[dict], scored: dict) -> dict:
    """Each frozen val row's own-cell logit and admitted cross-cell logits, recomputed through the
    judge's scoring path, against Step 4's scored values. Exact equality, as score_run.py asks of
    training: it proves the checkpoint and the head order the gate reads are the ones Step 4 read."""
    own = {r["id"]: r["z"] for r in scored["val"]}
    cross: dict[str, list] = {}
    for c in scored["val_cross"]:
        cross.setdefault(c["id"], []).append((c["unit"], c["head"], c["z"]))
    absent = sorted(r["id"] for r in rows if r["id"] not in own)
    if absent:
        raise SystemExit(f"the scored file lacks {len(absent)} frozen val rows, e.g. {absent[:3]}")
    cells = differing = 0
    worst = 0.0
    for r in rows:
        _, _, z = judge.scored(r["text"])
        pairs = [(z[r["target"]][judge.index[r["rule"]]], own[r["id"]])]
        pairs += [(z[u][judge.index[h]], v) for u, h, v in cross.get(r["id"], [])]
        for got, want in pairs:
            cells += 1
            if got != want:
                differing += 1
                worst = max(worst, abs(got - want))
    return dict(rows=len(rows), cells=cells, differing=differing, max_abs_dz=worst,
                ok=bool(rows) and differing == 0)


def per_text(texts: list[tuple[str, str, str]], rows: list[dict], menu: list[str]) -> list[dict]:
    """Each gate text's fired rules and pass, re-read from the judge's rows; checked against sel's count."""
    by_text: dict[str, list[dict]] = {}
    for r in rows:
        by_text.setdefault(r["text"], []).append(r)
    out = []
    for cid, text, want in texts:
        got = by_text.get(text, [])
        entry = dict(id=cid, want=want, applicable=want == "none" or want in menu,
                     fired=sorted(r["rule"] for r in got if r["verdict"] == "YES"))
        if entry["applicable"]:
            entry["passed"] = entry["fired"] == [] if want == "none" else want in entry["fired"]
            entry["p"] = {r["rule"]: r["p"] for r in got}
        out.append(entry)
    return out


def printed(pattern: re.Pattern, out: str) -> dict | None:
    m = pattern.search(out)
    return None if m is None else dict(zip(("passed", "of", "errored"), map(int, m.groups())))


def span_results(span: list[tuple[str, str, str, str]], rows: list[dict]) -> list[dict]:
    """Each span text's verdict and whether its claim lands in the violating sentence, from the rows."""
    by_text = {r["text"]: r for r in rows}
    out = []
    for cid, rule, text, violating in span:
        r = by_text.get(text)
        verdict = None if r is None else r["verdict"]
        out.append(dict(id=cid, rule=rule, verdict=verdict, claim=None if r is None else r.get("claim"),
                        on_target=verdict == "YES" and sel.claim_on_target(r["claim"], violating)))
    return out


class Tee(io.StringIO):
    """Keeps what sel prints and passes it straight on, so a long run shows its progress."""

    def __init__(self, echo):
        super().__init__()
        self.echo = echo

    def write(self, s: str) -> int:
        self.echo.write(s)
        self.echo.flush()
        return super().write(s)


def read_menu(judge: Judge, name: str, menu: list[str], texts: list[tuple[str, str, str]]) -> dict:
    """sel's gate and span gate over one menu: their verdicts, their printed output and the rows behind it."""
    judge.set_menu(name, menu)
    sel.JUDGED = list(menu)
    sel.GATE = list(texts)
    sel.SPAN_GATE = [s for s in PHASE1_SPAN if s[1] in menu]
    sel.judge_rule = judge.judge_rule
    run = argparse.Namespace(runs=1, pool=1)    # one run, labelled (the determinism rule); one GPU
    start, err_start = len(judge.log), len(judge.errors)
    buf = Tee(sys.stdout)
    with contextlib.redirect_stdout(buf):
        gate_rc = sel.gate(run)
        print()
        span_rc = sel.span_gate(run) if sel.SPAN_GATE else None
    out = buf.getvalue()
    rows = judge.log[start:]
    texts_res = per_text(texts, rows, menu)
    span_res = span_results(sel.SPAN_GATE, rows)
    g, s = printed(GATE_LINE, out), printed(SPAN_LINE, out)
    if g is not None and (g["passed"], g["of"]) != (sum(e.get("passed", False) for e in texts_res),
                                                    sum(e["applicable"] for e in texts_res)):
        raise SystemExit(f"menu {name}: the gate printed {g}, and its rows say otherwise")
    if s is not None and (s["passed"], s["of"]) != (sum(e["on_target"] for e in span_res), len(span_res)):
        raise SystemExit(f"menu {name}: the span gate printed {s}, and its rows say otherwise")
    able = s4.gate_ability(menu)
    return dict(menu=list(menu), gate_ability=able, gate_rc=gate_rc, span_rc=span_rc, gate=g, span=s,
                passed=gate_rc == 0 and span_rc == 0 and able["gate_able"], texts=texts_res,
                span_texts=span_res, errors=judge.errors[err_start:], output=out, rows=rows)


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def now() -> str:
    return datetime.datetime.now(datetime.timezone.utc).isoformat()


def write_json(path: Path, obj: dict) -> None:
    tmp = path.with_name(path.name + ".tmp")
    tmp.write_text(json.dumps(obj, indent=1) + "\n")
    os.replace(tmp, path)


def code_state(root: Path, paths: list[str]) -> dict:
    """HEAD, the listed paths that differ from it (modified, staged, untracked or ignored), and each file's
    sha256. Ignored files are listed too: git status shows them in no mode but --ignored, so an ignored,
    uncommitted input would otherwise read as committed."""
    def git(*a):
        return subprocess.run(["git", "-C", str(root), *a], capture_output=True, text=True, check=True).stdout
    dirty = git("status", "--porcelain", "--untracked-files=all", "--ignored=matching", "--", *paths).splitlines()
    return dict(head=git("rev-parse", "HEAD").strip(), dirty=sorted(line[3:] for line in dirty),
                sha256={p: sha256(root / p) for p in paths if (root / p).is_file()})


def check_common(common: dict, run_dir: str, final_menu: list[str]) -> None:
    """The common menu is step4.py --common's over all nine checkpoints, and this checkpoint is one of them."""
    runs = common["runs"]
    if len(runs) != len(ARMS) * len(SEEDS):
        raise SystemExit(f"refused: the common menu covers {len(runs)} checkpoints, not {len(ARMS) * len(SEEDS)}")
    if (run_dir, final_menu) not in [(r["run_dir"], r["final_menu"]) for r in runs]:
        raise SystemExit("refused: this checkpoint's final menu is not among the common menu's runs")
    if sorted(common["common_menu"]) != sorted(set.intersection(*(set(r["final_menu"]) for r in runs))):
        raise SystemExit("refused: the common menu is not the intersection of its runs' final menus")


def clean_fired(res: dict, cid: str, rule: str) -> bool:
    """Whether `rule` fired on gate text `cid` on the checkpoint's own menu; refused if it was not judged."""
    entry = next((e for e in res["menus"]["own"]["texts"] if e["id"] == cid), None)
    if entry is None or rule not in entry.get("p", {}):
        raise SystemExit(f"{res.get('checkpoint')}: {rule} was not judged on {cid}, so it cannot be read")
    return rule in entry["fired"]


def summarize(results: dict[str, dict]) -> dict:
    """The registered readings over the nine checkpoints: § Stage 2's reading, ship rule and predictions."""
    names = [f"{a}-{s}" for a in ARMS for s in SEEDS]
    missing = [n for n in names if n not in results or results[n].get("status") != "completed"]
    if missing:
        raise SystemExit(f"no completed gate result for {missing}")
    shared = {json.dumps([results[n]["common"]["sha256"], results[n]["menus"]["common"]["menu"],
                          results[n]["gate_texts"]]) for n in names}
    if len(shared) != 1:
        raise SystemExit("the nine results do not share one common menu and one set of gate texts")
    # A seed that did not learn counts as a failure for its arm.
    ok = {(a, s, m): results[f"{a}-{s}"]["learned"] and results[f"{a}-{s}"]["menus"][m]["passed"]
          for a in ARMS for s in SEEDS for m in ("own", "common")}
    count = {a: {m: sum(ok[a, s, m] for s in SEEDS) for m in ("own", "common")} for a in ARMS}
    common_able = results[names[0]]["menus"]["common"]["gate_ability"]["gate_able"]
    if not common_able:
        causal = "not possible: the common menu keeps fewer than 2 of the 3 gate positives"
    elif count["n"]["common"] >= 2 and count["b"]["common"] == 0:
        causal = "pattern holds: N at 2 or 3 of 3 and B at 0 of 3 on the common menu"
    else:
        causal = "withheld"
    semi = {a: sum(clean_fired(results[f"{a}-{s}"], "clean-12", "d_semicolon") for s in SEEDS) for a in ARMS}
    return dict(
        passed=ok_table(ok), seeds_passing=count, causal=causal,
        ship=dict(nc_own_passing=count["nc"]["own"], proceeds_to_t=count["nc"]["own"] == 3,
                  checkpoint=f"nc-{SHIP_SEED}" if count["nc"]["own"] == 3 else None),
        clean12_d_semicolon_fired=semi,
        predictions={
            "1": count["b"]["own"] == 0 and count["b"]["common"] == 0,
            "4": semi["n"] >= 2,
            "5": semi["nc"] <= 1,
        })


def ok_table(ok: dict) -> dict:
    return {f"{a}-{s}": {m: ok[a, s, m] for m in ("own", "common")} for a in ARMS for s in SEEDS}


def run_checkpoint(args) -> int:
    ct.check_against_doc()
    name = args.step4.stem.removeprefix("step4-")
    arm, _, seed = name.rpartition("-")
    out = args.out_dir / f"{name}.json"
    if out.exists():
        raise SystemExit(f"refused: {out} exists; each checkpoint's gate runs once")
    step = json.loads(args.step4.read_text())
    run_dir = Path(step["run_dir"])
    if arm not in RUN_NAMES or str(step["seed"]) != seed or run_dir.name != RUN_NAMES[arm].format(seed=seed):
        raise SystemExit(f"refused: {args.step4.name} names {name}, but Step 4 read {run_dir.name} "
                         f"at seed {step['seed']}")
    marker = run_dir / RESERVED
    if marker.exists():
        raise SystemExit(f"refused: {marker} exists; this checkpoint's gate has already run")
    if sha256(args.scored) != step["input"]["sha256"]:
        raise SystemExit(f"refused: {args.scored} is not the scored file {args.step4} read")
    ckpt = run_dir / "best.pt"
    if sha256(ckpt) != step["checkpoint_sha256"]:
        raise SystemExit(f"refused: {ckpt} is not the checkpoint Step 4 scored")
    heads = step["menu"]
    start = next(e for e in map(json.loads, (run_dir / "log.jsonl").read_text().splitlines())
                 if e["event"] == "start")
    if heads != ta.menu_from_manifest() or start["menu"] != heads:
        raise SystemExit("refused: the head order differs between the manifest, the run and Step 4")
    common = json.loads(args.common.read_text())
    check_common(common, str(run_dir), step["final_menu"])
    audit = json.loads((args.audit / "result.json").read_text())
    texts = gate_texts(PHASE1_GATE, audit["clean"]["survived"], ct.NEW_CLEAN + (ct.codex_clean() or []))
    inputs = [str(p.resolve().relative_to(ROOT)) for p in (args.step4, args.common)
              if p.resolve().is_relative_to(ROOT)]
    code = code_state(ROOT, CODE + inputs)
    if code["dirty"]:
        raise SystemExit(f"refused: commit these before the gate runs: {code['dirty']}")

    import torch
    device = torch.cuda.get_device_name(args.device)
    if torch.version.hip or device != DEVICE_NAME:
        raise SystemExit(f"refused: the gate runs on the {DEVICE_NAME} under CUDA, not {device}")
    model = ta.Arm(step["arm"], len(heads), args.device, step["recipe"])
    state = torch.load(ckpt)
    got = model.load_state_dict(state, strict=False)
    absent = sorted(set(state) - set(model.state_dict()))
    if got.unexpected_keys or absent:
        raise SystemExit(f"refused: checkpoint does not fit: unexpected {got.unexpected_keys[:3]} absent {absent[:3]}")
    model.eval()

    @torch.no_grad()
    def unit_logits(units: list[str]) -> list[list[float]]:
        return model.unit_logits(units).cpu().tolist()

    judge = Judge(unit_logits, heads, step["temperatures"], step["thresholds"])
    det = determinism(unit_logits)
    par = parity(judge, ta.load_rows("val"), json.loads(args.scored.read_text()))
    print(f"{name}: judge on {device} (cuda); determinism max |dz| {det['max_abs_dz']} over {det['cells']} "
          f"cells, finite {det['finite']}; parity {par['differing']} of {par['cells']} cells differ", flush=True)
    header = dict(checkpoint=name, run_dir=str(run_dir), checkpoint_sha256=step["checkpoint_sha256"],
                  step4=dict(file=str(args.step4), sha256=sha256(args.step4)),
                  scored=dict(file=str(args.scored), sha256=step["input"]["sha256"]),
                  common=dict(file=str(args.common), sha256=sha256(args.common)), code=code,
                  learned=step["measured"]["learned"], device=device, torch=torch.__version__,
                  determinism=det, parity=par, gate_texts=[c for c, _, _ in texts])
    args.out_dir.mkdir(parents=True, exist_ok=True)
    if not (det["ok"] and par["ok"]):
        with (args.out_dir / "refusals.jsonl").open("a") as fh:
            fh.write(json.dumps({**header, "status": "refused", "at": now()}) + "\n")
        print("refused: the instrument failed its checks; the gate was not run", file=sys.stderr)
        return 2

    started = now()
    with out.open("x") as fh:         # the reservations: a second invocation is refused from here on
        fh.write(json.dumps({**header, "status": "running", "started": started}, indent=1) + "\n")
    with marker.open("x") as fh:
        fh.write(json.dumps(dict(result=str(out), started=started)) + "\n")
    menus = {"own": step["final_menu"], "common": common["common_menu"]}
    res: dict[str, dict] = {}
    try:
        for text in [t for _, t, _ in texts] + [s[2] for s in PHASE1_SPAN]:
            judge.scored(text)        # in this thread, as parity was; the gate's pool reads the cache
        for m, menu in menus.items():
            res[m] = read_menu(judge, m, menu, texts)
            print(f"--- {name}, {m} menu ({len(menu)}): {'PASS' if res[m]['passed'] else 'FAIL'}", flush=True)
    except BaseException:
        write_json(out, {**header, "status": "crashed", "started": started, "ended": now(),
                         "traceback": traceback.format_exc(),
                         "menus": {m: {k: v for k, v in r.items() if k != "rows"} for m, r in res.items()},
                         "rows": judge.log, "errors": judge.errors})
        raise
    rows = [row for r in res.values() for row in r.pop("rows")]
    (args.out_dir / f"{name}.log.jsonl").write_text("".join(json.dumps(r) + "\n" for r in rows))
    write_json(out, {**header, "status": "completed", "started": started, "ended": now(), "menus": res})
    return 0


def main() -> int:
    ap = argparse.ArgumentParser()
    mode = ap.add_mutually_exclusive_group(required=True)
    mode.add_argument("--step4", type=Path, help="one checkpoint's step4.py output")
    mode.add_argument("--summary", type=Path, help="a directory of this script's results")
    ap.add_argument("--scored", type=Path, help="the score_run.py output --step4 read")
    ap.add_argument("--common", type=Path, help="step4.py --common's output")
    ap.add_argument("--audit", type=Path, default=HERE / "audit")
    ap.add_argument("--out-dir", type=Path, help="with --step4: where the result is written")
    ap.add_argument("--out", type=Path, help="with --summary: the summary file")
    ap.add_argument("--device", default="cuda:0")
    args = ap.parse_args()
    if args.summary:
        if args.out is None:
            ap.error("--summary needs --out")
        results = {p.stem: json.loads(p.read_text()) for p in sorted(args.summary.glob("*-*.json"))}
        res = summarize(results)
        write_json(args.out, res)
        print(json.dumps({k: res[k] for k in ("seeds_passing", "causal", "ship", "predictions")}, indent=1))
        return 0
    if args.scored is None or args.common is None or args.out_dir is None:
        ap.error("--step4 needs --scored, --common and --out-dir")
    return run_checkpoint(args)


if __name__ == "__main__":
    sys.exit(main())
