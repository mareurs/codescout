"""Validate the explicit owned document set, provenance, private boundary and raw manifest."""
import hashlib,json,re,subprocess,yaml
from pathlib import Path
HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[4]
DOC_IDS=json.loads((HERE/'validation-docs.json').read_text())
OTHER=['docs/trackers/bug-fix-session-log.md']
OLD='docs/issues/2026-11-09-call-graph-generic-call-returns-unrelated-callees.md'
def git(*args,check=True):
    return subprocess.run(['git',*args],cwd=ROOT,check=check,capture_output=True)
links=0
for rel,expected in DOC_IDS.items():
    text=(ROOT/rel).read_text()
    assert text.startswith('---\n')
    fm=yaml.safe_load(text.split('---\n',2)[1])
    assert fm['id']==expected,(rel,fm.get('id'))
    for target in re.findall(r'\[[^\]]*\]\(([^)\n]+)\)',text):
        if re.match(r'^[a-z]+:',target) or target.startswith('#'): continue
        path=target.split('#',1)[0].strip('<>')
        assert (ROOT/rel).parent.joinpath(path).exists(),(rel,target)
        links+=1
    assert not re.search(r'(?<![0-9a-f])'+'84029483'+'d2895d9b'+r'(?![0-9a-f])',text),rel
    if '/archive/' in rel:
        assert fm['status']=='fixed' and not fm.get('claimed_by')
        assert '4776e44bd' in text and '82470b4a164f780eab0ce409ca01dfafc29f0d7a' in text
assert not (ROOT/OLD).exists()
ledger=(ROOT/OTHER[0]).read_text()
assert len(re.findall(r'^## F-226 — ',ledger,re.M))==1
assert len(re.findall(r'^\| F-226 \|',ledger,re.M))==1
assert yaml.safe_load(ledger.split('---\n',2)[1])['entry_high_water_F']>=226
before=json.loads((HERE/'foreign-index-before.json').read_text())
assert {p:git('ls-files','--stage','--',p).stdout.decode().strip() for p in before}==before
private='.codescout/ecc-callgraph-identity-private-20261109/'
assert not git('ls-files','--',private).stdout
assert git('check-ignore','--',private+'pre-action-controller-input.jsonl',check=False).returncode==0
patterns=[rb'\bsk-[A-Za-z0-9_-]{32,}\b',rb'\bgh[pousr]_[A-Za-z0-9]{30,}\b',rb'-----BEGIN (?:RSA |OPENSSH )?PRIVATE KEY-----',rb'## Three Codex Instances',rb'Private notes for this machine']
samples=[b'sk-'+b'a'*40,b'ghp_'+b'b'*40,b'-----BEGIN OPENSSH PRIVATE KEY-----',b'## Three Codex Instances',b'Private notes for this machine']
assert all(re.search(p,s) for p,s in zip(patterns,samples))
raw={p for p in HERE.rglob('*') if p.is_file() and '__pycache__' not in p.parts and p.suffix!='.pyc'}
scanned=0
for path in raw|{ROOT/p for p in DOC_IDS}|{ROOT/p for p in OTHER}:
    assert not path.is_symlink()
    if path==Path(__file__).resolve(): continue
    assert not any(re.search(p,path.read_bytes()) for p in patterns),str(path)
    scanned+=1
result=dict(managed_documents_checked=len(DOC_IDS),local_markdown_targets_checked=links,ledger_entries_checked=['F-226'],foreign_index_blobs_unchanged=len(before),privacy_files_scanned=scanned,privacy_positive_controls=len(patterns),privacy_hits=0,privacy_limits='Selected credential/private-policy patterns; not universal secret detection. Scanner excludes its deliberate positive literals.',private_history_ignored_and_untracked=True,tracked_old_bug_path_absent=True)
(HERE/'validation-result.json').write_text(json.dumps(result,indent=2)+'\n')
raw={p for p in HERE.rglob('*') if p.is_file() and '__pycache__' not in p.parts and p.suffix!='.pyc'}|{HERE/'manifest.json',HERE/'owned-paths.json'}
owned=sorted(set(DOC_IDS)|set(OTHER)|{p.relative_to(ROOT).as_posix() for p in raw}|{OLD})
(HERE/'owned-paths.json').write_text(json.dumps(owned,indent=2)+'\n')
manifest=[dict(path=p.relative_to(HERE).as_posix(),bytes=p.stat().st_size,sha256=hashlib.sha256(p.read_bytes()).hexdigest()) for p in sorted(raw) if p.name!='manifest.json']
(HERE/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
print(json.dumps(dict(result,manifest_files=len(manifest),owned_paths=len(owned)),indent=2))
