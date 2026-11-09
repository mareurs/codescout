"""Reapply the two observed review survivors before accepting strengthened guards."""
import datetime
import hashlib
import json
import os
from pathlib import Path
import subprocess

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[4]
WORKTREE = ROOT / '.worktrees/ecc-callgraph-identity'
review = ROOT / '.worktrees/ecc-callgraph-identity-review'
original = json.loads((HERE / 'candidate-source.json').read_text())['files']
for p, digest in original.items():
    assert hashlib.sha256((review / p).read_bytes()).hexdigest() == digest
    assert (review / p).read_bytes().split(b'#[cfg(test)]', 1)[0] == (WORKTREE / p).read_bytes().split(b'#[cfg(test)]', 1)[0]
env = dict(os.environ, TMPDIR='/home/marius/.cache/codescout-ecc-tmp', CODESCOUT_GATE_TMPFS_BASE='off')
env.pop('CARGO_TARGET_DIR', None)
def hashes():
    return {p: hashlib.sha256((WORKTREE / p).read_bytes()).hexdigest() for p in original}
descriptors = json.loads((HERE / 'review/descriptors.json').read_text())
results = []
for name in ['M02-line', 'M03-column']:
    descriptor = next(d for d in descriptors if d['id'] == name)
    before = hashes()
    command = ['./scripts/mutation-probe.sh', '--strict', '--file', descriptor['file'], '--find', descriptor['find'], '--replace', descriptor['replace'], '--', './scripts/with-slot.sh', '--slot', '0', 'cargo', 'test', '--lib', 'tools::symbol::call_', '--', '--test-threads=4']
    started = datetime.datetime.now(datetime.timezone.utc).isoformat()
    with (HERE / ('final-' + name + '.log')).open('w') as log:
        result = subprocess.run(command, cwd=WORKTREE, env=env, stdout=log, stderr=subprocess.STDOUT)
    text = (HERE / ('final-' + name + '.log')).read_text()
    record = {'mutation': name, 'descriptor': descriptor, 'command': command, 'started': started, 'finished': datetime.datetime.now(datetime.timezone.utc).isoformat(), 'exit_code': result.returncode, 'source_before': before, 'source_after': hashes(), 'summaries': [line for line in text.splitlines() if line.startswith('test result:') or 'mutation-probe: KILLED' in line or 'mutation-probe: SURVIVED' in line]}
    (HERE / ('final-' + name + '-result.json')).write_text(json.dumps(record, indent=2) + '\n')
    print(json.dumps(record), flush=True)
    assert record['source_before'] == record['source_after']
    assert result.returncode != 0 and 'mutation-probe: KILLED' in text
    assert 'completed_resolutions_discriminate_definition_coordinates' in text
    assert 'exact definition' in text
    results.append(record)
subprocess.run(['python3', str(HERE / 'run_module.py'), 'final-module'], cwd=ROOT, check=True, env=env)
module = json.loads((HERE / 'final-module-result.json').read_text())
assert module['exit_code'] == 0
patch = subprocess.check_output(['git', 'diff', '--', *original], cwd=WORKTREE)
(HERE / 'final-candidate.patch').write_bytes(patch)
(HERE / 'final-source.json').write_text(json.dumps({'files': hashes(), 'production_prefix_unchanged': True, 'initial_candidate': 'candidate-source.json', 'observed_initial_survivors': ['review/M02-line/restoration.json', 'review/M03-column/restoration.json'], 'exact_rechecks': ['final-M02-line-result.json', 'final-M03-column-result.json'], 'module': 'final-module-result.json'}, indent=2) + '\n')
print('exact survivors killed and restored module passed', flush=True)
