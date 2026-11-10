"""Freeze candidate, build/copy under one lease, then collect gate and native exits independently."""
import datetime,hashlib,json,os,subprocess,sys
from pathlib import Path
HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[4]
WT=ROOT/'.worktrees/ecc-bfs-identity-20261110'
PATHS=['src/lsp/mock.rs','src/tools/symbol/call_edges/cache.rs','src/tools/symbol/call_edges/resolver.rs','src/tools/symbol/call_graph/mod.rs','src/tools/symbol/call_graph/traversal.rs','src/tools/symbol/call_graph/identity.rs','src/tools/symbol/call_graph/rust_target.rs','src/tools/symbol/call_graph/identity_tests.rs']
def hashes():return {p:hashlib.sha256((WT/p).read_bytes()).hexdigest() for p in PATHS}
def git(*args):return subprocess.check_output(['git',*args],cwd=WT)
if '--leased-build' in sys.argv:
    frozen=json.loads((HERE/'candidate-source.json').read_text())
    assert hashes()==frozen['paths']
    with (HERE/'candidate-build.log').open('w') as log:
        built=subprocess.run(['cargo','build','--no-default-features','--bin','codescout'],cwd=WT,stdout=log,stderr=subprocess.STDOUT)
    assert built.returncode==0
    assert hashes()==frozen['paths']
    target=Path(os.environ['CARGO_TARGET_DIR'])/'debug/codescout'
    data=target.read_bytes()
    snapshot=ROOT/'.codescout/ecc-bfs-identity-private-20261110/candidate-codescout'
    snapshot.write_bytes(data)
    snapshot.chmod(0o700)
    record=dict(snapshot=str(snapshot),leased_target=str(target),sha256=hashlib.sha256(data).hexdigest(),bytes=len(data),source_unchanged=True,exit_code=0,snapshot_copied_while_lease_held=True)
    (HERE/'candidate-binary.json').write_text(json.dumps(record,indent=2)+'\n')
    print(json.dumps(record),flush=True)
    sys.exit(0)
assert not (HERE/'candidate-source.json').exists()
base=git('rev-parse','HEAD').decode().strip()
assert base=='7f0dbba5fd6ba25e4a50a8aad5c25403a11ae345'
tracked=git('diff','--name-only').decode().splitlines()
all_untracked=git('ls-files','--others','--exclude-standard').decode().splitlines()
runtime_paths={'.codescout/native-provenance-v1.counter','.codescout/native-provenance-v1.lock'}
untracked=[p for p in all_untracked if p not in runtime_paths]
assert set(tracked+untracked)==set(PATHS),(tracked,untracked)
(HERE/'excluded-runtime-paths.json').write_text(json.dumps([p for p in all_untracked if p in runtime_paths],indent=2)+'\n')
frozen=dict(base=base,paths=hashes(),base_manifest='baseline.json',scope='Frozen formatted eight-file initial candidate before independent review; no source commit yet.')
(HERE/'candidate-source.json').write_text(json.dumps(frozen,indent=2)+'\n')
patch=git('diff','--',*PATHS)
for p in untracked:
    result=subprocess.run(['git','diff','--no-index','--','/dev/null',p],cwd=WT,capture_output=True)
    assert result.returncode==1
    patch+=result.stdout
(HERE/'candidate.patch').write_bytes(patch)
env=dict(os.environ,TMPDIR='/home/marius/.cache/codescout-ecc-tmp',CODESCOUT_GATE_TMPFS_BASE='off')
env.pop('CARGO_TARGET_DIR',None)
build=subprocess.run(['./scripts/with-slot.sh','--slot','2',sys.executable,str(Path(__file__).resolve()),'--leased-build'],cwd=WT,env=env)
assert build.returncode==0
assert hashes()==frozen['paths']
native_dir=HERE/'candidate-native'
native_dir.mkdir()
text=(HERE/'baseline-native/probe.py').read_text()
text=text.replace("HERE.parent/'baseline-binary.json'","HERE.parent/'candidate-binary.json'")
text=text.replace("project('baseline-bfs-unique',True)","project('candidate-bfs-unique',True)")
text=text.replace("project('baseline-bfs-duplicate',False)","project('candidate-bfs-duplicate',False)")
text=text.replace("assert len(candidate_rows)==3 and all(r['clean_render'] and not r['matches_expected'] for r in candidate_rows)","assert len(candidate_rows)==3 and all(r['matches_expected'] for r in candidate_rows)")
script=native_dir/'probe.py'
script.write_text(text)
native_log=(native_dir/'native.log').open('w')
native=subprocess.Popen([sys.executable,str(script)],cwd=ROOT,stdout=native_log,stderr=subprocess.STDOUT)
started=datetime.datetime.now(datetime.timezone.utc).isoformat()
gate_rc=None
try:
    with (HERE/'candidate-gate.log').open('w') as log:
        gate=subprocess.run(['./scripts/with-slot.sh','--slot','2','./scripts/gate.sh','--check'],cwd=WT,env=env,stdout=log,stderr=subprocess.STDOUT)
        gate_rc=gate.returncode
finally:
    timed_out=False
    try:
        native_rc=native.wait(timeout=600)
    except subprocess.TimeoutExpired:
        timed_out=True
        native.kill()
        native_rc=native.wait()
    native_log.close()
    native_record=dict(exit_code=native_rc,driver_exit_observed=True,timed_out=timed_out,script_sha256=hashlib.sha256(script.read_bytes()).hexdigest(),scope='Native probe driver process exit observed independently of gate outcome; no repair replay.')
    (native_dir/'process-result.json').write_text(json.dumps(native_record,indent=2)+'\n')
    unchanged=hashes()==frozen['paths']
    gate_record=dict(exit_code=gate_rc,started=started,finished=datetime.datetime.now(datetime.timezone.utc).isoformat(),workspace=str(WT),base=base,paths=hashes(),source_unchanged=unchanged,command=['./scripts/with-slot.sh','--slot','2','./scripts/gate.sh','--check'])
    (HERE/'candidate-gate-result.json').write_text(json.dumps(gate_record,indent=2)+'\n')
print(json.dumps(dict(gate=gate_rc,native_driver=native_rc,source_unchanged=unchanged)),flush=True)
assert gate_rc==0 and native_rc==0 and unchanged
