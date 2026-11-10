"""Qualify duplicated-name BFS with actual isolated MCP and complete edge multisets."""
from collections import Counter
import copy,hashlib,importlib.util,json,re,subprocess
from importlib.machinery import SourceFileLoader
from pathlib import Path
HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[5]
runtime=json.loads((HERE.parent/'final-binary.json').read_text())
binary=Path(runtime['snapshot'])
assert hashlib.sha256(binary.read_bytes()).hexdigest()==runtime['sha256']
loader=SourceFileLoader('resolver_followup_transport',str(HERE.parent.parent/'2026-11-09-doc-create-dogfood/transport-helper.py.txt'))
spec=importlib.util.spec_from_loader(loader.name,loader)
helper=importlib.util.module_from_spec(spec)
loader.exec_module(helper)
private=ROOT/'.codescout/ecc-bfs-identity-private-20261110'
cases=[]
def fixture(unique):
    a='call_a' if unique else 'helper'
    b='call_b' if unique else 'helper'
    return {'Cargo.toml':'[package]\nname="bfs_fixture"\nversion="0.1.0"\nedition="2021"\n',
            '.codescout/project.toml':'[project]\nname="bfs-fixture"\nlanguages=["rust"]\n',
            'src/lib.rs':f'pub mod a;\npub mod b;\npub fn entry() {{\n    a::{a}();\n    b::{b}();\n}}\n',
            'src/a.rs':f'pub fn {a}() {{ leaf_a(); }}\npub fn leaf_a() {{}}\n',
            'src/b.rs':f'pub fn {b}() {{ leaf_b(); }}\npub fn leaf_b() {{}}\n'}
def project(name,unique):
    root=private/name
    path=root/'project'
    path.mkdir(parents=True)
    files=fixture(unique)
    for rel,data in files.items():
        target=path/rel
        target.parent.mkdir(parents=True,exist_ok=True)
        target.write_text(data)
    subprocess.run(['git','init','-q',str(path)],check=True)
    return path,helper.isolated_server_env(root/'state'),files
def expected(unique,depth=2):
    a='call_a' if unique else 'helper'
    b='call_b' if unique else 'helper'
    first=[('entry',a,'src/lib.rs',4,1),('entry',b,'src/lib.rs',5,1)]
    return first if depth==1 else first+[(a,'leaf_a','src/a.rs',1,2),(b,'leaf_b','src/b.rs',1,2)]
def inspect(response,symbol,wanted):
    text=response.get('content',[{}])[0].get('text','')
    edges=[]
    file=None
    for line in text.splitlines():
        header=re.fullmatch(r'\s+(\S+) \(\d+\)',line)
        if header: file=header[1]
        edge=re.fullmatch(r'\s*(\d+):\s+(.+?) → (.+?) \(depth=(\d+), (lsp|ts)\)',line)
        if edge: edges.append((edge[2],edge[3],file,int(edge[1]),int(edge[4])))
    totals=re.search(r'callees: (\d+) edges across (\d+) files',text)
    clean=(not response.get('isError',False) and text.startswith('call_graph for `'+symbol+'`\n')
           and totals is not None and int(totals[1])==len(edges)
           and int(totals[2])==len({e[2] for e in edges})
           and 'warning:' not in text.lower() and 'completeness_warning' not in text
           and 'truncated: true' not in text)
    return dict(edges=edges,clean_render=bool(clean),matches_expected=bool(clean and Counter(edges)==Counter(wanted)))
def call(probe,label,path,symbol,depth,wanted):
    args=dict(path=path,symbol=symbol,direction='callees',max_depth=depth,detail_level='full')
    response=probe.request('tools/call',dict(name='call_graph',arguments=args),timeout=120)
    row=dict(label=label,arguments=args,response=response,expected=wanted,**inspect(response,symbol,wanted))
    cases.append(row)
    (HERE/'bfs-cases.json').write_text(json.dumps(dict(binary=runtime,cases=cases),indent=2)+'\n')
    print(json.dumps({k:row[k] for k in ['label','edges','clean_render','matches_expected']}),flush=True)
    return response
path,env,unique_fixture=project('final-bfs-unique',True)
with helper.McpProbe(binary,path,env=env) as probe:
    positive=call(probe,'unique-names-depth2','src/lib.rs','entry',2,expected(True))
path,env,duplicate_fixture=project('final-bfs-duplicate',False)
with helper.McpProbe(binary,path,env=env) as probe:
    call(probe,'duplicate-cold-depth2','src/lib.rs','entry',2,expected(False))
with helper.McpProbe(binary,path,env=env) as probe:
    call(probe,'duplicate-restart-depth2','src/lib.rs','entry',2,expected(False))
    call(probe,'duplicate-seed-depth1','src/lib.rs','entry',1,expected(False,1))
    call(probe,'pinned-helper-a-depth1','src/a.rs','helper',1,[('helper','leaf_a','src/a.rs',1,1)])
    call(probe,'pinned-helper-b-depth1','src/b.rs','helper',1,[('helper','leaf_b','src/b.rs',1,1)])
    call(probe,'duplicate-after-both-pinned-helpers','src/lib.rs','entry',2,expected(False))
controls=[dict(name='actual-unique-positive',actual=inspect(positive,'entry',expected(True))['matches_expected'],expected=True)]
for name,old,new in [('wrong-file','src/a.rs','src/wrong.rs'),('wrong-depth','depth=2','depth=3'),('wrong-callee','leaf_a','leaf_wrong')]:
    altered=copy.deepcopy(positive)
    altered['content'][0]['text']=altered['content'][0]['text'].replace(old,new)
    controls.append(dict(name=name,actual=inspect(altered,'entry',expected(True))['matches_expected'],expected=False))
altered=copy.deepcopy(positive)
altered['content'].append(copy.deepcopy(altered['content'][0]))
altered['content'][0]['text']='No completed graph; edges mentioned only in later guidance.'
controls.append(dict(name='guide-only-edges',actual=inspect(altered,'entry',expected(True))['matches_expected'],expected=False))
(HERE/'bfs-scorer-controls.json').write_text(json.dumps(controls,indent=2)+'\n')
fixtures=dict(unique=unique_fixture,duplicate=duplicate_fixture)
(HERE/'bfs-fixtures.json').write_text(json.dumps(dict(files=fixtures,sha256={name:{p:hashlib.sha256(data.encode()).hexdigest() for p,data in files.items()} for name,files in fixtures.items()}),indent=2)+'\n')
control_names={'unique-names-depth2','duplicate-seed-depth1','pinned-helper-a-depth1','pinned-helper-b-depth1'}
control_rows=[r for r in cases if r['label'] in control_names]
candidate_rows=[r for r in cases if r['label'] not in control_names]
summary=dict(native_cases=len(cases),qualifying_controls=len(control_rows),qualifying_controls_matched=sum(r['matches_expected'] for r in control_rows),duplicate_depth2_cases=len(candidate_rows),duplicate_depth2_mismatches=sum(not r['matches_expected'] for r in candidate_rows),duplicate_clean_renders=sum(r['clean_render'] for r in candidate_rows),scorer_controls=len(controls),scorer_controls_as_expected=sum(r['actual']==r['expected'] for r in controls),limits='One tiny Rust topology and one current-source executable; no general LSP completeness, performance or ECC causality claim.')
(HERE/'bfs-summary.json').write_text(json.dumps(summary,indent=2)+'\n')
print(json.dumps(summary),flush=True)
assert all(r['actual']==r['expected'] for r in controls)
assert len(control_rows)==4 and all(r['matches_expected'] for r in control_rows)
assert len(candidate_rows)==3 and all(r['matches_expected'] for r in candidate_rows)
