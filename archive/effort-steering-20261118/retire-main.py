#!/usr/bin/env python3
"""Retire only manifest-bound main artifacts after verified branch publication."""
from pathlib import Path
import hashlib
import json
import os
import shutil
import subprocess
from datetime import datetime, timezone

MAIN=Path('/home/marius/work/claude/codescout')
WT=Path('/home/marius/work/claude/codescout.worktrees/effort-steering-archive-20261118')
EXT=Path('/home/marius/archives/codescout/effort-steering-20261118')
SHA='a1182ca93296f2d495cebef8bea68b93e902bb16'
META=Path('archive/effort-steering-20261118')

def digest(p):
    if p.is_symlink():return hashlib.sha256(os.fsencode(os.readlink(p))).hexdigest()
    with p.open('rb') as f:return hashlib.file_digest(f,'sha256').hexdigest()

def main():
    # Check the remote ref again. An unsuccessful network lookup aborts before any move.
    remote=subprocess.check_output(['git','-C',str(WT),'ls-remote','--heads','origin','refs/heads/archive/effort-steering-20261118'],text=True)
    assert remote.split()[0]==SHA,remote
    manifest_bytes=(WT/META/'custody-manifest.json').read_bytes()
    assert subprocess.check_output(['git','-C',str(WT),'show',SHA+':'+str(META/'custody-manifest.json')])==manifest_bytes
    manifest=json.loads(manifest_bytes)
    ops=json.loads(Path('/tmp/effort-main-cleanup-operations-20261118.json').read_text())
    keep={o['path'] for o in ops if o['action'] in ['index','cited-pointer']}
    removed={o['path'] for o in ops if o['action']=='retire'}
    # Validate the whole remaining population before moving a single member.
    moves=[];dir_rows=[]
    for r in manifest['files']:
        if r['source_root']!=str(MAIN):continue
        p=MAIN/r['path'];scope=r['scope']
        if scope in ['main-sdd','main-legacy-resident']:
            assert digest(p)==r['sha256'],str(p)
            dir_rows.append(r)
        elif scope=='main-tracker':
            if r['path'] in keep:continue
            if r['path'] in removed:
                assert not p.exists(),str(p)
                continue
            assert digest(p)==r['sha256'],str(p)
            dest=Path(r['destination']) if r['storage']=='external' else EXT/'main-original-files'/r['path']
            if dest.exists():assert digest(dest)==r['sha256'],str(dest)
            moves.append((p,dest,r))
        elif scope=='main-related-docs' and p.suffix!='.md':
            assert digest(p)==r['sha256'],str(p)
            dest=EXT/'main-original-files'/r['path']
            assert not dest.exists(),str(dest)
            moves.append((p,dest,r))
    roots=[MAIN/'.superpowers/sdd'/n for n in [
        '2026-11-11-native-public-feedback-transfer',
        '2026-11-11-transport-headroom-correction',
        '2026-11-11-public-ledger-diagnosis']]
    roots.append(MAIN/'.codescout/resident-private/blind-challenger-20261101')
    for root in roots:
        assert root.is_dir() and not root.is_symlink(),str(root)
        assert not (EXT/'main-original-directories'/root.relative_to(MAIN)).exists()
    known={r['path'] for r in manifest['files'] if r['source_root']==str(MAIN)}
    tracker=MAIN/'docs/trackers/effort-steering'
    unexpected=[str(p) for p in tracker.rglob('*') if (p.is_file() or p.is_symlink())
                and str(p.relative_to(MAIN)) not in known
                and p.suffix!='.pyc' and '__pycache__' not in p.parts]
    assert not unexpected,('new files not in custody manifest',unexpected)
    def journal(event):
        with (EXT/'main-retirement-progress.jsonl').open('a') as f:
            f.write(json.dumps(event)+'\n');f.flush();os.fsync(f.fileno())
    # No deleting: originals are renamed outside the checkout, and an existing
    # large capsule is replaced only after its identical hash was confirmed above.
    moved=[]
    for p,dest,r in moves:
        dest.parent.mkdir(parents=True,exist_ok=True)
        journal(dict(state='planned',source=str(p),destination=str(dest),sha256=r['sha256']))
        os.replace(p,dest)
        journal(dict(state='moved',source=str(p),destination=str(dest)))
        moved.append(dict(source=str(p),destination=str(dest),bytes=r['bytes'],sha256=r['sha256']))
    directories=[]
    for root in roots:
        dest=EXT/'main-original-directories'/root.relative_to(MAIN)
        dest.parent.mkdir(parents=True,exist_ok=True)
        journal(dict(state='planned-directory',source=str(root),destination=str(dest)))
        os.rename(root,dest)
        journal(dict(state='moved-directory',source=str(root),destination=str(dest)))
        rows=[r for r in dir_rows if (MAIN/r['path']).is_relative_to(root)]
        for r in rows:
            assert digest(dest/(MAIN/r['path']).relative_to(root))==r['sha256'],r['path']
        directories.append(dict(source=str(root),destination=str(dest),manifest_members_verified=len(rows)))
    # Preserve transient or otherwise unregistered leftovers too, except retained
    # redirect files. Do not silently discard a concurrent file.
    leftovers=[]
    for p in sorted(tracker.rglob('*')):
        if not (p.is_file() or p.is_symlink()) or str(p.relative_to(MAIN)) in keep:continue
        dest=EXT/'main-original-files'/p.relative_to(MAIN)
        assert not dest.exists(),str(dest)
        h=digest(p);dest.parent.mkdir(parents=True,exist_ok=True);os.rename(p,dest)
        leftovers.append(dict(source=str(p),destination=str(dest),sha256=h))
    for p in sorted((p for p in tracker.rglob('*') if p.is_dir()),key=lambda p:len(p.parts),reverse=True):
        try:p.rmdir()
        except OSError:pass
    empty_memory=MAIN/'.codescout/memories/research/effort-steering'
    if empty_memory.is_dir() and not any(empty_memory.iterdir()):empty_memory.rmdir()
    remaining=[str(p.relative_to(MAIN)) for p in tracker.rglob('*') if p.is_file() or p.is_symlink()]
    assert set(remaining)==keep,(remaining,keep)
    report=dict(completed_at=datetime.now(timezone.utc).isoformat(),published_archive_sha=SHA,
                moved_individual_files=moved,moved_original_directories=directories,
                preserved_unregistered_leftovers=leftovers,remaining_tracker_files=remaining,
                limit='No Docker resources, old research worktrees or external scratch roots were removed. Original absolute runtime paths are retired, not retargeted.')
    (EXT/'main-retirement.json').write_text(json.dumps(report,indent=2)+'\n')
    Path('/tmp/effort-main-retirement-20261118.json').write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps(dict(moved_files=len(moved),moved_directories=len(directories),remaining_tracker_files=len(remaining),extra_originals_preserved=len(leftovers)),indent=2))

if __name__=='__main__':main()
