"""Check only lexical wrong-target removal and warning; nested collector edge remains a separate bug."""
import copy,hashlib,importlib.util,json,re,subprocess
from pathlib import Path
from importlib.machinery import SourceFileLoader
HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[4]
runtime=json.loads((HERE/'final-binary.json').read_text())
binary=Path(runtime['snapshot'])
assert hashlib.sha256(binary.read_bytes()).hexdigest()==runtime['sha256']
loader=SourceFileLoader('bfs_final_lexical_transport',str(HERE.parent/'2026-11-09-doc-create-dogfood/transport-helper.py.txt'))
spec=importlib.util.spec_from_loader(loader.name,loader)
helper=importlib.util.module_from_spec(spec);loader.exec_module(helper)
cases=json.loads((HERE/'review/native-results.json').read_text())['cases']
def predicate(response):
    text=response.get('content',[{}])[0].get('text','')
    return not response.get('isError',False) and text.startswith('warning:') and 'lexical binding' in text and 'entry → helper' in text and 'helper → wrong' not in text
results=[]
for old in cases:
    case=old['case']
    scratch=ROOT/'.codescout/ecc-bfs-identity-private-20261110'/('final-lexical-'+case['name'])
    project=scratch/'project';project.mkdir(parents=True)
    files={'src/lib.rs':case['source'],'Cargo.toml':'[package]\nname="bfs_lexical_final"\nversion="0.1.0"\nedition="2021"\n','.codescout/project.toml':'[project]\nname="bfs-lexical-final"\nlanguages=["rust"]\n'}
    for rel,data in files.items():
        file=project/rel;file.parent.mkdir(parents=True,exist_ok=True);file.write_text(data)
    subprocess.run(['git','init','-q',str(project)],check=True)
    with helper.McpProbe(binary,project,env=helper.isolated_server_env(scratch/'state')) as probe:
        response=probe.request('tools/call',dict(name='call_graph',arguments=old['args']),timeout=120)
    text=response.get('content',[{}])[0].get('text','')
    result=dict(case=case,arguments=old['args'],response=response,no_wrong_leaf_and_incomplete=predicate(response),known_collector_extra_edge='entry → right' in text,fixture_sha256={p:hashlib.sha256(data.encode()).hexdigest() for p,data in files.items()})
    results.append(result);(HERE/'final-lexical-native-cases.json').write_text(json.dumps(dict(binary=runtime,cases=results),indent=2)+'\n')
    print(case['name'],result['no_wrong_leaf_and_incomplete'],flush=True)
controls=[]
actual=results[0]['response']
controls.append(dict(name='actual',actual=predicate(actual),expected=True))
removed=copy.deepcopy(actual);text=removed['content'][0]['text'];removed['content'][0]['text']=text[text.index('call_graph for'):]
controls.append(dict(name='warning-removed',actual=predicate(removed),expected=False))
wrong=copy.deepcopy(actual);wrong['content'][0]['text']+='\n          2: helper → wrong (depth=2, ts)\n'
controls.append(dict(name='wrong-leaf-reintroduced',actual=predicate(wrong),expected=False))
(HERE/'final-lexical-native-controls.json').write_text(json.dumps(controls,indent=2)+'\n')
summary=dict(cases=len(results),no_wrong_leaf_and_incomplete=sum(r['no_wrong_leaf_and_incomplete'] for r in results),controls=len(controls),controls_as_expected=sum(c['actual']==c['expected'] for c in controls),known_collector_extra_edge_cases=[r['case']['name'] for r in results if r['known_collector_extra_edge']],limits='Checks only unsupported lexical binding disclosure and removal of arbitrary wrong leaf; does not certify full graph correctness. Nested-body collector defect remains separately open.')
(HERE/'final-lexical-native-summary.json').write_text(json.dumps(summary,indent=2)+'\n')
assert all(r['no_wrong_leaf_and_incomplete'] for r in results)
assert all(c['actual']==c['expected'] for c in controls)
