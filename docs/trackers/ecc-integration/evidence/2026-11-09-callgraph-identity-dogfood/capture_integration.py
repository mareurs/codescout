"""Bind source, integration, exact gate trees and preserved foreign staged blobs."""
import hashlib,json,subprocess
from pathlib import Path
HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[4]
SOURCE=ROOT/'.worktrees/ecc-callgraph-identity'
GATED=ROOT/'.worktrees/ecc-callgraph-identity-integration-gate'
paths=['src/tools/symbol/call_edges/cache.rs','src/tools/symbol/call_graph/mod.rs']
def git(*args,cwd=ROOT,data=None):
    return subprocess.check_output(['git',*args],cwd=cwd,input=data)
def rev(value,cwd=ROOT):
    return git('rev-parse',value,cwd=cwd).decode().strip()
def identity(commit,cwd):
    sha=rev(commit,cwd)
    patchid=subprocess.check_output(['git','patch-id','--stable'],input=git('show',sha,cwd=cwd)).decode().split()[0]
    actual=git('show','--format=','--name-only',sha,cwd=cwd).decode().splitlines()
    assert actual==paths,actual
    return dict(commit=sha,tree=rev(sha+'^{tree}',cwd),parent=rev(sha+'^',cwd),stable_patch_id=patchid,paths=actual)
source=identity('HEAD',SOURCE)
integration=identity('HEAD',GATED)
assert source['commit'].startswith('555f2817c')
assert integration['commit'].startswith('4776e44bd')
assert source['stable_patch_id']==integration['stable_patch_id']
frozen=json.loads((HERE/'final-source.json').read_text())['files']
for cwd,commit in [(SOURCE,source['commit']),(ROOT,integration['commit']),(GATED,integration['commit'])]:
    for path,digest in frozen.items():
        committed=git('show',commit+':'+path,cwd=cwd)
        assert hashlib.sha256(committed).hexdigest()==digest
        assert (cwd/path).read_bytes()==committed
before=json.loads((HERE/'foreign-index-before.json').read_text())
after={p:git('ls-files','--stage','--',p).decode().strip() for p in before}
assert before==after
assert set(git('diff','--cached','--name-only').decode().splitlines())==set(before)
gate=json.loads((HERE/'integration-gate-result.json').read_text())
assert gate['exit_code']==0 and gate['head_unchanged'] and gate['source_clean']
assert gate['head']==integration['commit'] and gate['tree']==integration['tree']
peer=git('diff','--name-status',source['parent'],integration['parent']).decode()
(HERE/'peer-integration-delta.txt').write_text(peer)
record=dict(source=source,integration=integration,source_hashes=frozen,identical_committed_trees=source['tree']==integration['tree'],gate_binding='Final candidate gate covers isolated base plus patch. Separate integration gate covers exact4776e44bd committed tree including peer changes. Only owned source files and stable patch-id are identical across source and integration.',foreign_staged_blobs_unchanged=after,replay_base=rev('refs/ecc-replay/callgraph-identity-20261109'),home_head_at_capture=rev('HEAD'),limits='No repair replay, push, release, profile change or shared-MCP rollout.')
(HERE/'integration.json').write_text(json.dumps(record,indent=2)+'\n')
for number in [0x90,0x91,0x93,0x94,0x95,0x96,0x97,0x98,0x99,0xa0]:
    path=Path('/tmp/codescout-bg')/('1520813-b902d740-bg_'+format(number,'08x')+'.json')
    if path.exists():
        dst=HERE/'jobs'/path.name
        dst.parent.mkdir(exist_ok=True)
        dst.write_bytes(path.read_bytes())
print(json.dumps(record,indent=2))
