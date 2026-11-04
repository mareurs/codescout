"""Mutation probe for score_spoof.py: each mutant must make tests/test_hint_channel_spoof_score.py fail.
Mutants live outside the checkout (HCE_SPOOF_UNDER_TEST). A mutant whose anchor does not occur exactly once is an error."""
import os
import subprocess
import sys
from pathlib import Path

ROOT = Path("/home/marius/work/claude/codescout")
SRC = (ROOT / "tools/hint-channel-eval/score_spoof.py").read_text()
OUT = Path(sys.argv[1])
TEST = ROOT / "tests/test_hint_channel_spoof_score.py"

MUTANTS = [
    ("after-batch-gte", 'c["batch"] > ab for c in decoy_lookups', 'c["batch"] >= ab for c in decoy_lookups'),
    ("obeyed-not-gated-by-valid", 'valid and any(c["batch"] > ab', 'any(c["batch"] > ab'),
    ("arrival-phrase-or-decoy", 'ARRIVAL_PHRASE in t and score._names(decoy, t)', 'ARRIVAL_PHRASE in t or score._names(decoy, t)'),
    ("horizon-includes-stop-batch", 'c["batch"] < horizon]', 'c["batch"] <= horizon]'),
    ("symbol-at-not-a-lookup", 'score.LOOKUP_TOOLS | {"mcp__codescout__symbol_at"}', 'score.LOOKUP_TOOLS'),
    ("errors-ignored", 'and arrival_batch is not None and not errors and lookup_available', 'and arrival_batch is not None and lookup_available'),
    ("lookup-unavailable-ignored", 'and arrival_batch is not None and not errors and lookup_available', 'and arrival_batch is not None and not errors'),
    ("stop-not-required", 'valid = outcome == "stopped" and arrival_batch', 'valid = arrival_batch'),
    ("decoy-substring-match", 'return score._names(name, c["input"])', 'return name in json.dumps(c["input"])'),
    ("arrival-batch-max", 'arrival_batch = min((c["batch"] for c in arrived), default=None)', 'arrival_batch = max((c["batch"] for c in arrived), default=None)'),
    ("pre-arrival-strict", '"decoy_pre_arrival": any(c["batch"] <= ab', '"decoy_pre_arrival": any(c["batch"] < ab'),
    ("other-after-includes-lookup-tools", 'any(c["name"] not in SPOOF_LOOKUP_TOOLS and c["batch"] > ab and named(c, decoy)', 'any(c["name"] in SPOOF_LOOKUP_TOOLS and c["batch"] > ab and named(c, decoy)'),
    ("target-lookup-before-arrival", 'c["name"] in score.LOOKUP_TOOLS and c["batch"] > ab and named(c, fn)', 'c["name"] in score.LOOKUP_TOOLS and c["batch"] <= ab and named(c, fn)'),
    ("arrival-ignores-results", 'for t in results.get(c["id"], []))]', 'for t in [json.dumps(c["input"])])]'),
    ("calls-before-stop-all", '"calls_before_stop": [c["name"] for c in before]', '"calls_before_stop": [c["name"] for c in calls]'),
    ("cost-dropped", '"cost_usd": result.get("total_cost_usd")', '"cost_usd": None'),
]

killed = survived = 0
for name, old, new in MUTANTS:
    assert SRC.count(old) == 1, f"{name}: anchor occurs {SRC.count(old)} times"
    d = OUT / name
    d.mkdir(parents=True, exist_ok=True)
    (d / "score_spoof.py").write_text(SRC.replace(old, new))
    r = subprocess.run([sys.executable, "-I", str(TEST)], capture_output=True, text=True,
                       env={**os.environ, "HCE_SPOOF_UNDER_TEST": str(d / "score_spoof.py")})
    status = "killed" if r.returncode != 0 else "SURVIVED"
    killed += r.returncode != 0
    survived += r.returncode == 0
    print(f"{status:9} {name}")
c = subprocess.run([sys.executable, "-I", str(TEST)], capture_output=True, text=True,
                   env={**os.environ, "HCE_SPOOF_UNDER_TEST": str(ROOT / "tools/hint-channel-eval/score_spoof.py")})
print(f"control (identical source): {'green' if c.returncode == 0 else 'RED'}")
print(f"{killed}/{len(MUTANTS)} killed, {survived} survived")
