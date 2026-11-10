"""Repair missing staging attribution only for positively owned byte-reviewed experiment paths."""
import hashlib,json,subprocess
from pathlib import Path
HERE=Path(__file__).resolve().parent;ROOT=HERE.parents[4];PRIVATE=ROOT/'.codescout/ecc-bfs-identity-private-20261110'
owned=set(json.loads((HERE/'owned-paths.json').read_text()))
foreign=json.loads((HERE/'foreign-index-before.json').read_text())
def git(*a):return subprocess.check_output(['git',*a],cwd=ROOT)
def foreign_state():return {p:git('ls-files','--stage','--',p).decode().strip() for p in foreign}
def classify():return [r.split('\t',2) for r in subprocess.check_output(['bash','scripts/pre-commit-foreign-index.sh','--classify'],cwd=ROOT).decode().splitlines()]
control=json.loads((PRIVATE/'stage-control.json').read_text())
assert control['after']==[['mine','codex:01a1bf68-e650-7f20-911e-b53f86597c27',control['path']]]
assert control['worktree_unchanged'] and control['foreign_index_unchanged']
assert foreign_state()==foreign
before_patch=git('diff','--cached','--binary','--',*sorted(owned))
assert before_patch==(PRIVATE/'evidence-cached.patch').read_bytes()
rows=[r for r in classify() if r[2] in owned]
unknown=[r[2] for r in rows if r[0]=='theirs']
assert unknown and all(r[1]=='-' for r in rows if r[0]=='theirs')
assert all((ROOT/p).is_file() for p in unknown),'no deletion is re-staged through missing-path add'
hashes={p:hashlib.sha256((ROOT/p).read_bytes()).hexdigest() for p in unknown}
(HERE/'evidence-commit-initial-refusal.log').write_bytes((PRIVATE/'evidence-commit.log').read_bytes())
(HERE/'staging-attribution-control.json').write_bytes((PRIVATE/'stage-control.json').read_bytes())
print('REPAIR_OWNED_UNKNOWN_PATHS',len(unknown),flush=True)
# Only index entries are changed. Working-tree files and the six foreign entries stay intact.
subprocess.run(['git','restore','--staged','--',*unknown],cwd=ROOT,check=True)
assert {p:hashlib.sha256((ROOT/p).read_bytes()).hexdigest() for p in unknown}==hashes
assert foreign_state()==foreign
subprocess.run(['git','add','--',*unknown],cwd=ROOT,check=True)
after=[r for r in classify() if r[2] in owned]
assert len(after)==len(owned)
assert all(r[:2]==['mine','codex:01a1bf68-e650-7f20-911e-b53f86597c27'] for r in after),[r for r in after if r[0]!='mine']
assert git('diff','--cached','--binary','--',*sorted(owned))==before_patch
assert {p:hashlib.sha256((ROOT/p).read_bytes()).hexdigest() for p in unknown}==hashes
assert foreign_state()==foreign
record=dict(control='staging-attribution-control.json',unknown_owned_paths_repaired=unknown,owned_population=len(owned),owned_after_all_mine=True,worktree_hashes_unchanged=True,scoped_staged_patch_sha256=hashlib.sha256(before_patch).hexdigest(),scoped_staged_patch_unchanged=True,foreign_index_unchanged=True,mechanism='Explicit named restore --staged and add for positively owned reviewed files; no direct ownership-log edit, wildcard ack, context impersonation or hook bypass',limits='Recorder classified original rows as owner minus / not-staging. Why their original staging observation was unreadable is not established; no named peer author was observed.')
(HERE/'staging-attribution-repair.json').write_text(json.dumps(record,indent=2)+'\n')
print(json.dumps({k:v for k,v in record.items() if k!='unknown_owned_paths_repaired'},indent=2),flush=True)
