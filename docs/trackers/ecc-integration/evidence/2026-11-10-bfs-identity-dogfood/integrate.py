"""Integrate only frozen owned source paths, preserving the six foreign staged blobs."""
import hashlib,json,os,subprocess,sys
from pathlib import Path
HERE=Path(__file__).resolve().parent;ROOT=HERE.parents[4]
def git(*a):return subprocess.check_output(['git',*a],cwd=ROOT)
foreign=json.loads((HERE/'foreign-index-before.json').read_text())
def foreign_state():return {p:git('ls-files','--stage','--',p).decode().strip() for p in foreign}
assert foreign_state()==foreign
source=json.loads((HERE/'source-commit.json').read_text())
frozen=json.loads((HERE/'final-source.json').read_text());paths=list(frozen['paths'])
gate=json.loads((HERE/'integrated-gate-result.json').read_text())
assert gate['exit_code']==0 and gate['source_unchanged'] and gate['tree_after']==gate['expected_tree']
assert git('branch','--show-current').decode().strip()=='experiments'
if sys.argv[1]=='stage':
    base=git('rev-parse','HEAD').decode().strip()
    assert base==gate['base'],('home HEAD moved; inspect delta before integration',base,gate['base'])
    subprocess.run(['git','diff','--quiet','HEAD','--',*paths],cwd=ROOT,check=True)
    subprocess.run(['git','apply','--check',str(HERE/'final-candidate.patch')],cwd=ROOT,check=True)
    subprocess.run(['git','apply',str(HERE/'final-candidate.patch')],cwd=ROOT,check=True)
    assert {p:hashlib.sha256((ROOT/p).read_bytes()).hexdigest() for p in paths}==frozen['paths']
    subprocess.run(['git','add','--',*paths],cwd=ROOT,check=True)
    patch=git('diff','--cached','--binary','--',*paths)
    (HERE/'integration-staged.patch').write_bytes(patch)
    patch_id=subprocess.check_output(['git','patch-id','--stable'],input=patch).decode().split()[0]
    assert patch_id==source['patch_id']
    assert foreign_state()==foreign
    print(git('diff','--cached','--stat','--',*paths).decode())
    print(git('diff','--cached','--name-only').decode())
elif sys.argv[1]=='commit':
    assert git('rev-parse','HEAD').decode().strip()==gate['base']
    assert git('diff','--cached','--binary','--',*paths)==(HERE/'integration-staged.patch').read_bytes()
    assert {p:hashlib.sha256((ROOT/p).read_bytes()).hexdigest() for p in paths}==frozen['paths']
    subprocess.run(['git','diff','--quiet','--',*paths],cwd=ROOT,check=True)
    message=HERE/'integration-message.txt'
    message.write_text('fix(call-graph): integrate declaration-aware traversal\n\nIntegrate isolated source '+source['sha']+'. Preserve separate provider and visited positions, semantic endpoint frontier, completed target metadata and bounded Rust fallback warnings.\n\nValidated final related95/0, fifteen applied mutations killed and separate full gates for source and exact integration tree. No shared runtime rollout or repair replay.\n\nSession-Id: codex:01a1bf68-e650-7f20-911e-b53f86597c27\n')
    subprocess.run(['git','commit','-F',str(message),'--',*paths],cwd=ROOT,check=True)
    sha=git('rev-parse','HEAD').decode().strip()
    patch_id=subprocess.check_output(['git','patch-id','--stable'],input=git('show','--format=','--binary',sha)).decode().split()[0]
    tree=git('rev-parse',sha+'^{tree}').decode().strip()
    assert tree==gate['expected_tree']
    assert patch_id==source['patch_id']
    assert set(git('diff-tree','--no-commit-id','--name-only','-r',sha).decode().splitlines())==set(paths)
    assert {p:hashlib.sha256(git('show',sha+':'+p)).hexdigest() for p in paths}==frozen['paths']
    assert foreign_state()==foreign
    record=dict(source=source['sha'],integration=sha,patch_id=patch_id,tree=tree,expected_gated_tree=gate['expected_tree'],exact_gated_tree_matches=True,parent=gate['base'],source_paths=frozen['paths'],foreign_index_unchanged=True)
    (HERE/'integration.json').write_text(json.dumps(record,indent=2)+'\n');print(json.dumps(record,indent=2))
else:raise SystemExit('stage or commit')
