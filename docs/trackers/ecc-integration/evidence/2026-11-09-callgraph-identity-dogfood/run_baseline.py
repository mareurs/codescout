"""Build and run the existing relevant module from one source-bound lease."""
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
env = dict(os.environ, TMPDIR='/home/marius/.cache/codescout-ecc-tmp', CODESCOUT_GATE_TMPFS_BASE='off')
env.pop('CARGO_TARGET_DIR', None)
commands = [('baseline-build', ['cargo', 'build', '--bin', 'codescout']), ('baseline-module', ['cargo', 'test', '--lib', 'tools::symbol::call_', '--', '--test-threads=4'])]
for label, command in commands:
    started = datetime.datetime.now(datetime.timezone.utc).isoformat()
    with (HERE / (label + '.log')).open('w') as log:
        result = subprocess.run(['./scripts/with-slot.sh', '--slot', '0', *command], cwd=WORKTREE, env=env, stdout=log, stderr=subprocess.STDOUT)
    record = {'label': label, 'command': command, 'workspace': str(WORKTREE), 'head': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=WORKTREE, text=True).strip(), 'started': started, 'finished': datetime.datetime.now(datetime.timezone.utc).isoformat(), 'exit_code': result.returncode}
    (HERE / (label + '-result.json')).write_text(json.dumps(record, indent=2) + '\n')
    print(json.dumps(record), flush=True)
    assert result.returncode == 0
    if label == 'baseline-build':
        binary = Path('/home/marius/.cache/codescout-gate/slot-0/debug/codescout')
        snapshot = ROOT / '.codescout/ecc-callgraph-identity-private-20261109/baseline-codescout'
        shutil.copy2(binary, snapshot)
        digest = hashlib.sha256(binary.read_bytes()).hexdigest()
        assert hashlib.sha256(snapshot.read_bytes()).hexdigest() == digest
        (HERE / 'baseline-binary.json').write_text(json.dumps({'snapshot': str(snapshot), 'sha256': digest, 'head': record['head'], 'source': str(binary)}, indent=2) + '\n')
