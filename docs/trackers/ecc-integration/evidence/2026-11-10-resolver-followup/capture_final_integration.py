"""Capture the final two-file source and integration identities without touching peer changes."""
import hashlib,json,subprocess
from pathlib import Path
HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[4]
SOURCE=ROOT/'.worktrees/ecc-resolver-followup-20261110'
frozen=json.loads((HERE/'final-source.json').read_text())
PATHS=sorted(frozen['paths'])
def git(*args,cwd=ROOT,data=None):return subprocess.check_output(['git',*args],cwd=cwd,input=data)
def identity(cwd):
    commit=git('rev-parse','HEAD',cwd=cwd).decode().strip()
    paths=git('show','--format=','--name-only',commit,cwd=cwd).decode().splitlines()
    assert sorted(paths)==PATHS,paths
    patchid=subprocess.check_output(['git','patch-id','--stable'],input=git('show',commit,cwd=cwd)).decode().split()[0]
    return dict(commit=commit,tree=git('rev-parse','HEAD^{tree}',cwd=cwd).decode().strip(),parent=git('rev-parse','HEAD^',cwd=cwd).decode().strip(),stable_patch_id=patchid,paths=paths)
source=identity(SOURCE)
integration=identity(ROOT)
assert source['stable_patch_id']==integration['stable_patch_id']
for cwd,record in [(SOURCE,source),(ROOT,integration)]:
    for path,expected in frozen['paths'].items():
        assert hashlib.sha256(git('show',record['commit']+':'+path,cwd=cwd)).hexdigest()==expected
        assert hashlib.sha256((cwd/path).read_bytes()).hexdigest()==expected
    assert hashlib.sha256((cwd/'src/observations/store.rs').read_bytes()).hexdigest()==frozen['production_store_sha256']
before=json.loads((HERE/'foreign-index-before.json').read_text())
after={p:git('ls-files','--stage','--',p).decode().strip() for p in before}
assert before==after
delta=git('diff','--name-status',source['parent'],integration['parent']).decode()
(HERE/'peer-integration-delta.txt').write_text(delta)
gate=json.loads((HERE/'final-gate-result.json').read_text())
assert gate['exit_code']==0 and gate['source_unchanged'] and gate['paths']==frozen['paths']
native=json.loads((HERE/'native-observation-result.json').read_text())
assert native['qualification_data_validated']
record=dict(source=source,integration=integration,source_sha256=frozen['paths'],gate_binding='Frozen two-file candidate full gate; exact owned source and patch-id preserved at integration. Peer delta separately recorded; no blanket whole-tree equality claim.',identical_committed_trees=source['tree']==integration['tree'],foreign_index_blobs_unchanged=after,peer_delta='peer-integration-delta.txt',replay_ref='refs/ecc-replay/resolver-followup-20261110',limits='Resolver documentation and observation-store fixture only; production store unchanged; BFS defect remains open. No repair replay, push, shared runtime/profile change.')
(HERE/'integration.json').write_text(json.dumps(record,indent=2)+'\n')
for number in [0xa9,0xaa,0xae,0xaf,0xb0,0xb1,0xb3,0xb4,0xb5]:
    path=Path('/tmp/codescout-bg')/('1520813-b902d740-bg_'+format(number,'08x')+'.json')
    if path.exists():
        target=HERE/'jobs'/path.name
        target.parent.mkdir(exist_ok=True)
        target.write_bytes(path.read_bytes())
print(json.dumps(record,indent=2))
