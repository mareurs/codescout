"""A-47 base-only current Codex routing; private fixture/profile and wire evidence."""
from pathlib import Path
import argparse,concurrent.futures,datetime,hashlib,importlib.util,json,os,shutil,subprocess,sys,tempfile
from importlib.machinery import SourceFileLoader

HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[4]
SCRATCH=Path("/home/marius/.cache/codescout-ecc-tmp")
MODEL="gpt-6.1-sol"; EFFORT="high"
SOURCE="""def write_row(value):
    return value

WRITE_HANDLER = write_row

def persist_batch(values):
    return [write_row(v) for v in values]

def flush_queue(values):
    return persist_batch(values)

def handle_ingest(values):
    return flush_queue(values)

def handle_shutdown(values):
    return flush_queue(values)

def handle_preview(values):
    return len(values)
"""
CHAIN=[
"write_row in store.py will change its signature. Which public handle_ entry points are affected through any intermediate wrappers?",
"Identify every public handle_ entry point whose call chain can reach write_row in store.py, including indirect callers.",
"What is the complete public handle_ blast radius of changing write_row in store.py? Include callers reached through wrappers.",
"Starting at write_row in store.py, determine which public handle_ functions depend on it along the incoming call chain.",
"Which public handle_ entry points need review when write_row in store.py changes, even if they do not call it directly?"
]
DIRECT=[
"Find every direct usage of write_row in store.py, including non-call value bindings. Return the enclosing function or binding names; exclude its declaration.",
"Where is write_row in store.py directly referenced as either a called function or a stored value? Return those containing function/binding names, excluding the declaration.",
"List the direct usages of write_row in store.py. Include assignments of the function itself as well as calls; exclude the definition and indirect callers."
]
def digest(data):return hashlib.sha256(data).hexdigest()
def module(name,path):
    spec=importlib.util.spec_from_loader(name,SourceFileLoader(name,str(path)))
    obj=importlib.util.module_from_spec(spec);spec.loader.exec_module(obj);return obj
helper=module("routing_mcp",HERE.parent/"2026-11-09-doc-create-dogfood/transport-helper.py.txt")
grader=module("routing_score",HERE/"score.py")
def fixture(project,nav):
    project.mkdir()
    subprocess.run(["git","init","-q",str(project)],check=True)
    (project/"store.py").write_text(SOURCE)
    cfg=project/".codescout";cfg.mkdir()
    (cfg/"project.toml").write_text('[project]\nname="routing-fixture"\nlanguages=["python"]\n')
    # Exact current generic navigation section; repo-specific paths/rules are not copied.
    (project/"AGENTS.md").write_text("# Routing fixture\n\n"+nav)
    (project/"README.md").write_text("Public entry points are functions prefixed handle_. Other functions and uppercase bindings are internal.\n")
def static_and_nav():
    text=(ROOT/"src/prompts/source.md").read_text()
    static=text.split("<!-- @surface server_instructions -->",1)[1].split("<!-- @end -->",1)[0].strip()
    stored=(ROOT/".codescout/system-prompt.md").read_text()
    nav="## Navigation Strategy"+stored.split("## Navigation Strategy",1)[1].split("\n## ",1)[0]
    return static,nav
def preflight(binary,static,nav,out):
    with tempfile.TemporaryDirectory(prefix="routing-preflight-",dir=SCRATCH) as tmp:
        root=Path(tmp);project=root/"project";fixture(project,nav)
        with helper.McpProbe(binary,project,env=helper.isolated_server_env(root/"state")) as probe:
            tools=probe.request("tools/list",{})
            result=probe.request("tools/call",{"name":"call_graph","arguments":{"symbol":"write_row","path":"store.py","direction":"callers","max_depth":3,"workspace":str(project)}},timeout=90)
            refs=probe.request("tools/call",{"name":"references","arguments":{"symbol":"write_row","path":"store.py","workspace":str(project)}},timeout=90)
            records=probe.responses
        assert any(r.get("result",{}).get("instructions","").startswith(static) for r in records),"compiled static slice differs"
        graph=result["content"][0]["text"]; reference=refs["content"][0]["text"]
        assert all(x in graph for x in ["handle_ingest","handle_shutdown","persist_batch","flush_queue"]),graph
        assert "handle_preview" not in graph,graph
        assert "WRITE_HANDLER" in reference and "write_row(v)" in reference,reference
        (out/"preflight.json").write_text(json.dumps({"responses":records,"fixture_source":SOURCE,"static_prompt":static,"project_navigation":nav},indent=2)+"\n")
        return digest(json.dumps(sorted([t for t in tools["tools"] if t["name"] in ("call_graph","references")],key=lambda t:t["name"]),sort_keys=True).encode())
def run(index,kind,prompt,binary,static,nav,metadata_hash,out):
    run_out=out/f"{index:02d}-{kind}";run_out.mkdir()
    started=datetime.datetime.now(datetime.timezone.utc).isoformat()
    with tempfile.TemporaryDirectory(prefix=f"routing-{index:02d}-",dir=SCRATCH) as tmp:
        root=Path(tmp);project=root/"project";fixture(project,nav)
        home=root/"codex";home.mkdir(mode=0o700)
        original=Path(os.environ.get("CODEX_HOME",str(Path.home()/".codex")))
        shutil.copy2(original/"auth.json",home/"auth.json");(home/"auth.json").chmod(0o600)
        state=helper.isolated_server_env(root/"state")
        config='model='+json.dumps(MODEL)+'\nmodel_reasoning_effort='+json.dumps(EFFORT)+'\ncli_auth_credentials_store="file"\n[mcp_servers.codescout]\ncommand='+json.dumps(sys.executable)+'\nargs='+json.dumps([str(HERE/"wire_proxy.py"),str(binary),str(project),str(run_out/"wire.jsonl")])+'\n[mcp_servers.codescout.env]\n'
        for key in ("LIBRARIAN_DB","LIBRARIAN_WORKSPACE","CODESCOUT_ARTIFACT_BACKEND","XDG_DATA_HOME","XDG_STATE_HOME","XDG_CONFIG_HOME"):
            config+=key+"="+json.dumps(state[key])+"\n"
        (home/"config.toml").write_text(config)
        task=prompt+'\nUse codescout navigation to verify the answer. Stay read-only within this project; do not run behavior tests or edit files. Stop after the verified answer, within eight navigation calls. Return only JSON {"symbols":[...]} with the exact requested names.'
        (run_out/"task.txt").write_text(task+"\n")
        env=dict(os.environ,CODEX_HOME=str(home),TMPDIR=str(SCRATCH))
        cmd=["codex","exec","--ephemeral","--json","--sandbox","read-only","--skip-git-repo-check","--cd",str(project),"-"]
        try:
            r=subprocess.run(cmd,input=task,text=True,capture_output=True,cwd=project,env=env,timeout=300)
            (run_out/"events.jsonl").write_text(r.stdout);(run_out/"stderr.txt").write_text(r.stderr)
            events=[]
            for line in r.stdout.splitlines():
                try:events.append(json.loads(line))
                except ValueError:pass
            wire=[json.loads(s) for s in (run_out/"wire.jsonl").read_text().splitlines()] if (run_out/"wire.jsonl").exists() else []
            scored=grader.score(events,wire,kind,static)
            sets=[x["result"]["tools"] for x in wire if x.get("method")=="tools/list" and "tools" in (x.get("result") or {})]
            metadata_match=any(digest(json.dumps(sorted([t for t in ts if t["name"] in ("call_graph","references")],key=lambda t:t["name"]),sort_keys=True).encode())==metadata_hash for ts in sets)
            record={"index":index,"case":kind,"model":MODEL,"effort":EFFORT,"exit_code":r.returncode,"metadata_match":metadata_match,"profile_sha256":digest(config.encode()),"fixture_sha256":digest(SOURCE.encode()),"navigation_sha256":digest(nav.encode()),"started":started,"finished":datetime.datetime.now(datetime.timezone.utc).isoformat(),**scored}
        except subprocess.TimeoutExpired:
            record={"index":index,"case":kind,"error":"timeout","delivery":"unbound","routing":None,"correct_answer":None}
        (run_out/"result.json").write_text(json.dumps(record,indent=2)+"\n")
        print(json.dumps(record),flush=True)
        return record
def main():
    p=argparse.ArgumentParser();p.add_argument("--binary",type=Path,required=True);p.add_argument("--output",type=Path,required=True);p.add_argument("--smoke",action="store_true");a=p.parse_args()
    a.output.mkdir(parents=True,exist_ok=False)
    controls=grader.selftest();(a.output/"checker-controls.json").write_text(json.dumps(controls,indent=2)+"\n")
    static,nav=static_and_nav()
    metadata_hash=preflight(a.binary,static,nav,a.output)
    runtime={"model":MODEL,"effort":EFFORT,"binary":str(a.binary),"binary_sha256":digest(a.binary.read_bytes()),"static_sha256":digest(static.encode()),"metadata_sha256":metadata_hash,"cli":subprocess.check_output(["codex","--version"],text=True).strip(),"base_commit":subprocess.check_output(["git","rev-parse","HEAD"],cwd=ROOT,text=True).strip(),"source_files":{p:digest((ROOT/p).read_bytes()) for p in ["src/prompts/source.md","src/prompts/builders.rs",".codescout/system-prompt.md","src/prompts/guides/symbol-navigation.md","src/tools/symbol/call_graph/mod.rs"]}}
    (a.output/"runtime.json").write_text(json.dumps(runtime,indent=2)+"\n")
    cases=[("chain",q) for q in CHAIN]*2+[("direct",q) for q in DIRECT]
    if a.smoke:cases=cases[:1]
    with concurrent.futures.ThreadPoolExecutor(max_workers=2) as pool:
        results=list(pool.map(lambda row:run(row[0],row[1][0],row[1][1],a.binary,static,nav,metadata_hash,a.output),enumerate(cases)))
    (a.output/"results.json").write_text(json.dumps(results,indent=2)+"\n")
if __name__=="__main__":main()
