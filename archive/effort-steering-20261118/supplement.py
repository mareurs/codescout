#!/usr/bin/env python3
from pathlib import Path
import hashlib,json,os,sqlite3,stat,subprocess
from datetime import datetime,timezone
MAIN=Path('/home/marius/work/claude/codescout')
WT=Path('/home/marius/work/claude/codescout.worktrees/effort-steering-archive-20261118')
EXT=Path('/home/marius/archives/codescout/effort-steering-20261118')
META=WT/'archive/effort-steering-20261118'

def digest(p):
    with p.open('rb') as f:return hashlib.file_digest(f,'sha256').hexdigest()

def main():
    paths=[p for p in (MAIN/'docs/research/data').iterdir() if p.is_file()]
    paths += [MAIN/p for p in [
      'docs/superpowers/plans/2026-11-05-agent-background-continuation.md',
      'docs/superpowers/plans/2026-11-05-verified-worker-composition.md',
      'docs/superpowers/specs/2026-11-06-semantic-contract-pilot-design.md',
      'docs/superpowers/specs/2026-11-06-semantic-contract-adapter-design.md',
      'docs/superpowers/specs/2026-11-07-baseline-first-oracle-design.md',
      'docs/superpowers/specs/2026-11-08-oracle-uncertainty-resolution-design.md',
      'docs/research/2026-11-06-own-cache-cleanup-followup.md']]
    rows=[]
    for p in sorted(set(paths)):
        rel=p.relative_to(MAIN);h=digest(p);external=p.stat().st_size>32*1024*1024
        dest=EXT/'capsules'/rel if external else WT/rel
        dest.parent.mkdir(parents=True,exist_ok=True)
        if dest.exists():assert digest(dest)==h,str(dest)
        else:subprocess.run(['cp','--reflink=auto','--preserve=mode,timestamps','--',str(p),str(dest)],check=True)
        assert digest(p)==h==digest(dest),str(p)
        r=dict(path=str(rel),source_root=str(MAIN),bytes=p.stat().st_size,mode=stat.S_IMODE(p.stat().st_mode),sha256=h,storage='external' if external else 'git',destination=str(dest if external else rel))
        rows.append(r)
        if external:(WT/(str(rel)+'.external.json')).write_text(json.dumps(r,indent=2)+'\n')
    c=sqlite3.connect('file:/home/marius/.local/share/librarian/catalog.db?mode=ro',uri=True);c.row_factory=sqlite3.Row;c.execute('BEGIN')
    names={str(p) for p in paths}
    artifacts=[dict(r) for r in c.execute('SELECT * FROM artifact') if r['abs_path'] in names]
    ids={a['id'] for a in artifacts}
    catalog={'artifact':artifacts}
    for table,match in [('events',lambda r:r['artifact_id'] in ids),('artifact_augmentation',lambda r:r['artifact_id'] in ids),('artifact_observation',lambda r:r['artifact_id'] in ids),('artifact_link',lambda r:r['src_id'] in ids or r['dst_id'] in ids)]:
        catalog[table]=[dict(r) for r in c.execute('SELECT * FROM "'+table+'"') if match(r)]
    c.rollback();c.close()
    (META/'supplemental-catalog-snapshot.json').write_text(json.dumps(catalog,indent=2,ensure_ascii=False)+'\n')
    manifest=dict(captured_at=datetime.now(timezone.utc).isoformat(),reason='Final sweep: original historical data outside the consolidated tracker and remaining full experiment plans/specs',files=rows)
    (META/'supplemental-custody-manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
    (EXT/'supplemental-custody-manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
    print(json.dumps(dict(files=len(rows),bytes=sum(r['bytes'] for r in rows),external_files=[r['path'] for r in rows if r['storage']=='external'],catalog_artifacts=len(artifacts)),indent=2))

if __name__=='__main__':main()
