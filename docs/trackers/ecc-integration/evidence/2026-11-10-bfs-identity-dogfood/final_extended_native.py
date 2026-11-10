"""Additional real-MCP identity/context cases; immutable candidate, no repair replay."""
from collections import Counter
import ast,copy,hashlib,importlib.util,json,re,subprocess
from importlib.machinery import SourceFileLoader
from pathlib import Path
HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[4]
runtime=json.loads((HERE/'final-binary.json').read_text())
binary=Path(runtime['snapshot'])
assert hashlib.sha256(binary.read_bytes()).hexdigest()==runtime['sha256']
loader=SourceFileLoader('bfs_extended_transport',str(HERE.parent/'2026-11-09-doc-create-dogfood/transport-helper.py.txt'))
spec=importlib.util.spec_from_loader(loader.name,loader)
helper=importlib.util.module_from_spec(spec)
loader.exec_module(helper)
# Extract only the pure previously calibrated first-content-block parser.
source=(HERE/'baseline-native/probe.py').read_text()
node=next(n for n in ast.parse(source).body if isinstance(n,ast.FunctionDef) and n.name=='inspect')
exec(compile(ast.Module(body=[node],type_ignores=[]),str(HERE/'baseline-native/probe.py'),'exec'))
topologies=[
 dict(name='same-file-same-line-ast',symbol='entry',path='src/lib.rs',files={
 'src/lib.rs':'struct A; impl A { fn helper() { leaf_a(); } } struct B; impl B { fn helper() { leaf_b(); } }\nfn leaf_a() {}\nfn leaf_b() {}\nfn entry() { A::helper(); B::helper(); }\n'},
 wanted=[('entry','helper','src/lib.rs',4,1),('entry','helper','src/lib.rs',4,1),('helper','leaf_a','src/lib.rs',1,2),('helper','leaf_b','src/lib.rs',1,2)],warning=False),
 dict(name='inline-module-ast',symbol='entry',path='src/lib.rs',files={
 'src/lib.rs':'pub mod a { pub fn helper() { leaf_a(); } fn leaf_a() {} }\npub mod b { pub fn helper() { leaf_b(); } fn leaf_b() {} }\npub fn entry() { a::helper(); b::helper(); }\n'},
 wanted=[('entry','helper','src/lib.rs',3,1),('entry','helper','src/lib.rs',3,1),('helper','leaf_a','src/lib.rs',1,2),('helper','leaf_b','src/lib.rs',2,2)],warning=False),
 dict(name='same-name-seed-child-module',symbol='helper',path='src/a.rs',files={
 'src/lib.rs':'pub mod a;\n','src/a.rs':'pub mod b;\npub fn helper() { b::helper(); }\n','src/a/b.rs':'pub fn helper() { leaf_b(); }\nfn leaf_b() {}\n'},
 wanted=[('helper','helper','src/a.rs',2,1),('helper','leaf_b','src/a/b.rs',1,2)],warning=False),
 dict(name='unresolved-receiver-context',symbol='entry',path='src/lib.rs',files={
 'src/lib.rs':'pub fn entry() { unknown.helper(); }\n'},
 wanted=[('entry','helper','src/lib.rs',1,1)],warning=True),
]
def score(response,case):
    measured=inspect(response,case['symbol'],case['wanted'])
    text=response.get('content',[{}])[0].get('text','')
    totals=re.search(r'callees: (\d+) edges across (\d+) files',text)
    header='call_graph for `'+case['symbol']+'`\n'
    warning_ok=(text.startswith('warning:') and 'helper' in text and 'incomplete' in text) if case['warning'] else measured['clean_render']
    return bool(not response.get('isError',False) and header in text and totals
        and int(totals[1])==len(measured['edges']) and int(totals[2])==len({e[2] for e in measured['edges']})
        and Counter(measured['edges'])==Counter(case['wanted']) and warning_ok),measured
cases=[]
for case in topologies:
    scratch=ROOT/'.codescout/ecc-bfs-identity-private-20261110'/('final-extended-'+case['name'])
    project=scratch/'project'
    project.mkdir(parents=True)
    files=dict(case['files'],**{'Cargo.toml':'[package]\nname="bfs_extended"\nversion="0.1.0"\nedition="2021"\n','.codescout/project.toml':'[project]\nname="bfs-extended"\nlanguages=["rust"]\n'})
    for rel,data in files.items():
        file=project/rel
        file.parent.mkdir(parents=True,exist_ok=True)
        file.write_text(data)
    subprocess.run(['git','init','-q',str(project)],check=True)
    with helper.McpProbe(binary,project,env=helper.isolated_server_env(scratch/'state')) as probe:
        args=dict(symbol=case['symbol'],path=case['path'],direction='callees',max_depth=2,detail_level='full')
        response=probe.request('tools/call',dict(name='call_graph',arguments=args),timeout=120)
    verdict,measured=score(response,case)
    row=dict(case=case,fixture_sha256={p:hashlib.sha256(data.encode()).hexdigest() for p,data in files.items()},arguments=args,response=response,measured=measured,matched=verdict)
    cases.append(row)
    (HERE/'final-extended-native-cases.json').write_text(json.dumps(dict(binary=runtime,cases=cases),indent=2)+'\n')
    print(json.dumps(dict(name=case['name'],matched=verdict,edges=measured['edges'])),flush=True)
controls=[]
last=cases[-1]
controls.append(dict(name='actual-warning',actual=score(last['response'],last['case'])[0],expected=True))
changed=copy.deepcopy(last['response'])
text=changed['content'][0]['text']
start=text.index('call_graph for')
changed['content'][0]['text']=text[start:]
controls.append(dict(name='warning-removed',actual=score(changed,last['case'])[0],expected=False))
(HERE/'final-extended-native-controls.json').write_text(json.dumps(controls,indent=2)+'\n')
summary=dict(cases=len(cases),matched=sum(r['matched'] for r in cases),controls=len(controls),controls_as_expected=sum(c['actual']==c['expected'] for c in controls),limits='Four additional concrete Rust cases; no exhaustive binding, cross-language or latency claim.')
(HERE/'final-extended-native-summary.json').write_text(json.dumps(summary,indent=2)+'\n')
print(json.dumps(summary),flush=True)
assert all(r['matched'] for r in cases)
assert all(c['actual']==c['expected'] for c in controls)
