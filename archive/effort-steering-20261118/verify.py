#!/usr/bin/env python3
"""Verify custody and scan every added payload, including nested compressed members."""
import gzip
import argparse
import hashlib
import io
import json
from pathlib import Path
import re
import stat
import subprocess
import tarfile
import zipfile

DEST=Path('/home/marius/work/claude/codescout.worktrees/effort-steering-archive-20261118')
META=DEST/'archive/effort-steering-20261118'
PATTERNS={
    'private_key':rb'-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----',
    'anthropic_token':rb'sk-ant-[A-Za-z0-9_-]{30,}',
    'openai_token':rb'(?<![A-Za-z0-9_-])sk-(?:proj-|svcacct-)?[A-Za-z0-9_-]{32,}',
    'github_token':rb'(?:gh[pousr]_[A-Za-z0-9]{30,}|github_pat_[A-Za-z0-9_]{50,})',
    'aws_access_key':rb'AKIA[0-9A-Z]{16}',
    'jwt':rb'eyJ[A-Za-z0-9_-]{12,}\.[A-Za-z0-9_-]{16,}\.[A-Za-z0-9_-]{16,}',
    'bearer_value':rb'(?i)bearer[ \t]+[A-Za-z0-9._~+/=-]{24,}',
    'assigned_token':rb'(?i)(?:api[_-]?key|s5[_-]?token|auth[_-]?token|access[_-]?token)[\x22\x27]?[ \t]*[:=][ \t]*[\x22\x27][A-Za-z0-9._~+/=-]{24,}[\x22\x27]',
}
RX={k:re.compile(v) for k,v in PATTERNS.items()}
findings=[]
seen=set()
scanned=0
payload_bytes=0

def digest(p):
    if p.is_symlink():
        import os
        return hashlib.sha256(os.fsencode(os.readlink(p))).hexdigest()
    with p.open('rb') as f:return hashlib.file_digest(f,'sha256').hexdigest()

def scan(name,data,depth=0):
    global scanned,payload_bytes
    assert depth<15, ('nesting limit',name)
    key=hashlib.sha256(data).digest()
    if key in seen:return
    seen.add(key); scanned+=1;payload_bytes+=len(data)
    for kind,rx in RX.items():
        for m in rx.finditer(data):
            # Never print credential material; report positions and hashes only.
            findings.append(dict(path=name,kind=kind,offset=m.start(),match_sha256=hashlib.sha256(m.group()).hexdigest()))
    if data.startswith(b'\x1f\x8b'):
        raw=gzip.decompress(data)
        try:
            with tarfile.open(fileobj=io.BytesIO(raw),mode='r:') as t:
                for m in t:
                    if m.isfile():scan(name+'!'+m.name,t.extractfile(m).read(),depth+1)
        except tarfile.ReadError:
            scan(name+'!gunzip',raw,depth+1)
    elif data.startswith(b'PK\x03\x04'):
        with zipfile.ZipFile(io.BytesIO(data)) as z:
            for m in z.infolist():
                if not m.is_dir():scan(name+'!'+m.filename,z.read(m),depth+1)
    elif name.endswith('.tar'):
        with tarfile.open(fileobj=io.BytesIO(data),mode='r:') as t:
            for m in t:
                if m.isfile():scan(name+'!'+m.name,t.extractfile(m).read(),depth+1)

def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('--retained-only',action='store_true',help='Do not require retired original paths after cleanup')
    args=parser.parse_args()
    manifest=json.loads((META/'custody-manifest.json').read_text())
    rows=manifest['files']; errors=[]; tar_cache={}
    for r in rows:
        source=Path(r['source_root'])/r['path']
        if not args.retained_only and digest(source)!=r['sha256']:errors.append(['source-changed',r['scope'],r['path']])
        if r['storage'] in ['git_tar','external_tar']:
            k=r['destination']
            if k not in tar_cache:
                t=tarfile.open(Path(k) if r['storage']=='external_tar' else DEST/k,'r:gz')
                tar_cache[k]={}
                for m in t:
                    if m.issym():
                        h=hashlib.sha256(m.linkname.encode()).hexdigest()
                    else:
                        with t.extractfile(m) as f:h=hashlib.file_digest(f,'sha256').hexdigest()
                    tar_cache[k][m.name]=h
                t.close()
            h=tar_cache[k][r['member']]
        else:
            p=Path(r['destination']) if r['storage']=='external' else DEST/r['destination']
            h=digest(p)
        if h!=r['sha256']:errors.append(['destination-mismatch',r['scope'],r['path']])
    bundle=manifest['external_history_bundle']
    assert digest(Path(bundle['path']))==bundle['sha256']
    legacy=manifest.get('external_legacy_capsule')
    if legacy:
        assert digest(Path(legacy['path']))==legacy['sha256']
    candidates=set(subprocess.check_output(['git','-C',str(DEST),'ls-files','--others','--exclude-standard','-z']).decode().strip('\0').split('\0'))
    candidates.update(subprocess.check_output(['git','-C',str(DEST),'diff','--name-only','-z',manifest['published_base']]).decode().strip('\0').split('\0'))
    candidates.discard('')
    for rel in sorted(candidates):
        p=DEST/rel
        if p.is_file() and not p.is_symlink():scan(rel,p.read_bytes())
    allow_path=META/'reviewed-patterns.json'
    reviewed=json.loads(allow_path.read_text()) if allow_path.exists() else {}
    unresolved=[f for f in findings if f['match_sha256'] not in reviewed]
    report=dict(custody_rows=len(rows),originals_compared=not args.retained_only,custody_errors=errors,publish_files=len(candidates),
                distinct_payloads_scanned=scanned,recursive_payload_bytes=payload_bytes,
                credential_pattern_findings=findings,unresolved_credential_pattern_findings=unresolved,
                limits='Pattern scan, not proof of absence of every possible credential. Deduplicates identical payload SHA-256 values. External large capsules and history bundle are not Git payloads.')
    (META/'verification.json').write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps(report,indent=2))
    raise SystemExit(1 if errors or unresolved else 0)

if __name__=='__main__':main()
