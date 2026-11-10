"""Validate explicit owned current docs and immutable evidence; historical captures remain historical."""
import hashlib,json,re,subprocess,yaml
from pathlib import Path
HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[4]
DOCS=json.loads((HERE/'validation-docs.json').read_text())
OLD='docs/issues/2026-11-09-call-graph-resolver-doc-denies-outgoing-ts-fallback.md'
OLD_GATE='docs/issues/2026-11-10-observation-store-subprocess-writer-lean-gate-failure.md'
LEDGER='docs/trackers/bug-fix-session-log.md'
def git(*args,check=True):return subprocess.run(['git',*args],cwd=ROOT,check=check,capture_output=True)
links=0
integration=json.loads((HERE/'integration.json').read_text())['integration']
for path,expected in DOCS.items():
    text=(ROOT/path).read_text()
    fm=yaml.safe_load(text.split('---\n',2)[1])
    assert fm['id']==expected,path
    for target in re.findall(r'\[[^\]]*\]\(([^)\n]+)\)',text):
        if re.match(r'^[a-z]+:',target) or target.startswith('#'):continue
        assert (ROOT/path).parent.joinpath(target.split('#',1)[0].strip('<>')).exists(),(path,target)
        links+=1
    for old in ['81436f2c'+'48ad8c74','d0b8e235'+'49eaa576']:
        assert old not in text,path
    if '/archive/' in path:
        assert fm['status']=='fixed' and not fm.get('claimed_by')
        assert integration['commit'] in text and integration['stable_patch_id'] in text
assert not (ROOT/OLD).exists() and not (ROOT/OLD_GATE).exists()
ledger=(ROOT/LEDGER).read_text()
assert '**Status:** mitigated' in ledger.split('## F-228 —',1)[1].split('\n## ',1)[0]
assert '**Status:** fixed-verified' in ledger.split('## F-229 —',1)[1].split('\n## ',1)[0]
before=json.loads((HERE/'foreign-index-before.json').read_text())
assert {p:git('ls-files','--stage','--',p).stdout.decode().strip() for p in before}==before
private='.codescout/ecc-resolver-followup-private-20261110/'
assert not git('ls-files','--',private).stdout
assert git('check-ignore','--',private+'pre-action-controller-input.jsonl',check=False).returncode==0
prov=json.loads((HERE/'session-input-provenance.json').read_text())
data=Path(prov['private_snapshot']).read_bytes()
assert len(data)==prov['bytes'] and hashlib.sha256(data).hexdigest()==prov['sha256']
assert (Path(prov['private_snapshot']).stat().st_mode&0o777)==0o600
patterns=[rb'\bsk-[A-Za-z0-9_-]{32,}\b',rb'\bgh[pousr]_[A-Za-z0-9]{30,}\b',rb'-----BEGIN (?:RSA |OPENSSH )?PRIVATE KEY-----',rb'## Three Codex Instances',rb'Private notes for this machine']
samples=[b'sk-'+b'a'*40,b'ghp_'+b'b'*40,b'-----BEGIN OPENSSH PRIVATE KEY-----',b'## Three Codex Instances',b'Private notes for this machine']
assert all(re.search(p,s) for p,s in zip(patterns,samples))
raw={p for p in HERE.rglob('*') if p.is_file() and '__pycache__' not in p.parts and p.suffix!='.pyc'}
scanned=0
for path in raw|{ROOT/p for p in DOCS}:
    assert not path.is_symlink()
    # Both validators contain deliberate positive controls, not discovered secrets.
    if path.name in ['validate.py','validate_final.py']:continue
    assert not any(re.search(p,path.read_bytes()) for p in patterns),str(path)
    scanned+=1
result=dict(managed_documents_checked=len(DOCS),local_markdown_targets_checked=links,foreign_staged_blobs_unchanged=len(before),private_prefix_hash_size_mode_verified=True,private_ignored_and_untracked=True,privacy_files_scanned=scanned,privacy_positive_controls=len(patterns),privacy_hits=0,privacy_limits='Selected patterns only. Scanner excludes two validators containing deliberate positive literals. Hot ledger checks only owned sections; historical links are outside this change.')
(HERE/'validation-result.json').write_text(json.dumps(result,indent=2)+'\n')
raw={p for p in HERE.rglob('*') if p.is_file() and '__pycache__' not in p.parts and p.suffix!='.pyc'}|{HERE/'manifest.json',HERE/'owned-paths.json'}
# Only OLD existed in HEAD; the newly filed gate incident was moved before its first commit.
assert git('cat-file','-e','HEAD:'+OLD).returncode==0
assert git('cat-file','-e','HEAD:'+OLD_GATE,check=False).returncode!=0
owned=sorted(set(DOCS)|{OLD,LEDGER}|{p.relative_to(ROOT).as_posix() for p in raw})
(HERE/'owned-paths.json').write_text(json.dumps(owned,indent=2)+'\n')
manifest=[dict(path=p.relative_to(HERE).as_posix(),bytes=p.stat().st_size,sha256=hashlib.sha256(p.read_bytes()).hexdigest()) for p in sorted(raw) if p.name!='manifest.json']
(HERE/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
print(json.dumps(dict(result,raw_manifest_records=len(manifest),owned_paths=len(owned)),indent=2))
