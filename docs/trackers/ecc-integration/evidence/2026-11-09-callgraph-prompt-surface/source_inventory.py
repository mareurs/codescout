"""Bind reviewed source surfaces to committed bytes and retained native delivery."""
import hashlib
import json
from pathlib import Path
import re
import subprocess
HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[4]
COMMIT="6586622a30f0d790db17b1bf234aa7962837f510"
paths=["src/prompts/source.md","src/prompts/builders.rs",".codescout/system-prompt.md","src/prompts/guides/symbol-navigation.md","src/tools/symbol/call_graph/mod.rs"]
def blob(path):return subprocess.check_output(["git","show",f"{COMMIT}:{path}"],cwd=ROOT)
files={path:blob(path) for path in paths}
preflight=json.loads((HERE/"base/preflight.json").read_text())
static=files[paths[0]].decode().split("<!-- @surface server_instructions -->",1)[1].split("<!-- @end -->",1)[0].strip()
assert static==preflight["static_prompt"]
assert len(static)<=1900
nav="## Navigation Strategy"+files[paths[2]].decode().split("## Navigation Strategy",1)[1].split("\n## ",1)[0]
assert nav==preflight["project_navigation"]
guide=files[paths[3]].decode().strip()
delivery=[]
for directory in sorted((HERE/"base").glob("[0-9][0-9]-*")):
    found=False
    for line in (directory/"events.jsonl").read_text().splitlines():
        event=json.loads(line)
        if event.get("type")!="item.completed":continue
        item=event.get("item",{})
        if item.get("type")!="mcp_tool_call" or item.get("server")!="codescout":continue
        for block in (item.get("result") or {}).get("content",[]):
            if block.get("type")=="text" and guide in block.get("text",""):found=True
    delivery.append({"run":directory.name,"exact_symbol_navigation_guide_present":found})
assert all(row["exact_symbol_navigation_guide_present"] for row in delivery)
scan=re.compile(r"formerly|used to|was removed|since [0-9a-f]{7,40}|\b202[0-9]-\d{2}-\d{2}\b|BUG-|LIMIT-",re.I)
literal_hits={}
for path in [paths[0],paths[2],paths[3]]:
    literal_hits[path]=[{"line":i,"text":line} for i,line in enumerate(files[path].decode().splitlines(),1) if scan.search(line)]
report=dict(commit=COMMIT,tree=subprocess.check_output(["git","rev-parse",COMMIT+"^{tree}"],cwd=ROOT,text=True).strip(),files={path:dict(sha256=hashlib.sha256(data).hexdigest(),bytes=len(data)) for path,data in files.items()},static_characters=len(static),static_cap=1900,static_equals_actual_initialize_prefix=True,navigation_equals_fixture_AGENTS_section=True,guide_delivery=delivery,literal_history_pattern=scan.pattern,literal_history_hits=literal_hits,scope="Literal scans of source.md, stored prompt and symbol-navigation guide plus manual review of call_graph-related builder/description/schema/trigger bodies. Not a semantic audit of all guides, refusal strings or tool descriptions. Contributor comments/tests and immutable evidence preserve history.")
(HERE/"source-inventory.json").write_text(json.dumps(report,indent=2)+"\n")
print(json.dumps(dict(static_characters=len(static),guide_present_runs=sum(r["exact_symbol_navigation_guide_present"] for r in delivery),literal_hits=literal_hits)))
