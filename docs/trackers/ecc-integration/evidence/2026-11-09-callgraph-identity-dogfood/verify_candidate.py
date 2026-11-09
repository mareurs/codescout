"""Freeze candidate bytes, then build/native-probe/full-gate without source mutations."""
import datetime
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[4]
WORKTREE = ROOT / '.worktrees/ecc-callgraph-identity'
frozen = json.loads((HERE / 'candidate-source.json').read_text())['files']
def check():
    assert all(hashlib.sha256((WORKTREE / p).read_bytes()).hexdigest() == h for p, h in frozen.items())
check()
env = dict(os.environ, TMPDIR='/home/marius/.cache/codescout-ecc-tmp', CODESCOUT_GATE_TMPFS_BASE='off')
env.pop('CARGO_TARGET_DIR', None)
probe = None
for label, command in [('candidate-build', ['cargo', 'build', '--bin', 'codescout']), ('candidate-gate', ['./scripts/gate.sh', '--check'])]:
    check()
    started = datetime.datetime.now(datetime.timezone.utc).isoformat()
    with (HERE / (label + '.log')).open('w') as log:
        result = subprocess.run(['./scripts/with-slot.sh', '--slot', '0', *command], cwd=WORKTREE, env=env, stdout=log, stderr=subprocess.STDOUT)
    check()
    record = {'label': label, 'command': command, 'workspace': str(WORKTREE), 'head': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=WORKTREE, text=True).strip(), 'source_hashes': frozen, 'source_unchanged': True, 'started': started, 'finished': datetime.datetime.now(datetime.timezone.utc).isoformat(), 'exit_code': result.returncode}
    (HERE / (label + '-result.json')).write_text(json.dumps(record, indent=2) + '\n')
    print(json.dumps(record), flush=True)
    assert result.returncode == 0
    if label == 'candidate-build':
        source = Path('/home/marius/.cache/codescout-gate/slot-0/debug/codescout')
        snapshot = ROOT / '.codescout/ecc-callgraph-identity-private-20261109/candidate-codescout'
        shutil.copy2(source, snapshot)
        digest = hashlib.sha256(snapshot.read_bytes()).hexdigest()
        assert digest == hashlib.sha256(source.read_bytes()).hexdigest()
        (HERE / 'candidate-binary.json').write_text(json.dumps({'snapshot': str(snapshot), 'source': str(source), 'sha256': digest, 'source_hashes': frozen, 'head': record['head']}, indent=2) + '\n')
        probe_log = (HERE / 'candidate-native.log').open('w')
        probe = subprocess.Popen(['python3', str(HERE / 'probe_fixed.py'), 'candidate'], cwd=ROOT, env=env, stdout=probe_log, stderr=subprocess.STDOUT)
assert probe is not None
probe_rc = probe.wait()
probe_log.close()
(HERE / 'candidate-native-result.json').write_text(json.dumps({'exit_code': probe_rc}, indent=2) + '\n')
assert probe_rc == 0
print('candidate build/native/full gate passed', flush=True)
