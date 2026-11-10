import pathlib,json,subprocess,os,hashlib,difflib
r=pathlib.Path("/home/marius/work/claude/codescout/.worktrees/ecc-bfs-review-20261110");e=pathlib.Path("/home/marius/work/claude/codescout/docs/trackers/ecc-integration/evidence/2026-11-10-bfs-identity-dogfood/review")
file='src/tools/symbol/call_graph/identity_tests.rs';source=(r/file).read_text()
start=source.index('#[tokio::test]\nasync fn lsp_same_file_same_line_helpers_preserve_columns_and_cached_target_metadata()')
old=source[start:];new=old.replace('outgoing(b, 3, 26)','outgoing(b, 3, 13)')
new=new.replace('        "entry>helper@fixture.rs:4#1",\n        "entry>helper@fixture.rs:4#1",','        "entry>helper@fixture.rs:4#1",')
assert new!=old
(e/'D01.patch').write_text(''.join(difflib.unified_diff(old.splitlines(True),new.splitlines(True),fromfile=file,tofile=file)))
cmd=['./scripts/mutation-probe.sh','--strict','--file',file,'--find',old,'--replace',new,'--','./scripts/with-slot.sh','--slot','1','cargo','test','--no-default-features','--lib','tools::symbol::call_graph::identity_tests::lsp_same_file_same_line_helpers_preserve_columns_and_cached_target_metadata','--','--exact','--nocapture']
env=os.environ.copy();env.pop('CARGO_TARGET_DIR',None);env['TMPDIR']='/home/marius/.cache/codescout-ecc-tmp';env['CODESCOUT_GATE_TMPFS_BASE']='off'
with (e/'D01.log').open('wb') as f:p=subprocess.run(cmd,cwd=r,env=env,stdout=f,stderr=subprocess.STDOUT)
raw=(e/'D01.log').read_bytes();(e/'D01-result.json').write_text(json.dumps(dict(exit=p.returncode,log_sha256=hashlib.sha256(raw).hexdigest(),source_restored=(r/file).read_text()==source,scope='Diagnostic test fixture alteration, excluded from production mutation population',command=cmd),indent=2))
print('D01 exit',p.returncode)
