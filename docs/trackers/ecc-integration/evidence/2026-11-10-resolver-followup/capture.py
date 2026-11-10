"""Freeze the repair base and private pre-action input; never run a repair replay."""
import hashlib,json,subprocess
from pathlib import Path
HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[4]
WORKTREE=ROOT/'.worktrees/ecc-resolver-followup-20261110'
BASE='5b40efc48d31ccdcebbf9a9a2429c768d7aa6ca3'
def git(*args,cwd=ROOT): return subprocess.check_output(['git',*args],cwd=cwd)
assert git('rev-parse','HEAD',cwd=WORKTREE).decode().strip()==BASE
assert not git('status','--porcelain',cwd=WORKTREE)
assert not (HERE/'baseline.json').exists()
inputs=['CLAUDE.md','AGENTS.md','Cargo.toml','Cargo.lock','src/tools/symbol/call_edges/resolver.rs','src/tools/symbol/call_graph/mod.rs','src/tools/symbol/call_graph/traversal.rs','src/lsp/mock.rs']
records=[]
for path in inputs:
    data=git('show',BASE+':'+path)
    assert data==(WORKTREE/path).read_bytes()
    target=HERE/'baseline-inputs'/(path.replace('/','__')+'.txt')
    target.parent.mkdir(exist_ok=True)
    target.write_bytes(data)
    records.append(dict(path=path,git_blob=git('rev-parse',BASE+':'+path).decode().strip(),sha256=hashlib.sha256(data).hexdigest(),snapshot=str(target.relative_to(HERE))))
(HERE/'baseline-inputs/candidate-brief.txt').write_text('Reconcile the one-hop resolver strategy documentation with current implementation. Separately inspect multi-hop path-pinned graph identity with duplicate Rust names, using isolated current-source execution before deciding whether a behavioral correction is justified. Preserve valid empty results and error distinctions. Record exact evidence and replay provenance. No repair replay or shared runtime/config changes.\n')
skills=[]
for name in ['ecc-spec-miner','ecc-silent-failure-review']:
    path=Path('/home/marius/.codex/plugins/cache/ecc-codescout-pilot/ecc-codescout/0.1.0/skills')/name/'SKILL.md'
    data=path.read_bytes()
    target=HERE/'controller-only'/(name+'.txt')
    target.parent.mkdir(exist_ok=True)
    target.write_bytes(data)
    skills.append(dict(name=name,path=str(path),sha256=hashlib.sha256(data).hexdigest(),use='explicit installed-body use, not proof of automatic residency'))
ref='refs/ecc-replay/resolver-followup-20261110'
subprocess.run(['git','update-ref',ref,BASE,'0'*40],cwd=ROOT,check=True)
foreign={p:git('ls-files','--stage','--',p).decode().strip() for p in git('diff','--cached','--name-only').decode().splitlines()}
(HERE/'foreign-index-before.json').write_text(json.dumps(foreign,indent=2)+'\n')
private=ROOT/'.codescout/ecc-resolver-followup-private-20261110'
private.mkdir(mode=0o700)
(private/'.gitignore').write_text('*\n')
source=Path('/home/marius/.codex/sessions/2026/11/09/rollout-2026-11-09T10-24-45-01a1bf68-e650-7f20-911e-b53f86597c27.jsonl')
lines=source.read_bytes().splitlines(keepends=True)
rows=[json.loads(line) for line in lines]
matches=[i for i,row in enumerate(rows) if row.get('type')=='response_item' and row.get('payload',{}).get('role')=='user' and '\n'.join(b.get('text','') for b in row['payload'].get('content',[]) if b.get('type')=='input_text').strip()=='ok,  continua']
assert matches
start=matches[-1]
end=next(i for i in range(start+1,len(rows)) if rows[i].get('type')=='response_item' and rows[i].get('payload',{}).get('role')=='assistant')
data=b''.join(lines[:end])
snapshot=private/'pre-action-controller-input.jsonl'
snapshot.write_bytes(data)
snapshot.chmod(0o600)
context=[r['payload'] for r in rows[:end] if r.get('type')=='turn_context'][-1]
provenance=dict(source=str(source),private_snapshot=str(snapshot),bytes=len(data),sha256=hashlib.sha256(data).hexdigest(),mode='0600',cutoff='before first assistant response to latest exact ok,  continua event',user_event_timestamp=rows[start].get('timestamp'),configured_model=context.get('model'),configured_effort=context.get('effort'),limits='Host history, not serialized provider request or proof all history was resident. Earlier ECC conditioning/solutions require explicit exclusion/control in any future comparison; no replay ran.')
(HERE/'session-input-provenance.json').write_text(json.dumps(provenance,indent=2)+'\n')
result=dict(base_commit=BASE,base_tree=git('rev-parse',BASE+'^{tree}').decode().strip(),replay_ref=ref,workspace=str(WORKTREE),source_clean_before_edits=True,inputs=records,skills=skills,scope='Documentation correction and bounded BFS investigation; no causal ECC claim, repair replay, push or shared runtime/config change.')
(HERE/'baseline.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps(dict(base=BASE,inputs=len(records),skills=len(skills),private_input_bytes=len(data),foreign_staged_paths=len(foreign),replay_ran=False)))
