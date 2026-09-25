"""Diagnostic (val only, already consumed by Stage 3): do rule heads fire on OTHER rules' texts?

For each val row, score its target unit with all 14 heads. A head's own-rule rows are what it
was trained/thresholded on; the other 13 rules' rows are texts about something else, which no
training row ever labelled for this head (masked cells). Reports, per head, the firing rate at
its precision threshold on (a) own-rule negatives and (b) other-rule rows, positives and
negatives alike. Not a rescue: nothing here changes the Stage 4 outcome."""
import json, math, sys
sys.path.insert(0, "scripts")
import importlib.util
spec = importlib.util.spec_from_file_location("lt", "scripts/phase1-local-trained.py")
lt = importlib.util.module_from_spec(spec); spec.loader.exec_module(lt)
import torch

arm = sys.argv[1]
model, menu, temps, thr = lt.load_arm(arm, "cuda:0")
ri = {r: i for i, r in enumerate(menu)}
val = lt.ta.load_rows("val")
own_neg = {r: [0, 0] for r in menu}
other = {r: [0, 0] for r in menu}
with torch.no_grad():
    for row in val:
        z = model.unit_logits(lt.ta.segment(row["text"]))[row["target"]].cpu().tolist()
        for h in menu:
            fire = lt.sigmoid(z[ri[h]] / temps[h]["T"]) >= thr[h]["precision_t"]
            if h == row["rule"]:
                if row["label"] == 0:
                    own_neg[h][0] += fire; own_neg[h][1] += 1
            else:
                other[h][0] += fire; other[h][1] += 1
print(f"{'head':22s} {'own-rule negatives fired':>26s} {'other-rules texts fired':>26s}")
tot_o = [0, 0]
for h in menu:
    a, b = own_neg[h], other[h]
    tot_o[0] += b[0]; tot_o[1] += b[1]
    print(f"{h:22s} {a[0]:>12d}/{a[1]:<4d} ({a[0]/a[1]:.2f})   {b[0]:>12d}/{b[1]:<4d} ({b[0]/b[1]:.2f})")
print(f"all heads, other-rule texts: {tot_o[0]}/{tot_o[1]} ({tot_o[0]/tot_o[1]:.2f})")
