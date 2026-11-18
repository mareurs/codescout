#!/usr/bin/env python3
"""One-off, non-destructive custody snapshot. Removal is a separate reviewed step."""
import hashlib
import io
import json
import os
from pathlib import Path
import re
import shutil
import sqlite3
import stat
import subprocess
import tarfile
from datetime import datetime, timezone

MAIN = Path('/home/marius/work/claude/codescout')
RESEARCH = Path('/home/marius/.codex/worktrees/2aa6/codescout')
DEST = Path('/home/marius/work/claude/codescout.worktrees/effort-steering-archive-20261118')
EXTERNAL = Path('/home/marius/archives/codescout/effort-steering-20261118')
META = Path('archive/effort-steering-20261118')
BASE = '034e4783f9ba65d1b8da735263c8bfed61e2ae58'
LIMIT = 32 * 1024 * 1024
ROWS = []

def git(root, *args):
    return subprocess.check_output(['git', '-C', str(root), *args])

def digest(path):
    if path.is_symlink():
        return hashlib.sha256(os.fsencode(os.readlink(path))).hexdigest()
    with path.open('rb') as f:
        return hashlib.file_digest(f, 'sha256').hexdigest()

def files(root):
    # Do not dereference links or include transient bytecode caches.
    if root.is_file() or root.is_symlink():
        yield root
    elif root.is_dir():
        for p in sorted(root.rglob('*')):
            if '__pycache__' not in p.parts and not p.name.endswith('.pyc') and (p.is_file() or p.is_symlink()):
                yield p

def record(root, path, scope, **storage):
    s = path.lstat()
    row = dict(scope=scope, source_root=str(root), path=str(path.relative_to(root)),
               type='symlink' if path.is_symlink() else 'file', bytes=s.st_size,
               mode=stat.S_IMODE(s.st_mode), sha256=digest(path), **storage)
    if path.is_symlink():
        row['link_target'] = os.readlink(path)
    ROWS.append(row)
    return row

def copy(root, path, scope):
    rel = path.relative_to(root)
    large = path.stat().st_size > LIMIT
    dest = EXTERNAL/'capsules'/rel if large else DEST/rel
    dest.parent.mkdir(parents=True, exist_ok=True)
    row = record(root, path, scope, storage='external' if large else 'git', destination=str(dest if large else rel))
    if dest.exists() or dest.is_symlink():
        if digest(dest) != row['sha256']:
            if large:
                raise RuntimeError(f'External destination differs: {dest}')
            shutil.copy2(path, dest, follow_symlinks=False)
    else:
        subprocess.run(['cp', '--reflink=auto', '--preserve=mode,timestamps', '--no-dereference', '--', str(path), str(dest)], check=True)
    assert digest(path) == row['sha256'] == digest(dest), str(path)
    if large:
        pointer = DEST/(str(rel)+'.external.json')
        pointer.write_text(json.dumps(row, indent=2)+'\n')
    return row

def pack(root, paths, scope, filename):
    target = DEST/META/filename
    members = sorted(set(p for d in paths for p in files(d)))
    with tarfile.open(target, 'w:gz', dereference=False) as t:
        for p in members:
            row = record(root, p, scope, storage='git_tar', destination=str(META/filename), member=str(p.relative_to(root)))
            t.add(p, arcname=row['member'], recursive=False)
            assert digest(p) == row['sha256'], str(p)
    with tarfile.open(target, 'r:gz') as t:
        by_name = {r['member']:r for r in ROWS if r.get('destination') == str(META/filename)}
        actual = t.getmembers()
        assert len(actual) == len(by_name)
        for m in actual:
            expected = by_name[m.name]
            if m.issym():
                h = hashlib.sha256(os.fsencode(m.linkname)).hexdigest()
            else:
                with t.extractfile(m) as f:
                    h = hashlib.file_digest(f, 'sha256').hexdigest()
            assert h == expected['sha256'], m.name

def catalog_snapshot(paths):
    c = sqlite3.connect('file:/home/marius/.local/share/librarian/catalog.db?mode=ro', uri=True)
    c.row_factory = sqlite3.Row
    c.execute('BEGIN')
    artifacts = [dict(r) for r in c.execute('SELECT * FROM artifact') if r['abs_path'] in paths]
    ids = {r['id'] for r in artifacts}
    slugs = {r['slug'] for r in artifacts}
    output = dict(source='read-only SQLite transaction', captured_at=datetime.now(timezone.utc).isoformat(), artifact=artifacts)
    for table, predicate in [
        ('artifact_augmentation', lambda r:r['artifact_id'] in ids),
        ('events', lambda r:r['artifact_id'] in ids),
        ('artifact_link', lambda r:r['src_id'] in ids or r['dst_id'] in ids),
        ('artifact_observation', lambda r:r['artifact_id'] in ids),
        ('entry_reservation', lambda r:r['artifact_id'] in ids),
        ('entry_cite', lambda r:r['src_slug'] in slugs),
    ]:
        output[table] = [dict(r) for r in c.execute('SELECT * FROM "'+table+'"') if predicate(r)]
    c.rollback()
    c.close()
    (DEST/META/'catalog-snapshot.json').write_text(json.dumps(output, indent=2, ensure_ascii=False)+'\n')
    print('Catalog snapshot:', {k:len(v) for k,v in output.items() if isinstance(v,list)}, flush=True)

def main():
    assert git(DEST,'rev-parse','HEAD').decode().strip() == BASE
    EXTERNAL.mkdir(parents=True, exist_ok=True)
    (DEST/META).mkdir(parents=True, exist_ok=True)
    extras = json.loads(Path('/tmp/effort-extra-docs-20261118.json').read_text())
    extras += ['docs/superpowers/specs/2026-11-06-executable-native-tmp-design.md',
               'docs/research/data/2026-11-04-hint-authority-gate.log.gz',
               'docs/research/data/2026-11-04-hint-authority-inputs.tar.gz']
    for p in files(MAIN/'docs/trackers/effort-steering'):
        copy(MAIN,p,'main-tracker')
    for rel in sorted(set(extras)):
        p = MAIN/rel
        if p.exists():
            copy(MAIN,p,'main-related-docs')
    main_sdd = [MAIN/'.superpowers/sdd'/n for n in [
        '2026-11-11-native-public-feedback-transfer',
        '2026-11-11-transport-headroom-correction',
        '2026-11-11-public-ledger-diagnosis']]
    pack(MAIN,main_sdd,'main-sdd','main-sdd-recipes.tar.gz')
    research_sdd = [p for p in (RESEARCH/'.superpowers/sdd').iterdir() if p.is_dir()]
    pack(RESEARCH,research_sdd,'research-sdd','research-sdd-recipes.tar.gz')
    pack(RESEARCH,[RESEARCH/'tools'],'research-code','research-tools.tar.gz')
    (DEST/META/'research-uncommitted.patch').write_bytes(git(RESEARCH,'diff','--binary','HEAD'))
    (DEST/META/'research-status.z').write_bytes(git(RESEARCH,'status','--porcelain=v1','-z','--untracked-files=all'))
    (DEST/META/'research-tree.txt').write_bytes(git(RESEARCH,'ls-tree','-r','HEAD'))
    (DEST/META/'research-history-provenance.txt').write_bytes(git(RESEARCH,'log','--format=fuller',BASE+'..HEAD'))
    (EXTERNAL/'main-index-before.patch').write_bytes(git(MAIN,'diff','--cached','--binary'))
    (EXTERNAL/'main-index-before.z').write_bytes(git(MAIN,'ls-files','--stage','-z'))
    bundle = EXTERNAL/'research-history.bundle'
    if not bundle.exists():
        subprocess.run(['git','-C',str(RESEARCH),'bundle','create',str(bundle),'HEAD','^'+BASE],check=True)
    subprocess.run(['git','-C',str(DEST),'bundle','verify',str(bundle)],check=True)
    catalog_snapshot({str(MAIN/r['path']) for r in ROWS if r['source_root']==str(MAIN)})
    manifest = dict(version=1, captured_at=datetime.now(timezone.utc).isoformat(), session='codex:01a15ca9-f939-7603-acb5-01c6b02f68e2',
                    published_base=BASE, branch='archive/effort-steering-20261118',
                    main_head_at_end=git(MAIN,'rev-parse','HEAD').decode().strip(),
                    research_head=git(RESEARCH,'rev-parse','HEAD').decode().strip(),
                    external_root=str(EXTERNAL), large_threshold_bytes=LIMIT,
                    external_history_bundle=dict(path=str(bundle),bytes=bundle.stat().st_size,sha256=digest(bundle)),
                    files=ROWS)
    (DEST/META/'custody-manifest.json').write_text(json.dumps(manifest,indent=2,ensure_ascii=False)+'\n')
    shutil.copy2(DEST/META/'custody-manifest.json',EXTERNAL/'custody-manifest.json')
    stats={}
    for r in ROWS:
        s=stats.setdefault(r['scope'],dict(files=0,bytes=0)); s['files']+=1;s['bytes']+=r['bytes']
    print(json.dumps(dict(groups=stats,external_capsules=sum(r['storage']=='external' for r in ROWS)),indent=2),flush=True)

if __name__ == '__main__':
    main()
