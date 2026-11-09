"""Re-score retained events and reproduce the review's two observed false positives."""
import copy
import hashlib
import importlib.util
import json
from pathlib import Path
HERE=Path(__file__).resolve().parent
def load(name,path):
    spec=importlib.util.spec_from_file_location(name,path)
    module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
    return module
old=load("original_score",HERE/"score-prereview.py")
new=load("hardened_score",HERE/"score.py")
def lines(path):return [json.loads(s) for s in path.read_text().splitlines() if s]
static=json.loads((HERE/"base/preflight.json").read_text())["static_prompt"]
controls=new.selftest()
runs=[]
for directory in sorted((HERE/"base").glob("[0-9][0-9]-*")):
    previous=json.loads((directory/"result.json").read_text())
    result=new.score(lines(directory/"events.jsonl"),lines(directory/"wire.jsonl"),previous["case"],static)
    assert result["delivery"]=="verified" and result["routing"] and result["correct_answer"],(directory,result)
    assert previous["metadata_match"] and previous["exit_code"]==0
    runs.append(dict(run=directory.name,**result))
assert len(runs)==13
wire=lines(HERE/"base/06-chain/wire.jsonl")
original=lines(HERE/"base/06-chain/events.jsonl")
mutations=[]
for name in ["wrong_server","endpoint_names_without_edges"]:
    events=copy.deepcopy(original)
    target=next(e["item"] for e in events if e.get("type")=="item.completed" and e.get("item",{}).get("tool")=="call_graph")
    if name=="wrong_server":target["server"]="unrelated-server"
    else:target["result"]["content"]=[{"type":"text","text":"call_graph for write_row: handle_ingest and handle_shutdown are excluded; no caller edges found."}]
    before=old.score(events,wire,"chain",static);after=new.score(events,wire,"chain",static)
    assert before["routing"] is True and after["routing"] is False,(name,before,after)
    mutations.append(dict(name=name,before=before,after=after))
report=dict(original_scorer_sha256=hashlib.sha256((HERE/"score-prereview.py").read_bytes()).hexdigest(),hardened_scorer_sha256=hashlib.sha256((HERE/"score.py").read_bytes()).hexdigest(),controls=controls,control_count=len(controls),observed_false_positive_mutations=mutations,runs=runs,summary={"chain":10,"direct":3,"all_delivery_routing_answer_pass":True},scope="Existing raw events only; no new model calls. Fixture-specific edge parser, not a general graph evaluator.")
(HERE/"rescored.json").write_text(json.dumps(report,indent=2)+"\n")
print(json.dumps(dict(controls=len(controls),mutations_before_survived=2,mutations_after_survived=0,runs=len(runs),chain=10,direct=3)))
