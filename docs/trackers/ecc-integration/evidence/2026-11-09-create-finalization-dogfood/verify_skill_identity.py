import hashlib
import json
from pathlib import Path
HERE=Path(__file__).resolve().parent
installed=Path("/home/marius/.codex/plugins/cache/ecc-codescout-pilot/ecc-codescout/0.1.0")
rows=[]
for name in ["ecc-spec-miner","ecc-silent-failure-review"]:
    used=(HERE/"controller-only"/(name+".txt")).read_bytes()
    data=(installed/"skills"/name/"SKILL.md").read_bytes()
    assert used==data,name
    rows.append(dict(name=name,source_snapshot_sha256=hashlib.sha256(used).hexdigest(),installed_sha256=hashlib.sha256(data).hexdigest(),same_bytes=True))
(HERE/"skill-identity.json").write_text(json.dumps(dict(installed_cache=str(installed),skills=rows,scope="Explicit body reads from adapter source match installed0.1.0 bytes; does not independently prove automatic residency/activation in the main turn."),indent=2)+"\n")
print(json.dumps(dict(identical_bodies=len(rows))))
