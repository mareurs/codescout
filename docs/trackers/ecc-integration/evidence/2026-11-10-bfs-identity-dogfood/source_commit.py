"""Explicit owned-path source commit; stage and commit are separate invocations."""
import hashlib,json,subprocess,sys
from pathlib import Path
HERE=Path(__file__).resolve().parent;ROOT=HERE.parents[4];WT=ROOT/'.worktrees/ecc-bfs-identity-20261110'
frozen=json.loads((HERE/'final-source.json').read_text());paths=list(frozen['paths'])
def git(*args):return subprocess.check_output(['git',*args],cwd=WT)
assert {p:hashlib.sha256((WT/p).read_bytes()).hexdigest() for p in paths}==frozen['paths']
assert json.loads((HERE/'final-gate-result.json').read_text())['exit_code']==0
assert json.loads((HERE/'final-mutations/summary.json').read_text())['killed']==15
assert json.loads((HERE/'final-native/process-result.json').read_text())['exit_code']==0
if sys.argv[1]=='stage':
    subprocess.run(['git','add','--',*paths],cwd=WT,check=True)
    patch=git('diff','--cached','--',*paths)
    (HERE/'source-staged.patch').write_bytes(patch)
    print(git('diff','--cached','--stat','--',*paths).decode())
    print(git('diff','--cached','--name-only').decode())
elif sys.argv[1]=='commit':
    patch=git('diff','--cached','--',*paths)
    assert patch==(HERE/'source-staged.patch').read_bytes()
    assert set(git('diff','--cached','--name-only').decode().splitlines())==set(paths)
    subprocess.run(['git','diff','--quiet','--',*paths],cwd=WT,check=True)
    message=HERE/'source-message.txt'
    message.write_text('fix(call-graph): preserve declaration identity across traversal hops\n\nKeep provider query coordinates separate from visited identity, preserve LSP endpoints through completed adjacency, and resolve bounded Rust call-site context. Schedule semantic endpoints before display aggregation. Disclose unsupported lexical bindings.\n\nAdd behavioral guards after observed review survivors; final related95/0, fifteen applied mutations killed, source full gate and native identity probes passed. Baseline snapshot lease correction: bug-fix-session-log:F-231.\n\nSession-Id: codex:01a1bf68-e650-7f20-911e-b53f86597c27\n')
    subprocess.run(['git','commit','-F',str(message),'--',*paths],cwd=WT,check=True)
    sha=git('rev-parse','HEAD').decode().strip()
    patch=git('show','--format=','--binary',sha)
    patch_id=subprocess.check_output(['git','patch-id','--stable'],input=patch).decode().split()[0]
    changed=git('diff-tree','--no-commit-id','--name-only','-r',sha).decode().splitlines()
    assert set(changed)==set(paths)
    assert {p:hashlib.sha256(git('show',sha+':'+p)).hexdigest() for p in paths}==frozen['paths']
    (HERE/'source-commit.json').write_text(json.dumps(dict(sha=sha,tree=git('rev-parse',sha+'^{tree}').decode().strip(),patch_id=patch_id,paths=changed),indent=2)+'\n')
    print(sha,patch_id)
else:raise SystemExit('stage or commit')
