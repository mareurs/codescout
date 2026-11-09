import pathlib, subprocess, os, json, hashlib, re, datetime
root=pathlib.Path('/home/marius/work/claude/codescout/.worktrees/ecc-create-finalization-review')
e=pathlib.Path(__file__).parent
source=root/'src/librarian/tools/create.rs'
original=source.read_bytes()
sha=lambda b: hashlib.sha256(b).hexdigest()
assert sha(original)=='68f4fdaa9030bd0bd78939ee7debb37c6dd5b30917db07e43278505fee27d3bd'
(e/'candidate.rs').write_bytes(original)
(e/'candidate.patch').write_bytes(subprocess.check_output(['git','diff','HEAD','--','src/librarian/tools/create.rs'],cwd=root))
env=os.environ.copy()
env['TMPDIR']='/home/marius/.cache/codescout-ecc-tmp'
env['CODESCOUT_GATE_TMPFS_BASE']='off'
env['CARGO_BUILD_JOBS']='4'
cmd=['cargo','test','--lib','librarian::tools::create::tests::','--','--test-threads=4']
results=[]
def run(name, command):
    started=datetime.datetime.now(datetime.timezone.utc).isoformat()
    with (e/(name+'.log')).open('wb') as log:
        p=subprocess.run(command,cwd=root,env=env,stdout=log,stderr=subprocess.STDOUT)
    output=(e/(name+'.log')).read_text()
    r={'name':name,'exit_code':p.returncode,'started':started,'finished':datetime.datetime.now(datetime.timezone.utc).isoformat(),'summaries':re.findall(r'^test result:.*$',output,re.M),'verdicts':re.findall(r'^mutation-probe: (?:KILLED|SURVIVED|INCONCLUSIVE).*$',output,re.M),'source_restored':source.read_bytes()==original}
    trees=re.findall(r'tree=(.+)',output)
    r['mutation_tree']=trees[-1] if trees else None
    if trees:
        restored=pathlib.Path(trees[-1])/'src/librarian/tools/create.rs'
        r['isolated_source_restored']=restored.read_bytes()==original
        r['isolated_restored_sha256']=sha(restored.read_bytes())
    results.append(r)
    (e/'results.json').write_text(json.dumps(results,indent=2)+'\n')
    print(json.dumps(r),flush=True)
    return r
r=run('baseline',cmd)
if r['exit_code'] or not any('51 passed; 0 failed' in s for s in r['summaries']):
    raise SystemExit('Baseline not green; refusing mutations')
for name,find,replacement in json.loads((e/'mutations.json').read_text()):
    assert original.decode().count(find)==1,(name,'nonunique')
    import difflib
    mutated=original.decode().replace(find,replacement)
    (e/(name+'.patch')).write_text(''.join(difflib.unified_diff(original.decode().splitlines(True),mutated.splitlines(True),fromfile='a/src/librarian/tools/create.rs',tofile='b/src/librarian/tools/create.rs')))
    r=run(name,['./scripts/mutation-probe.sh','--strict','--file','src/librarian/tools/create.rs','--find',find,'--replace',replacement,'--']+cmd)
    assert r['source_restored'],name
run('restored-control',cmd)
