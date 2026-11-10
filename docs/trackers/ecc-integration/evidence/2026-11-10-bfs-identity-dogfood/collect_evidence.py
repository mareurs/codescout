"""Collect terminal host job metadata and select replay evidence, excluding runtime state."""
import hashlib,json,shutil
from pathlib import Path
HERE=Path(__file__).resolve().parent
jobs=HERE/'jobs';jobs.mkdir(exist_ok=True)
handles=['c2','c4','c5','c6','c7','c8','c9','ca','cb','cc','cd','ce','cf','d0','d7','d8','d9','da','db','dc','dd','de','e1','e3','e4','e5','e8','e9','eb','ec','ef','f0']
found=[]
for suffix in handles:
    name='1520813-b902d740-bg_'+suffix.zfill(8)+'.json'
    source=Path('/tmp/codescout-bg')/name
    if source.exists():
        data=source.read_bytes();record=json.loads(data)
        (jobs/name).write_bytes(data)
        found.append(dict(handle='@bg_'+suffix.zfill(8),state=record.get('state'),exit_code=record.get('exit_code')))
def selected(path):
    p=path.relative_to(HERE)
    if not path.is_file():return False
    if p.parts[:2]==('review','native'):
        return path.name=='rustc.stderr'
    return not any(x in {'.git','__pycache__','state','target'} for x in p.parts) and path.suffix not in {'.pyc','.db','.sqlite','.rmeta','.lock'}
paths=[str(p.relative_to(HERE)) for p in sorted(HERE.rglob('*')) if selected(p)]
excluded=[dict(path=str(p.relative_to(HERE)),bytes=p.stat().st_size,reason='local runtime project/state/compiler output retained, not evidence commit input') for p in sorted((HERE/'review/native').rglob('*')) if p.is_file() and not selected(p)]
(HERE/'excluded-local-runtime-artifacts.json').write_text(json.dumps(excluded,indent=2)+'\n')
(HERE/'selected-raw-paths.json').write_text(json.dumps(sorted(set(paths+['excluded-local-runtime-artifacts.json','selected-raw-paths.json'])),indent=2)+'\n')
print(json.dumps(dict(jobs=found,selected_raw_paths=len(paths),excluded_runtime_files=len(excluded)),indent=2))
