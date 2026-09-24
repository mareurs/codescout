"""Render the operator's 40-row blind sample as one markdown file (no labels, no hints).

    python3 render_operator_packet.py <labels_dir> <out.md>
"""
import json, pathlib, sys

d, out = pathlib.Path(sys.argv[1]), pathlib.Path(sys.argv[2])
menu = json.loads((d / "menu.json").read_text())
rows = [json.loads(l) for l in (d / "operator-sample.jsonl").read_text().splitlines()]
parts = ["# Blind labelling sample: 40 rows\n",
         "For each row, give ONE label: a rule key below, `not-a-violation`, `not-a-pair` or "
         "`unsure`. Judge the ORIGINAL sentence by the rule's LAW (the spec is guidance). "
         "Rules are in `docs/evals/data/2026-09-24-rule-tell/stage2/label-instruction.md`.\n",
         "## Rules\n"]
for k, v in menu.items():
    parts.append(f"- **`{k}`**: {v['law']}  \n  *spec:* {v['spec']}")
parts.append("\n## Rows\n")
for n, r in enumerate(rows, 1):
    parts.append(f"### {n}. row id {r['id']}\n")
    parts.append(f"**Original sentence:** {r['positive']}\n")
    if r.get("twin"):
        parts.append(f"**Corrected to:** {r['twin']}\n")
    if r.get("note"):
        parts.append(f"**Correction note appended:** {r['note']}\n")
    parts.append(f"**Commit subject:** {r['subject']}\n")
    parts.append(f"<details><summary>context before the correction</summary>\n\n{r['context_before']}\n\n</details>\n")
    parts.append("**Your label:** \n")
out.write_text("\n".join(parts))
print(f"wrote {out} ({len(rows)} rows)")
