import pathlib,json,hashlib,importlib.util,subprocess
from importlib.machinery import SourceFileLoader
HERE=pathlib.Path("/home/marius/work/claude/codescout/docs/trackers/ecc-integration/evidence/2026-11-10-bfs-identity-dogfood")
runtime=json.loads((HERE/'candidate-binary.json').read_text());binary=pathlib.Path(runtime['snapshot'])
assert hashlib.sha256(binary.read_bytes()).hexdigest()==runtime['sha256']
loader=SourceFileLoader('review_transport',str(HERE.parent/'2026-11-09-doc-create-dogfood/transport-helper.py.txt'))
spec=importlib.util.spec_from_loader(loader.name,loader);helper=importlib.util.module_from_spec(spec);loader.exec_module(helper)
cases=json.loads("[{\"name\":\"block-import-shadows-root\",\"source\":\"mod a { pub fn helper() { right(); } fn right() {} }\\nfn helper() { wrong(); }\\nfn wrong() {}\\nfn entry() { use a::helper; helper(); }\\n\"},{\"name\":\"callable-parameter-shadows-root\",\"source\":\"fn helper() { wrong(); }\\nfn wrong() {}\\nfn entry<F: Fn()>(helper: F) { helper(); }\\n\"},{\"name\":\"nested-function-shadows-root\",\"source\":\"fn helper() { wrong(); }\\nfn wrong() {}\\nfn right() {}\\nfn entry() { fn helper() { right(); } helper(); }\\n\"},{\"name\":\"generic-type-shadows-concrete\",\"source\":\"struct A; impl A { fn helper() { wrong(); } }\\nfn wrong() {}\\ntrait T { fn helper(); }\\nfn entry<A: T>() { A::helper(); }\\n\"}]"); results=[]
for case in cases:
 scratch=HERE/'review'/'native'/case['name'];project=scratch/'project';project.mkdir(parents=True,exist_ok=False)
 files={'src/lib.rs':case['source'],'Cargo.toml':'[package]\nname="review_probe"\nversion="0.1.0"\nedition="2021"\n','.codescout/project.toml':'[project]\nname="review-probe"\nlanguages=["rust"]\n'}
 for rel,data in files.items():
  file=project/rel;file.parent.mkdir(parents=True,exist_ok=True);file.write_text(data)
 subprocess.run(['git','init','-q',str(project)],check=True)
 with helper.McpProbe(binary,project,env=helper.isolated_server_env(scratch/'state')) as probe:
  args=dict(symbol='entry',path='src/lib.rs',direction='callees',max_depth=2,detail_level='full')
  response=probe.request('tools/call',dict(name='call_graph',arguments=args),timeout=120)
 row=dict(case=case,args=args,response=response,fixture_sha256={p:hashlib.sha256(d.encode()).hexdigest() for p,d in files.items()})
 results.append(row);(HERE/'review/native-results.json').write_text(json.dumps(dict(binary=runtime,cases=results),indent=2));print(case['name'],response,flush=True)
