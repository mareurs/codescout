"""Record terminal outcomes and source hashes; RED and GREEN remain separate."""
import datetime
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[4]
WORKTREE = ROOT / '.worktrees/ecc-callgraph-identity'
label = sys.argv[1]
test_filter = sys.argv[2] if len(sys.argv) > 2 else 'tools::symbol::call_'
paths = ['src/tools/symbol/call_graph/mod.rs', 'src/tools/symbol/call_edges/cache.rs']
def hashes():
    return {p: hashlib.sha256((WORKTREE / p).read_bytes()).hexdigest() for p in paths}
before = hashes()
started = datetime.datetime.now(datetime.timezone.utc).isoformat()
env = dict(os.environ, TMPDIR='/home/marius/.cache/codescout-ecc-tmp', CODESCOUT_GATE_TMPFS_BASE='off')
env.pop('CARGO_TARGET_DIR', None)
command = ['./scripts/with-slot.sh', '--slot', '0', 'cargo', 'test', '--lib', test_filter, '--', '--test-threads=4']
with (HERE / (label + '.log')).open('w') as log:
    process = subprocess.run(command, cwd=WORKTREE, env=env, stdout=log, stderr=subprocess.STDOUT)
text = (HERE / (label + '.log')).read_text()
record = {'label': label, 'command': command, 'workspace': str(WORKTREE), 'started': started, 'finished': datetime.datetime.now(datetime.timezone.utc).isoformat(), 'exit_code': process.returncode, 'source_before': before, 'source_after': hashes(), 'summaries': [line for line in text.splitlines() if line.startswith('test result:') or line.startswith('error[')]}
assert record['source_before'] == record['source_after']
(HERE / (label + '-result.json')).write_text(json.dumps(record, indent=2) + '\n')
print(json.dumps(record), flush=True)
