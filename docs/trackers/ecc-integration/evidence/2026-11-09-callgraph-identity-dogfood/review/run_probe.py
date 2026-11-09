import sys, subprocess, pathlib, json, hashlib, difflib, os
e=pathlib.Path("/home/marius/work/claude/codescout/docs/trackers/ecc-integration/evidence/2026-11-09-callgraph-identity-dogfood/review")
root=pathlib.Path.cwd()
files=["src/tools/symbol/call_edges/cache.rs","src/tools/symbol/call_graph/mod.rs"]
def hashes(r): return {f:hashlib.sha256((r/f).read_bytes()).hexdigest() for f in files}
if sys.argv[1]=="driver":
    ident=sys.argv[2]
    d=e/ident
    d.mkdir(exist_ok=True)
    (d/"applied-source.json").write_text(json.dumps({"root":str(root),"hashes":hashes(root)},indent=2))
    (d/"applied-full.patch").write_bytes(subprocess.check_output(["git","diff","HEAD","--",*files]))
    original=pathlib.Path("/home/marius/work/claude/codescout/.worktrees/ecc-callgraph-identity-review")
    diff="".join("".join(difflib.unified_diff((original/f).read_text().splitlines(True),(root/f).read_text().splitlines(True),fromfile=f,tofile=f)) for f in files)
    (d/"applied-mutation.patch").write_text(diff)
    sys.exit(subprocess.call(["./scripts/with-slot.sh","--slot","1","cargo","test","--lib","tools::symbol::call_","--","--test-threads=4"]))
ident=sys.argv[1]
m=next(x for x in json.loads((e/"descriptors.json").read_text()) if x["id"]==ident)
d=e/ident
d.mkdir(exist_ok=True)
(d/"descriptor.json").write_text(json.dumps(m,indent=2))
before=hashes(root)
env=os.environ.copy()
env.pop("CARGO_TARGET_DIR",None)
env["TMPDIR"]="/home/marius/.cache/codescout-ecc-tmp"
env["CODESCOUT_GATE_TMPFS_BASE"]="off"
cmd=["./scripts/mutation-probe.sh","--strict","--file",m["file"],"--find",m["find"],"--replace",m["replace"],"--","python3",str(e/"run_probe.py"),"driver",ident]
with (d/"run.log").open("w") as log:
    rc=subprocess.call(cmd,stdout=log,stderr=subprocess.STDOUT,env=env)
after=hashes(root)
applied=json.loads((d/"applied-source.json").read_text()) if (d/"applied-source.json").exists() else {}
tree=pathlib.Path(applied["root"]) if applied else None
restored=hashes(tree) if tree else None
out={"mutation":ident,"returncode":rc,"before":before,"review_after":after,"mutation_tree":str(tree),"mutation_tree_restored":restored,"restored_exactly":before==after==restored}
(d/"restoration.json").write_text(json.dumps(out,indent=2))
print(json.dumps(out))
log=(d/"run.log").read_text()
for line in log.splitlines():
    if "test result:" in line or "VERDICT" in line or "SURVIVED" in line or "KILLED" in line or "INCONCLUSIVE" in line: print(line)
sys.exit(0 if out["restored_exactly"] else 4)
