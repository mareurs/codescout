"""Stage 3 of the phase-1 pre-registration: train one local arm (L1-MBERT or L2-QWEN).

What this implements, each point from `docs/evals/phase1-local-classifier-preregistration.md`
§ Stage 3 and the Stage 3 execution amendment:

- Input: the draft's segmenter units (`stage2/segment.py`), each followed by a marker token.
  The marker's final hidden state feeds one linear layer -> one logit per menu rule.
- Loss: per-(sentence, rule) BCE with positive-class weighting. A frozen row labels exactly
  one cell (its `target` unit, its `rule`); every other cell is unknown and masked.
- Selection: the epoch with the lowest validation-fold loss. Nothing else is chosen on val
  except the per-rule thresholds, after training.
- Calibration: one temperature per rule head, fitted on the calibration fold only.
- It reads train, val and cal. It refuses any other frozen file (amendment 1: T and the
  T-syn sets are read only after every choice is fixed).

Run (CUDA):  ~/work/claude/jevk5/.venv/bin/python -u train_arm.py --arm qwen --out <dir>
Run (ROCm):  ~/work/claude/rule-tell-rocm/.venv/bin/python -u train_arm.py --arm mbert --out <dir>
"""
import argparse
import json
import math
import random
import sys
import time
from dataclasses import dataclass
from pathlib import Path

import torch
import torch.nn as nn
import torch.nn.functional as F

HERE = Path(__file__).resolve().parent
STAGE2 = HERE.parent / "stage2"
FROZEN = STAGE2 / "frozen"
sys.path.insert(0, str(STAGE2))
from segment import segment  # noqa: E402

READABLE = {"train", "val", "cal"}          # amendment 1: nothing else before choices are fixed
SEED = 20260935
PERMUTE_SEED = 20260938     # the permutation null's label shuffle (diagnostic only)

# Fixed before any run; registered in the Stage 3 execution amendment. Not tuned.
ARMS = {
    "mbert": dict(model="answerdotai/ModernBERT-large", marker="[SEP]", max_len=8192,
                  epochs=5, lr_body=3e-5, lr_head=1e-3, lora=False),
    "qwen": dict(model="alibiserikbay/JevK5", marker="<|box_end|>", max_len=None,
                 epochs=3, lr_body=2e-4, lr_head=1e-3, lora=True),
}
ACCUM = 16            # effective batch 16 rows (batch 1 x 16 accumulation)
WARMUP = 0.06
WEIGHT_DECAY = 0.01
CLIP = 1.0
T_BOUNDS = (0.25, 10.0)     # per-rule temperature search range; a fit at a bound is reported
PREC_TARGET = 0.9           # precision-oriented threshold (L1/L2 standalone)
REC_TARGET = 0.9            # C1 first-stage threshold

# Training recipes. "phase1" is phase 1's registered recipe and the default; its values come from
# ARMS and the constants above, and every other key is off, so phase-1 runs reproduce exactly.
# The others are registered in the phase-1b pre-registration, Stage 1, before they run. None of
# their values was tuned; each comes from docs/research/2026-09-25-phase1-training-research-synthesis.md.
RECIPES = {
    "phase1": dict(lr_body=None, lr_head=None, epochs=None, accum=ACCUM, warmup=WARMUP,
                   space_fix=False, norm_feature=False, pair_windows=False, sep_clip=False),
    # head at the body's lr; 10% warmup; 1,120 optimizer steps (5 epochs at accumulation 8);
    # both texts of a pair in one accumulation window; head and body clipped separately.
    "s1-r1": dict(lr_body=1e-4, lr_head=1e-4, epochs=5, accum=8, warmup=0.10,
                  space_fix=True, norm_feature=False, pair_windows=True, sep_clip=True),
    # s1-r1 plus a parameter-free LayerNorm on the marker feature before the head.
    "s1-r2": dict(lr_body=1e-4, lr_head=1e-4, epochs=5, accum=8, warmup=0.10,
                  space_fix=True, norm_feature=True, pair_windows=True, sep_clip=True),
}


def load_rows(name: str) -> list[dict]:
    if name not in READABLE:
        raise SystemExit(f"refused: Stage 3 may not read frozen/{name}.jsonl (amendment 1)")
    return [json.loads(line) for line in (FROZEN / f"{name}.jsonl").open()]


def menu_from_manifest() -> list[str]:
    man = json.loads((FROZEN / "freeze-manifest.json").read_text())
    return sorted(man["files"]["train"]["per_rule"])


@dataclass
class Encoded:
    ids: list[int]
    markers: list[int]      # token position of the marker after each unit
    units: list[str]


def encode_units(units: list[str], tok, marker_id: int, bos: list[int], eos: list[int],
                 space_fix: bool = False, first_index: int = 0) -> Encoded:
    """Units joined by marker tokens. With `space_fix`, every unit after the draft's first is
    tokenised with its leading space, as it appears in running text. Without it, a sentence starts
    `Nobody` where running text has `ĠNobody`
    (docs/issues/2026-09-25-encode-units-drops-sentence-leading-space.md). Phase 1's registered
    recipe ran without it. `first_index` is the draft index of `units[0]`, for windows."""
    ids, markers = list(bos), []
    for k, u in enumerate(units):
        text = " " + u if space_fix and first_index + k > 0 else u
        ids.extend(tok(text, add_special_tokens=False)["input_ids"])
        markers.append(len(ids))
        ids.append(marker_id)
    ids.extend(eos)
    return Encoded(ids, markers, units)


def chunks(units: list[str], tok, marker_id, bos, eos, max_len: int | None,
           space_fix: bool = False) -> list[tuple[int, Encoded]]:
    """(first unit index, encoding) windows. One window when the draft fits; otherwise windows of
    whole units with half-window overlap, so a unit near a cut is also seen with context on both
    sides. A unit's score is its maximum over the windows containing it (Stage 3, L1)."""
    whole = encode_units(units, tok, marker_id, bos, eos, space_fix)
    if max_len is None or len(whole.ids) <= max_len:
        return [(0, whole)]

    def text(i: int) -> str:
        return " " + units[i] if space_fix and i > 0 else units[i]

    lens = [len(tok(text(i), add_special_tokens=False)["input_ids"]) + 1 for i in range(len(units))]
    budget = max_len - len(bos) - len(eos)
    out, start = [], 0
    while start < len(units):
        end, used = start, 0
        while end < len(units) and used + lens[end] <= budget:
            used += lens[end]
            end += 1
        if end == start:                      # a single unit longer than the window: truncate it
            ids = tok(text(start), add_special_tokens=False)["input_ids"][: budget - 1]
            enc = Encoded(list(bos) + ids + [marker_id] + list(eos), [len(bos) + len(ids)], [units[start]])
            out.append((start, enc))
            start += 1
            continue
        out.append((start, encode_units(units[start:end], tok, marker_id, bos, eos, space_fix, start)))
        if end == len(units):
            break
        start = max(start + 1, start + (end - start) // 2)
    return out


class Arm(nn.Module):
    def __init__(self, arm: str, n_rules: int, device: str, recipe: str = "phase1"):
        super().__init__()
        from transformers import AutoModel, AutoModelForCausalLM, AutoTokenizer
        cfg = ARMS[arm]
        self.arm, self.cfg = arm, cfg
        self.space_fix = RECIPES[recipe]["space_fix"]
        self.norm_feature = RECIPES[recipe]["norm_feature"]
        self.tok = AutoTokenizer.from_pretrained(cfg["model"])
        self.marker_id = self.tok.convert_tokens_to_ids(cfg["marker"])
        assert self.marker_id is not None and self.marker_id != self.tok.unk_token_id, cfg["marker"]
        if arm == "mbert":
            self.body = AutoModel.from_pretrained(cfg["model"], dtype=torch.float32)
            self.bos, self.eos = [self.tok.cls_token_id], []
            hidden = self.body.config.hidden_size
        else:
            from peft import LoraConfig, get_peft_model
            lm = AutoModelForCausalLM.from_pretrained(cfg["model"], dtype=torch.bfloat16)
            base = lm.model                       # drop lm_head: no vocabulary logits are needed
            base.gradient_checkpointing_enable(gradient_checkpointing_kwargs={"use_reentrant": False})
            base.enable_input_require_grads()
            lcfg = LoraConfig(r=16, lora_alpha=32, lora_dropout=0.05, target_modules="all-linear", bias="none")
            self.body = get_peft_model(base, lcfg)
            self.bos, self.eos = [], []
            hidden = base.config.hidden_size
        self.head = nn.Linear(hidden, n_rules)
        nn.init.zeros_(self.head.weight)      # every cell starts at p = 0.5 (fixed before any val read)
        nn.init.zeros_(self.head.bias)
        self.to(device)
        self.device = device

    def logits_for(self, enc: Encoded) -> torch.Tensor:
        """[n_units, n_rules] logits for one window."""
        ids = torch.tensor([enc.ids], device=self.device)
        with torch.autocast(device_type="cuda", dtype=torch.bfloat16):
            out = self.body(input_ids=ids, attention_mask=torch.ones_like(ids))
        h = out.last_hidden_state[0, enc.markers].float()
        if self.norm_feature:          # parameter-free: the head reads a unit-scale, centred feature
            h = F.layer_norm(h, (h.shape[-1],))
        return self.head(h)

    def unit_logits(self, units: list[str]) -> torch.Tensor:
        """[n_units, n_rules]: each unit's maximum over the windows that contain it."""
        best = None
        for start, enc in chunks(units, self.tok, self.marker_id, self.bos, self.eos, self.cfg["max_len"],
                                 self.space_fix):
            lg = self.logits_for(enc)
            if best is None:
                best = torch.full((len(units), lg.shape[1]), -math.inf, device=lg.device)
            idx = torch.arange(start, start + lg.shape[0], device=lg.device)
            best[idx] = torch.maximum(best[idx], lg)
        return best

    def trainable(self):
        return [p for p in self.body.parameters() if p.requires_grad], list(self.head.parameters())

    def state_to_save(self) -> dict:
        keep = {k: v.detach().cpu().clone() for k, v in self.named_parameters() if v.requires_grad}
        return keep


def cell_logit(model: Arm, row: dict, rule_idx: dict) -> torch.Tensor:
    units = segment(row["text"])
    return model.unit_logits(units)[row["target"], rule_idx[row["rule"]]]


def pos_weights(train: list[dict], menu: list[str]) -> dict[str, float]:
    w = {}
    for r in menu:
        pos = sum(1 for x in train if x["rule"] == r and x["label"] == 1)
        neg = sum(1 for x in train if x["rule"] == r and x["label"] == 0)
        w[r] = neg / pos
    return w


def weighted_bce(logit: torch.Tensor, label: int, pw: float) -> torch.Tensor:
    y = torch.tensor(float(label), device=logit.device)
    return F.binary_cross_entropy_with_logits(logit, y, pos_weight=torch.tensor(pw, device=logit.device))


@torch.no_grad()
def fold_logits(model: Arm, rows: list[dict], rule_idx: dict) -> list[float]:
    model.eval()
    return [cell_logit(model, r, rule_idx).item() for r in rows]


def fold_loss(logits: list[float], rows: list[dict], pw: dict) -> float:
    tot = 0.0
    for z, r in zip(logits, rows):
        tot += weighted_bce(torch.tensor(z), r["label"], pw[r["rule"]]).item()
    return tot / len(rows)


def fit_temperature(z: list[float], y: list[int]) -> tuple[float, bool]:
    """Minimise NLL of sigmoid(z / T) over a log-spaced grid in T_BOUNDS, then refine by golden
    section. Returns (T, at_bound). With few, separable cells the optimum runs to the lower
    bound; that is reported, never hidden."""
    zt, yt = torch.tensor(z), torch.tensor(y, dtype=torch.float32)

    def nll(logT: float) -> float:
        return F.binary_cross_entropy_with_logits(zt / math.exp(logT), yt).item()

    lo, hi = math.log(T_BOUNDS[0]), math.log(T_BOUNDS[1])
    grid = [lo + (hi - lo) * i / 200 for i in range(201)]
    k = min(range(len(grid)), key=lambda i: nll(grid[i]))
    a, b = grid[max(k - 1, 0)], grid[min(k + 1, len(grid) - 1)]
    g = (math.sqrt(5) - 1) / 2
    for _ in range(60):
        c, d = b - g * (b - a), a + g * (b - a)
        if nll(c) < nll(d):
            b = d
        else:
            a = c
    logT = (a + b) / 2
    at_bound = k in (0, len(grid) - 1)
    return math.exp(logT), at_bound


def thresholds(p: list[float], y: list[int]) -> dict:
    """Per-rule thresholds on the validation fold, over calibrated probabilities.

    precision: the smallest t with precision >= PREC_TARGET (and >= 1 true positive) when
    firing at p >= t; if no t reaches it, the t maximising F0.5 (ties -> higher t).
    recall90: the largest t with recall >= REC_TARGET."""
    cands = sorted(set(p))
    npos = sum(y)

    def stats(t):
        tp = sum(1 for pi, yi in zip(p, y) if pi >= t and yi == 1)
        fp = sum(1 for pi, yi in zip(p, y) if pi >= t and yi == 0)
        return tp, fp

    prec_t, how = None, "precision>=target"
    for t in cands:
        tp, fp = stats(t)
        if tp >= 1 and tp / (tp + fp) >= PREC_TARGET:
            prec_t = t
            break
    if prec_t is None:
        how = "max F0.5 (target unreachable)"
        best = (-1.0, 0.0)
        for t in cands:
            tp, fp = stats(t)
            prec = tp / (tp + fp) if tp + fp else 0.0
            rec = tp / npos if npos else 0.0
            f = 1.25 * prec * rec / (0.25 * prec + rec) if prec + rec else 0.0
            if (f, t) > best:
                best = (f, t)
        prec_t = best[1]
    rec_t = max(t for t in cands if stats(t)[0] / npos >= REC_TARGET)
    tp, fp = stats(prec_t)
    tpr, fpr = stats(rec_t)
    return dict(precision_t=prec_t, precision_rule=how, val_tp=tp, val_fp=fp,
                recall90_t=rec_t, val_tp_at_recall90=tpr, val_fp_at_recall90=fpr, val_pos=npos, val_n=len(y))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--arm", choices=sorted(ARMS), required=True)
    ap.add_argument("--out", type=Path, required=True, help="run directory, outside the repo")
    ap.add_argument("--device", default="cuda:0")
    ap.add_argument("--recipe", choices=sorted(RECIPES), default="phase1",
                    help="training recipe; phase1 is phase 1's registered recipe (the default)")
    ap.add_argument("--seed", type=int, default=SEED,
                    help="training seed; the registered arm uses the default. Others are diagnostics")
    ap.add_argument("--permute-labels", action="store_true",
                    help="permutation null: shuffle train labels within each rule; val/cal untouched")
    ap.add_argument("--smoke", type=int, default=0,
                    help="engineering check only: train on the first N train rows for one epoch; reads no val/cal")
    args = ap.parse_args()

    random.seed(args.seed)
    torch.manual_seed(args.seed)
    args.out.mkdir(parents=True, exist_ok=True)
    log = (args.out / "log.jsonl").open("a")

    def emit(**kw):
        kw["t"] = round(time.time(), 1)
        print(json.dumps(kw), flush=True)
        log.write(json.dumps(kw) + "\n")
        log.flush()

    menu = menu_from_manifest()
    rule_idx = {r: i for i, r in enumerate(menu)}
    train = load_rows("train")
    assert {r["rule"] for r in train} <= set(menu)
    if args.permute_labels:
        # Within each rule, so every rule keeps its positive count (and pos_weight); only the
        # link between a text and its label is destroyed.
        prng = random.Random(PERMUTE_SEED)
        for rule in menu:
            idx = [i for i, r in enumerate(train) if r["rule"] == rule]
            labels = [train[i]["label"] for i in idx]
            prng.shuffle(labels)
            for i, y in zip(idx, labels):
                train[i] = {**train[i], "label": y}
    pw = pos_weights(train, menu)
    cfg = ARMS[args.arm]
    rec = RECIPES[args.recipe]
    lr_body = rec["lr_body"] if rec["lr_body"] is not None else cfg["lr_body"]
    lr_head = rec["lr_head"] if rec["lr_head"] is not None else cfg["lr_head"]
    epochs = rec["epochs"] if rec["epochs"] is not None else cfg["epochs"]
    accum, warmup = rec["accum"], rec["warmup"]
    if args.smoke:
        train, epochs = train[: args.smoke], 1

    model = Arm(args.arm, len(menu), args.device, args.recipe)
    body_p, head_p = model.trainable()
    emit(event="start", arm=args.arm, device=args.device, backend=("rocm" if torch.version.hip else "cuda"),
         torch=torch.__version__, gpu=torch.cuda.get_device_name(args.device), menu=menu, pos_weight=pw,
         trainable_body=sum(p.numel() for p in body_p), smoke=args.smoke, seed=args.seed,
         permute_labels=args.permute_labels, cfg=cfg, recipe=args.recipe,
         resolved=dict(rec, lr_body=lr_body, lr_head=lr_head, epochs=epochs))

    opt = torch.optim.AdamW([
        {"params": body_p, "lr": lr_body, "weight_decay": WEIGHT_DECAY},
        {"params": head_p, "lr": lr_head, "weight_decay": 0.0},
    ])
    steps = math.ceil(len(train) / accum) * epochs
    warm = max(1, int(warmup * steps))
    sched = torch.optim.lr_scheduler.LambdaLR(     # linear warmup, then linear decay to 0
        opt, lambda s: (s + 1) / warm if s < warm else max(0.0, (steps - s) / max(1, steps - warm)))

    def epoch_order(ep: int) -> list[int]:
        if not rec["pair_windows"]:
            order = list(range(len(train)))
            random.Random(args.seed + ep).shuffle(order)
            return order
        # Both texts of a pair adjacent, so they share an accumulation window: their shared feature
        # then cancels in the head's gradient. Unpaired rows go last, so no pair straddles a window
        # boundary (accum is even).
        groups: dict[str, list[int]] = {}
        for i, r in enumerate(train):
            groups.setdefault(r["id"].rpartition(":")[0], []).append(i)
        pairs = [g for g in groups.values() if len(g) == 2]
        rest = [i for g in groups.values() if len(g) != 2 for i in g]
        random.Random(args.seed + ep).shuffle(pairs)
        return [i for g in pairs for i in g] + rest

    val = [] if args.smoke else load_rows("val")
    best = (math.inf, -1)
    step, win_z = 0, []
    for ep in range(epochs):
        model.train()
        order = epoch_order(ep)
        run, n = 0.0, 0
        for i, j in enumerate(order):
            r = train[j]
            z = cell_logit(model, r, rule_idx)
            loss = weighted_bce(z, r["label"], pw[r["rule"]])
            (loss / accum).backward()
            run += loss.item()
            win_z.append(z.item())
            n += 1
            if (i + 1) % accum == 0 or i + 1 == len(order):
                if rec["sep_clip"]:
                    torch.nn.utils.clip_grad_norm_(body_p, CLIP)
                    torch.nn.utils.clip_grad_norm_(head_p, CLIP)
                else:
                    torch.nn.utils.clip_grad_norm_(body_p + head_p, CLIP)
                opt.step()
                sched.step()
                opt.zero_grad(set_to_none=True)
                step += 1
                if step % 50 == 0:     # the head-step mechanism, observed rather than inferred
                    emit(event="steplog", step=step, lr_head=sched.get_last_lr()[1],
                         head_weight_norm=round(model.head.weight.norm().item(), 4),
                         mean_logit=round(sum(win_z) / len(win_z), 4),
                         mean_abs_logit=round(sum(abs(v) for v in win_z) / len(win_z), 4))
                    win_z = []
            if (i + 1) % 200 == 0:
                emit(event="progress", epoch=ep, rows=i + 1, train_loss=run / n,
                     max_mem_gb=round(torch.cuda.max_memory_allocated(args.device) / 2**30, 2))
        emit(event="epoch", epoch=ep, train_loss=run / n)
        if args.smoke:
            continue
        vz = fold_logits(model, val, rule_idx)
        vl = fold_loss(vz, val, pw)
        emit(event="val", epoch=ep, val_loss=vl)
        if vl < best[0]:
            best = (vl, ep)
            torch.save(model.state_to_save(), args.out / "best.pt")

    if args.smoke:
        emit(event="smoke-done", max_mem_gb=round(torch.cuda.max_memory_allocated(args.device) / 2**30, 2))
        return

    emit(event="selected", epoch=best[1], val_loss=best[0])
    model.load_state_dict(torch.load(args.out / "best.pt"), strict=False)
    cal = load_rows("cal")
    cz = fold_logits(model, cal, rule_idx)
    vz = fold_logits(model, val, rule_idx)

    def sig(x: float) -> float:        # phase 1's formula, guarded only where it would overflow
        try:
            return 1 / (1 + math.exp(-x))
        except OverflowError:
            return 0.0

    temps, thr = {}, {}
    for rule in menu:
        ci = [i for i, r in enumerate(cal) if r["rule"] == rule]
        T, at_bound = fit_temperature([cz[i] for i in ci], [cal[i]["label"] for i in ci])
        temps[rule] = dict(T=T, at_bound=at_bound, cal_n=len(ci), cal_pos=sum(cal[i]["label"] for i in ci))
        vi = [i for i, r in enumerate(val) if r["rule"] == rule]
        vp = [sig(vz[i] / T) for i in vi]
        thr[rule] = thresholds(vp, [val[i]["label"] for i in vi])
    (args.out / "calibration.json").write_text(json.dumps(dict(menu=menu, temperatures=temps), indent=1))
    (args.out / "thresholds.json").write_text(json.dumps(thr, indent=1))
    (args.out / "fold-logits.json").write_text(json.dumps(dict(
        val=[dict(id=r["id"], rule=r["rule"], label=r["label"], z=z) for r, z in zip(val, vz)],
        cal=[dict(id=r["id"], rule=r["rule"], label=r["label"], z=z) for r, z in zip(cal, cz)])))
    emit(event="done", selected_epoch=best[1], val_loss=best[0],
         temps_at_bound=[r for r in menu if temps[r]["at_bound"]])


if __name__ == "__main__":
    main()
