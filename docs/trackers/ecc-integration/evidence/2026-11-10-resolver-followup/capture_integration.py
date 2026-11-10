"""Capture exact comment-patch identity and integration deltas without touching peers."""
import hashlib,json,subprocess
from pathlib import Path
HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[4]
SOURCE=ROOT/'.worktrees/ecc-resolver-followup-20261110'
FILE='src/tools/symbol/call_edges/resolver.rs'
def git(*args,cwd=ROOT,data=None):return subprocess.check_output(['git',*args],cwd=cwd,input=data)
def identity(cwd):
    commit=git('rev-parse','HEAD',cwd=cwd).decode().strip()
    paths=git('show','--format=','--name-only',commit,cwd=cwd).decode().splitlines()
    assert paths==[FILE],paths
    patchid=subprocess.check_output(['git','patch-id','--stable'],input=git('show',commit,cwd=cwd)).decode().split()[0]
    return dict(commit=commit,tree=git('rev-parse','HEAD^{tree}',cwd=cwd).decode().strip(),parent=git('rev-parse','HEAD^',cwd=cwd).decode().strip(),stable_patch_id=patchid,paths=paths)
source=identity(SOURCE)
integration=identity(ROOT)
assert source['stable_patch_id']==integration['stable_patch_id']
frozen=json.loads((HERE/'candidate-source.json').read_text())['sha256']
for cwd,record in [(SOURCE,source),(ROOT,integration)]:
    assert hashlib.sha256(git('show',record['commit']+':'+FILE,cwd=cwd)).hexdigest()==frozen
    assert hashlib.sha256((cwd/FILE).read_bytes()).hexdigest()==frozen
before=json.loads((HERE/'foreign-index-before.json').read_text())
after={p:git('ls-files','--stage','--',p).decode().strip() for p in before}
assert before==after
delta=git('diff','--name-status',source['parent'],integration['parent']).decode()
(HERE/'peer-integration-delta.txt').write_text(delta)
gate=json.loads((HERE/'gate-retry-result.json').read_text())
assert gate['exit_code']==0 and gate['source_unchanged']
native=json.loads((HERE/'native-observation-result.json').read_text())
assert native['qualification_data_validated']
record=dict(source=source,integration=integration,source_sha256=frozen,gate_binding='Frozen candidate full gate; exact owned source and patch-id preserved at integration. Peer delta separately recorded; no blanket equality claim across whole trees.',identical_committed_trees=source['tree']==integration['tree'],foreign_index_blobs_unchanged=after,peer_delta='peer-integration-delta.txt',replay_ref='refs/ecc-replay/resolver-followup-20261110',limits='Documentation-only source patch; BFS defect remains open. No repair replay, push, shared runtime/profile change.')
(HERE/'integration.json').write_text(json.dumps(record,indent=2)+'\n')
for number in [0xa9,0xaa,0xae,0xaf]:
    path=Path('/tmp/codescout-bg')/('1520813-b902d740-bg_'+format(number,'08x')+'.json')
    if path.exists():
        target=HERE/'jobs'/path.name
        target.parent.mkdir(exist_ok=True)
        target.write_bytes(path.read_bytes())
print(json.dumps(record,indent=2))
