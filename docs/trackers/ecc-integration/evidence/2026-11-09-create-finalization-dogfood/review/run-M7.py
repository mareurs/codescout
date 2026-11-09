import pathlib, subprocess, os, json, hashlib, re, datetime
e=pathlib.Path(__file__).parent
root=pathlib.Path('/home/marius/work/claude/codescout/.worktrees/ecc-create-finalization-review')
original=(e/'candidate.rs').read_bytes()
spec=json.loads((e/'M7-registration-before-transaction.json').read_text())
env=os.environ.copy();env['CARGO_BUILD_JOBS']='4'
cmd=['./scripts/mutation-probe.sh','--strict','--file','src/librarian/tools/create.rs','--find',spec['find'],'--replace',spec['replace'],'--','cargo','test','--lib','librarian::tools::create::tests::','--','--test-threads=4']
name='M7-registration-before-transaction'
started=datetime.datetime.now(datetime.timezone.utc).isoformat()
with (e/(name+'.log')).open('wb') as log:
 p=subprocess.run(cmd,cwd=root,env=env,stdout=log,stderr=subprocess.STDOUT)
out=(e/(name+'.log')).read_text()
trees=re.findall(r'tree=(.+)',out)
r={'name':name,'started':started,'finished':datetime.datetime.now(datetime.timezone.utc).isoformat(),'exit_code':p.returncode,'summaries':re.findall(r'^test result:.*$',out,re.M),'verdicts':re.findall(r'^mutation-probe: (?:KILLED|SURVIVED|INCONCLUSIVE).*$',out,re.M),'source_restored':(root/'src/librarian/tools/create.rs').read_bytes()==original,'mutation_tree':trees[-1] if trees else None}
if trees:
 restored=(pathlib.Path(trees[-1])/'src/librarian/tools/create.rs').read_bytes()
 r['isolated_source_restored']=restored==original
 r['isolated_restored_sha256']=hashlib.sha256(restored).hexdigest()
(e/'M7-result.json').write_text(json.dumps(r,indent=2)+'\n')
print(json.dumps(r),flush=True)
