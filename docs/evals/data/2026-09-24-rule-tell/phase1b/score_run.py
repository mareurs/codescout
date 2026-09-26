"""Phase 1b Stage 2, Step 4's input: score one trained run's val and cal cells from its selected
checkpoint, own cells plus admitted cross cells. Every arm (B, N, NC) goes through this script, so
Step 4 (`step4.py`) reads one file shape, produced by one piece of code, over one cell set.

Why a separate pass rather than the run's own fold-logits.json:
- Arm B's checkpoints were trained in Stage 1, without --cross and before the admission file
  existed, so their fold-logits.json holds own cells only.
- The cell set must be the same for every arm: the same admission file, and the same val and cal
  counterexample rows (arm NC's), which arms B and N never saw.

Parity, checked before anything is written: every frozen row's own-cell logit must equal the run's
own fold-logits.json exactly, and so must its cross cells when the run trained with --cross. A run
trained with --cross or --extra-rows is refused unless scored with those same files. Measured
before this script existed: s1-r1-20260935's val logits recomputed to max |dz| = 0.0 on the
training GPU.

Reads val and cal, and the val and cal rows of --extra-rows. Never train, T, the T-syn sets or a
gate text.

Run: ~/work/claude/jevk5/.venv/bin/python -u score_run.py --run-dir <dir> --cross <admission.json>
         [--extra-rows <counterexamples.jsonl>] [--expect-sha256 <best.pt sha256>] --out <file>
"""
import argparse
import hashlib
import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "stage3"))
import train_arm as ta  # noqa: E402


def sha256_file(p: Path) -> str:
    h = hashlib.sha256()
    with p.open("rb") as f:
        for block in iter(lambda: f.read(1 << 20), b""):
            h.update(block)
    return h.hexdigest()


def parity(ref: dict, got: dict, frozen_ids: dict[str, set], compare_cross: bool) -> dict:
    """Compare this pass with the run's own fold-logits.json. Own cells: every frozen row must be
    in both, with an identical logit; a counterexample row is compared where both hold it. Cross
    cells: only when the run trained with this admission file, and then keyed by (id, unit, head)
    with identical key sets. Returns the counts and max |dz|; `ok` is exact equality."""
    out, ok = {}, True
    for fold in ("val", "cal"):
        mine = {r["id"]: r["z"] for r in got[fold]}
        theirs = {r["id"]: r["z"] for r in ref[fold]}
        missing = sorted(frozen_ids[fold] - set(theirs))
        common = sorted(set(mine) & set(theirs))
        dz = max((abs(mine[i] - theirs[i]) for i in common), default=0.0)
        out[fold] = dict(compared=len(common), frozen_missing_from_run=len(missing), max_abs_dz=dz)
        ok &= not missing and dz == 0.0
        if compare_cross:
            key = lambda c: (c["id"], c["unit"], c["head"])  # noqa: E731
            mc = {key(c): c["z"] for c in got[f"{fold}_cross"]}
            tc = {key(c): c["z"] for c in ref.get(f"{fold}_cross", [])}
            # Counterexample rows carry no cross cells, so the key sets must match exactly.
            same = set(mc) == set(tc)
            cdz = max((abs(mc[k] - tc[k]) for k in set(mc) & set(tc)), default=0.0)
            out[f"{fold}_cross"] = dict(compared=len(set(mc) & set(tc)), same_cells=same, max_abs_dz=cdz)
            ok &= same and cdz == 0.0
    out["ok"] = ok
    return out


def training_mismatch(start: dict, cross_sha: str, extra_sha: str | None) -> list[str]:
    """A run is scored on the cells it trained on, or refused: a run trained with --cross must be
    scored with that admission file, and a run trained with --extra-rows with that counterexample
    file. A run trained with neither may take any: arm B for both files, and arm N for the
    counterexample file."""
    problems = []
    tc, te = start.get("cross"), start.get("extra_rows")
    if tc and tc["sha256"] != cross_sha:
        problems.append(f"trained with admission file {tc['sha256']}, scored with {cross_sha}")
    if te and te["sha256"] != extra_sha:
        problems.append(f"trained with counterexample file {te['sha256']}, scored with {extra_sha}")
    return problems


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--run-dir", type=Path, required=True)
    ap.add_argument("--arm", default="qwen", choices=sorted(ta.ARMS))
    ap.add_argument("--cross", type=Path, required=True, help="Step 1's admission file")
    ap.add_argument("--extra-rows", type=Path, default=None,
                    help="arm NC's counterexample rows; only the val and cal ones are scored")
    ap.add_argument("--expect-sha256", default=None, help="refuse unless best.pt has this sha256")
    ap.add_argument("--device", default="cuda:0")
    ap.add_argument("--out", type=Path, required=True)
    args = ap.parse_args()

    import torch

    run = args.run_dir
    ckpt = run / "best.pt"
    ckpt_sha = sha256_file(ckpt)
    if args.expect_sha256 and ckpt_sha != args.expect_sha256:
        raise SystemExit(f"{ckpt} has sha256 {ckpt_sha}, not the pinned {args.expect_sha256}")
    log = [json.loads(line) for line in (run / "log.jsonl").read_text().splitlines()]
    start = next(e for e in log if e["event"] == "start")
    menu = ta.menu_from_manifest()
    if start["menu"] != menu:
        raise SystemExit(f"{run} was trained on another head order: {start['menu']}")
    recipe = start.get("recipe", "phase1")
    cross = ta.load_cross(args.cross, menu)
    extra_sha = None if args.extra_rows is None else sha256_file(args.extra_rows)
    problems = training_mismatch(start, cross.sha256, extra_sha)
    if problems:
        raise SystemExit(f"{run}: " + "; ".join(problems))

    val, cal = ta.load_rows("val"), ta.load_rows("cal")
    frozen_ids = {"val": {r["id"] for r in val}, "cal": {r["id"] for r in cal}}
    extra = ({"train": [], "val": [], "cal": []} if args.extra_rows is None
             else ta.load_extra_rows(args.extra_rows, menu, frozen_ids["val"] | frozen_ids["cal"]))

    model = ta.Arm(args.arm, len(menu), args.device, recipe)
    state = torch.load(ckpt)
    got = model.load_state_dict(state, strict=False)
    absent = sorted(set(state) - set(model.state_dict()))
    if got.unexpected_keys or absent:
        raise SystemExit(f"checkpoint does not fit: unexpected {got.unexpected_keys[:3]} absent {absent[:3]}")
    ri = {r: i for i, r in enumerate(menu)}

    scored = {}
    for fold, rows in (("val", val + extra["val"]), ("cal", cal + extra["cal"])):
        own, cr = ta.fold_eval(model, rows, ri, cross)
        scored[fold] = [dict(id=r["id"], rule=r["rule"], label=r["label"], source=r["source"], z=z)
                        for r, z in zip(rows, own)]
        scored[f"{fold}_cross"] = [dict(id=r["id"], unit=u, head=h, z=v)
                                   for r, c in zip(rows, cr) for u, h, v in c]

    ref = json.loads((run / "fold-logits.json").read_text())
    trained_cross = start.get("cross")
    # training_mismatch above guarantees a run trained with --cross used this admission file.
    check = parity(ref, scored, frozen_ids, compare_cross=bool(trained_cross))
    print(json.dumps(dict(run=str(run), parity=check)), flush=True)
    if not check["ok"]:
        print("parity failed: nothing written", file=sys.stderr)
        return 1

    doc = dict(
        run_dir=str(run), arm=args.arm, recipe=recipe, seed=start.get("seed"),
        checkpoint_sha256=ckpt_sha, menu=menu,
        cross=dict(file=str(args.cross), sha256=cross.sha256, admitted=sorted(cross.admitted),
                   masked=len(cross.masked)),
        extra_rows=(None if args.extra_rows is None else
                    dict(file=str(args.extra_rows), sha256=extra_sha,
                         scored={f: len(extra[f]) for f in ("val", "cal")})),
        trained_with=dict(cross=trained_cross, extra_rows=start.get("extra_rows")),
        parity=check, **scored)
    args.out.write_text(json.dumps(doc))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
