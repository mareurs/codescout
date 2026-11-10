"""Read source/gate/peer scope and raw artifact inventory; does not stage or commit."""
import hashlib,json,os,re,subprocess
from pathlib import Path
HERE=Path(__file__).resolve().parent;ROOT=HERE.parents[4]
WT=ROOT/'.worktrees/ecc-bfs-identity-20261110'
def git(root,*a):return subprocess.check_output(['git',*a],cwd=root).decode().strip()
final=json.loads((HERE/'final-source.json').read_text())
assert {p:hashlib.sha256((WT/p).read_bytes()).hexdigest() for p in final['paths']}==final['paths']
foreign=json.loads((HERE/'foreign-index-before.json').read_text())
print('FOREIGN_BASELINE_SHAPE',str(foreign)[:500])
gate=(HERE/'final-gate.log').read_bytes().decode(errors='replace')
state=dict(home_head=git(ROOT,'rev-parse','HEAD'),source_head=git(WT,'rev-parse','HEAD'),peer_changed_paths=git(ROOT,'diff','--name-only',final['base'],'HEAD').splitlines(),source_paths=final['paths'],gate=json.loads((HERE/'final-gate-result.json').read_text()),gate_summaries=re.findall(r'^.*(?:Summary.*tests run:|GATE EXITS|PYTHON LIGHT EXIT|passed, 7 skipped).*$',gate,re.M),native=json.loads((HERE/'final-native/process-result.json').read_text()),environment={k:subprocess.check_output(cmd,cwd=WT,stderr=subprocess.STDOUT).decode().strip() for k,cmd in [('rustc',['rustc','-Vv']),('cargo',['cargo','-V']),('python',['python3','-V']),('kernel',['uname','-sr'])]})
(HERE/'preintegration-state.json').write_text(json.dumps(state,indent=2)+'\n')
print(json.dumps({k:state[k] for k in ['home_head','source_head','peer_changed_paths','gate_summaries']},indent=2))
inventory=[]
for p in sorted(HERE.rglob('*')):
    if p.is_file():
        rel=p.relative_to(HERE)
        inventory.append(dict(path=str(rel),bytes=p.stat().st_size))
(HERE/'raw-artifact-inventory.json').write_text(json.dumps(inventory,indent=2)+'\n')
print('TOP_LEVEL_ENTRIES',len(list(HERE.iterdir())))
print('FILES',len(inventory))
print('LARGE_FILES',json.dumps([p for p in inventory if p['bytes']>2000000]))
print('NATIVE_LOCAL_FILES',json.dumps([p for p in inventory if p['path'].startswith('review/native/')],indent=2))
