"""Review or commit the explicit evidence scope, then verify committed raw bytes and foreign index."""
import hashlib,json,subprocess,sys
from pathlib import Path
HERE=Path(__file__).resolve().parent;ROOT=HERE.parents[4]
PRIVATE=ROOT/'.codescout/ecc-bfs-identity-private-20261110'
def git(*a):return subprocess.check_output(['git',*a],cwd=ROOT)
owned=json.loads((HERE/'owned-paths.json').read_text())
foreign=json.loads((HERE/'foreign-index-before.json').read_text())
def check():
    assert {p:git('ls-files','--stage','--',p).decode().strip() for p in foreign}==foreign
    staged=set(git('diff','--cached','--name-only','--no-renames').decode().splitlines())
    assert staged-set(foreign)==set(owned),(staged-set(foreign)-set(owned),set(owned)-staged)
    subprocess.run(['git','diff','--quiet','--',*owned],cwd=ROOT,check=True)
    for row in json.loads((HERE/'raw-manifest.json').read_text())['records']:
        assert hashlib.sha256((ROOT/row['path']).read_bytes()).hexdigest()==row['sha256'],row['path']
    frozen=json.loads((HERE/'final-source.json').read_text())
    assert {p:hashlib.sha256((ROOT/p).read_bytes()).hexdigest() for p in frozen['paths']}==frozen['paths']
check()
patch=git('diff','--cached','--binary','--',*owned)
if sys.argv[1]=='review':
    (PRIVATE/'evidence-cached.patch').write_bytes(patch)
    print(git('diff','--cached','--stat','--',*owned).decode())
    print('OWNED',len(owned),'RAW_RECORDS',len(json.loads((HERE/'raw-manifest.json').read_text())['records']))
    print('FOREIGN_INDEX_UNCHANGED',True,'PATCH_SHA256',hashlib.sha256(patch).hexdigest())
elif sys.argv[1]=='commit':
    assert patch==(PRIVATE/'evidence-cached.patch').read_bytes()
    log=(HERE/'issue-frontmatter.log').read_bytes()
    assert b'test result: ok.' in log and b'0 failed;' in log
    # This check is immediately before the pathspec commit, independent of the earlier review.
    subprocess.run(['git','diff','--quiet','--',*owned],cwd=ROOT,check=True)
    subprocess.run(['git','commit','-F',str(HERE/'evidence-message.txt'),'--',*owned],cwd=ROOT,check=True)
    sha=git('rev-parse','HEAD').decode().strip()
    committed=set(git('diff-tree','--no-commit-id','--no-renames','--name-only','-r',sha).decode().splitlines())
    assert committed==set(owned),(committed-set(owned),set(owned)-committed)
    for name in owned:
        if not (ROOT/name).exists():
            assert not git('ls-tree','--name-only',sha,'--',name)
            continue
        assert git('show',sha+':'+name)==(ROOT/name).read_bytes(),name
    manifest=json.loads(git('show',sha+':'+str((HERE/'raw-manifest.json').relative_to(ROOT))))
    for row in manifest['records']:
        raw=git('show',sha+':'+row['path'])
        assert len(raw)==row['bytes'] and hashlib.sha256(raw).hexdigest()==row['sha256'],row['path']
    assert {p:git('ls-files','--stage','--',p).decode().strip() for p in foreign}==foreign
    integration=json.loads((HERE/'integration.json').read_text())
    subprocess.run(['git','merge-base','--is-ancestor',integration['integration'],sha],cwd=ROOT,check=True)
    subprocess.run(['git','diff','--quiet','--',*owned],cwd=ROOT,check=True)
    record=dict(evidence_commit=sha,integration=integration['integration'],source=integration['source'],owned_paths_verified=len(owned),committed_raw_records_verified=len(manifest['records']),foreign_index_unchanged=True,exact_source_hashes_verified=True,all_owned_worktree_paths_match_commit=True)
    (PRIVATE/'post-commit-verification.json').write_text(json.dumps(record,indent=2)+'\n')
    print(json.dumps(record,indent=2))
else:raise SystemExit('review or commit')
