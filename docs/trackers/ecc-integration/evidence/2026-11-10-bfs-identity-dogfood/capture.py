"""Freeze this repair's exact inputs before production or test edits; never run a replay."""
import hashlib,json,subprocess
from pathlib import Path
HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[4]
WT=ROOT/'.worktrees/ecc-bfs-identity-20261110'
BASE='7f0dbba5fd6ba25e4a50a8aad5c25403a11ae345'
def git(*args,cwd=ROOT):return subprocess.check_output(['git',*args],cwd=cwd)
assert git('rev-parse','HEAD',cwd=WT).decode().strip()==BASE
assert not git('status','--porcelain',cwd=WT)
assert not (HERE/'baseline.json').exists()
inputs=['CLAUDE.md','AGENTS.md','Cargo.toml','Cargo.lock','src/tools/symbol/call_graph/mod.rs','src/tools/symbol/call_graph/traversal.rs','src/tools/symbol/call_edges/resolver.rs','src/tools/symbol/call_edges/cache.rs','src/lsp/ops.rs','src/lsp/symbols.rs','src/lsp/mock.rs','src/ast/parser.rs']
records=[]
for path in inputs:
    data=git('show',BASE+':'+path)
    assert data==(WT/path).read_bytes()
    target=HERE/'baseline-inputs'/(path.replace('/','__')+'.txt')
    target.parent.mkdir(exist_ok=True)
    target.write_bytes(data)
    records.append(dict(path=path,git_blob=git('rev-parse',BASE+':'+path).decode().strip(),sha256=hashlib.sha256(data).hexdigest(),snapshot=str(target.relative_to(HERE))))
(HERE/'baseline-inputs/candidate-brief.txt').write_text('Investigate and correct call_graph multi-hop traversal losing distinct same-name declarations. Preserve declaration identity across hops and completed-cache use, existing direction/path/cap behavior and valid empty results. Disclose unresolved or ambiguous next-hop analysis instead of choosing an arbitrary definition. Verify with real isolated execution and actual applied mutations. No repair replay, push, shared runtime or profile changes.\n')
skills=[]
for name in ['ecc-spec-miner','ecc-silent-failure-review']:
    path=Path('/home/marius/.codex/plugins/cache/ecc-codescout-pilot/ecc-codescout/0.1.0/skills')/name/'SKILL.md'
    data=path.read_bytes()
    target=HERE/'controller-only'/(name+'.txt')
    target.parent.mkdir(exist_ok=True)
    target.write_bytes(data)
    skills.append(dict(name=name,path=str(path),sha256=hashlib.sha256(data).hexdigest(),use='explicit installed-body use; not proof of automatic residency'))
ref='refs/ecc-replay/bfs-identity-20261110'
subprocess.run(['git','update-ref',ref,BASE,'0'*40],cwd=ROOT,check=True)
foreign={p:git('ls-files','--stage','--',p).decode().strip() for p in git('diff','--cached','--name-only').decode().splitlines()}
(HERE/'foreign-index-before.json').write_text(json.dumps(foreign,indent=2)+'\n')
private=ROOT/'.codescout/ecc-bfs-identity-private-20261110'
private.mkdir(mode=0o700)
(private/'.gitignore').write_text('*\n')
source=Path('/home/marius/.codex/sessions/2026/11/09/rollout-2026-11-09T10-24-45-01a1bf68-e650-7f20-911e-b53f86597c27.jsonl')
lines=source.read_bytes().splitlines(keepends=True)
rows=[json.loads(line) for line in lines]
matches=[i for i,row in enumerate(rows) if row.get('type')=='response_item' and row.get('payload',{}).get('role')=='user' and '\n'.join(b.get('text','') for b in row['payload'].get('content',[]) if b.get('type')=='input_text').strip()=='ok, continue']
assert matches
start=matches[-1]
end=next(i for i in range(start+1,len(rows)) if rows[i].get('type')=='response_item' and (rows[i].get('payload',{}).get('role')=='assistant' or rows[i].get('payload',{}).get('type')=='function_call'))
data=b''.join(lines[:end])
snapshot=private/'pre-action-controller-input.jsonl'
snapshot.write_bytes(data)
snapshot.chmod(0o600)
context=[r['payload'] for r in rows[:end] if r.get('type')=='turn_context'][-1]
provenance=dict(source=str(source),private_snapshot=str(snapshot),bytes=len(data),sha256=hashlib.sha256(data).hexdigest(),mode='0600',cutoff='before first assistant message OR function_call after latest exact ok, continue event',first_excluded_type=rows[end]['payload'].get('type'),user_event_timestamp=rows[start].get('timestamp'),configured_model=context.get('model'),configured_effort=context.get('effort'),limits='Exact local host prefix, not serialized provider request or all-resident-history attestation. Prior ECC conditioning and solution/evaluator artifacts require explicit exclusion/control for future comparison. No replay ran.')
(HERE/'session-input-provenance.json').write_text(json.dumps(provenance,indent=2)+'\n')
result=dict(base_commit=BASE,base_tree=git('rev-parse',BASE+'^{tree}').decode().strip(),replay_ref=ref,workspace=str(WT),source_clean_before_edits=True,inputs=records,skills=skills,scope='Multi-hop declaration identity correction with explicit uncertainty; no causal ECC claim, repair replay, push or shared runtime/config changes.')
(HERE/'baseline.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps(dict(base=BASE,inputs=len(records),skills=len(skills),private_input_bytes=len(data),foreign_staged_paths=len(foreign),replay_ran=False)))
