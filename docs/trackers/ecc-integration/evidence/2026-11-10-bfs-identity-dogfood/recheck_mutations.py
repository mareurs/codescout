"""Reapply observed production mutations and two fix-boundary mutations on final frozen bytes."""
import datetime,difflib,hashlib,json,os,re,subprocess
from pathlib import Path
HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[4]
WT=ROOT/'.worktrees/ecc-bfs-final-mutations-20261110'
OUT=HERE/'final-mutations'
OUT.mkdir()
frozen=json.loads((HERE/'final-source.json').read_text())
subprocess.run(['git','worktree','add','--detach',str(WT),frozen['base']],cwd=ROOT,check=True)
subprocess.run(['git','apply',str(HERE/'final-candidate.patch')],cwd=WT,check=True)
def hashes(root):
    return {p:hashlib.sha256((root/p).read_bytes()).hexdigest() for p in frozen['paths']}
assert hashes(WT)==frozen['paths']
env=dict(os.environ,TMPDIR='/home/marius/.cache/codescout-ecc-tmp',CODESCOUT_GATE_TMPFS_BASE='off')
env.pop('CARGO_TARGET_DIR',None)
test=['./scripts/with-slot.sh','--slot','1','cargo','test','--no-default-features','--lib','tools::symbol::call_']
with (OUT/'baseline.log').open('wb') as log:
    baseline=subprocess.run(test,cwd=WT,env=env,stdout=log,stderr=subprocess.STDOUT)
assert baseline.returncode==0
assert b'test result: ok. 95 passed; 0 failed;' in (OUT/'baseline.log').read_bytes()
assert hashes(WT)==frozen['paths']
mutations=json.loads((HERE/'review/mutations.json').read_text())
mutations.extend([
 dict(id='M14',file='src/tools/symbol/call_graph/identity.rs',find='if site.lexical_binding {',replace='if false {',axis='new lexical guard bypassed'),
 dict(id='M15',file='src/tools/symbol/call_graph/traversal.rs',find='                level_raw\n                    .iter()',replace='                level_edges\n                    .iter()',axis='display rows replace endpoint frontier'),
])
(OUT/'mutations.json').write_text(json.dumps(mutations,indent=2)+'\n')
results=[]
for m in mutations:
    before=(WT/m['file']).read_text()
    assert before.count(m['find'])==1,m
    patch=''.join(difflib.unified_diff(before.splitlines(True),before.replace(m['find'],m['replace']).splitlines(True),fromfile=m['file'],tofile=m['file']))
    (OUT/(m['id']+'.patch')).write_text(patch)
    command=['./scripts/mutation-probe.sh','--strict','--file',m['file'],'--find',m['find'],'--replace',m['replace'],'--',*test]
    started=datetime.datetime.now(datetime.timezone.utc).isoformat()
    with (OUT/(m['id']+'.log')).open('wb') as log:
        process=subprocess.run(command,cwd=WT,env=env,stdout=log,stderr=subprocess.STDOUT)
    raw=(OUT/(m['id']+'.log')).read_bytes()
    text=raw.decode(errors='replace')
    summaries=re.findall(r'^test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out;.*$',text,re.M)
    last=summaries[-1] if summaries else None
    verdict='inconclusive' if not last or sum(map(int,last[:2]))!=95 else ('survived' if int(last[1])==0 else 'killed')
    trees=re.findall(r'mutation-probe: ARMED  mode=isolated  file=.*?  tree=(.*)',text)
    tree=Path(trees[-1]) if trees else None
    record=dict(m,exit_code=process.returncode,verdict=verdict,top_level_summary=last,started=started,finished=datetime.datetime.now(datetime.timezone.utc).isoformat(),command=command,log_sha256=hashlib.sha256(raw).hexdigest(),patch_sha256=hashlib.sha256((OUT/(m['id']+'.patch')).read_bytes()).hexdigest(),mutant_file_sha256=hashlib.sha256(before.replace(m['find'],m['replace']).encode()).hexdigest(),mutation_tree=str(tree),source_restored=hashes(WT)==frozen['paths'],mutation_tree_restored=hashes(tree)==frozen['paths'] if tree else False)
    results.append(record)
    (OUT/'results.json').write_text(json.dumps(results,indent=2)+'\n')
    print(m['id'],verdict,last,flush=True)
    assert record['source_restored'] and record['mutation_tree_restored'],record
summary=dict(applied=len(results),killed=sum(r['verdict']=='killed' for r in results),survived=sum(r['verdict']=='survived' for r in results),inconclusive=sum(r['verdict']=='inconclusive' for r in results),source=frozen,baseline_exit=baseline.returncode,baseline_log_sha256=hashlib.sha256((OUT/'baseline.log').read_bytes()).hexdigest(),worktree=str(WT))
(OUT/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
assert summary['killed']==15,summary
