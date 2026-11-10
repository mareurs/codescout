import pathlib,json,subprocess,os,re,hashlib,difflib,time
root=pathlib.Path("/home/marius/work/claude/codescout/.worktrees/ecc-bfs-review-20261110")
e=pathlib.Path("/home/marius/work/claude/codescout/docs/trackers/ecc-integration/evidence/2026-11-10-bfs-identity-dogfood/review")
manifest=json.loads((e.parent/'candidate-source.json').read_text())
def hashes(root): return {p:hashlib.sha256((root/p).read_bytes()).hexdigest() for p in manifest['paths']}
assert hashes(root)==manifest['paths']
baseline=(e/'baseline.log').read_bytes()
assert b'test result: ok. 87 passed; 0 failed;' in baseline
env=os.environ.copy();env.pop('CARGO_TARGET_DIR',None);env['TMPDIR']='/home/marius/.cache/codescout-ecc-tmp';env['CODESCOUT_GATE_TMPFS_BASE']='off'
results=[]
for m in json.loads((e/'mutations.json').read_text()):
    before=(root/m['file']).read_text()
    (e/(m['id']+'.patch')).write_text(''.join(difflib.unified_diff(before.splitlines(True),before.replace(m['find'],m['replace']).splitlines(True),fromfile=m['file'],tofile=m['file'])))
    cmd=['./scripts/mutation-probe.sh','--strict','--file',m['file'],'--find',m['find'],'--replace',m['replace'],'--','./scripts/with-slot.sh','--slot','1','cargo','test','--no-default-features','--lib','tools::symbol::call_']
    with (e/(m['id']+'.log')).open('wb') as f:
        p=subprocess.run(cmd,cwd=root,env=env,stdout=f,stderr=subprocess.STDOUT)
    raw=(e/(m['id']+'.log')).read_bytes(); s=raw.decode(errors='replace')
    summaries=re.findall(r'^test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out;.*$',s,re.M)
    last=summaries[-1] if summaries else None
    verdict='inconclusive' if not last or sum(map(int,last[:2]))!=87 else ('survived' if int(last[1])==0 else 'killed')
    tree=re.findall(r'mutation-probe: ARMED  mode=isolated  file=.*?  tree=(.*)',s)
    tree=tree[-1] if tree else None
    record=dict(m,exit=p.returncode,verdict=verdict,top_level_summary=last,log_sha256=hashlib.sha256(raw).hexdigest(),mutant_diff_sha256=hashlib.sha256((e/(m['id']+'.patch')).read_bytes()).hexdigest(),mutant_file_sha256=hashlib.sha256(before.replace(m['find'],m['replace']).encode()).hexdigest(),restored_root=hashes(root)==manifest['paths'],mutation_worktree=tree,restored_mutation_tree=hashes(pathlib.Path(tree))==manifest['paths'] if tree else None,command=cmd,failed_tests=re.findall(r'^test (tools::symbol::\S+) \.\.\. FAILED$',s,re.M))
    results.append(record);(e/'mutation-results.json').write_text(json.dumps(results,indent=2));print(m['id'],verdict,last,flush=True)
    assert record['restored_root'] and record['restored_mutation_tree'],record
print('DONE',flush=True)
