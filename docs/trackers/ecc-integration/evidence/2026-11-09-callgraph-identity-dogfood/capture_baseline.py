"""Freeze public repair inputs and private pre-action controller history."""
import hashlib
import json
from pathlib import Path
import subprocess

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[4]
WORKTREE = ROOT / '.worktrees/ecc-callgraph-identity'
BASE = '69ffdd12e6d51f4884c0190a1a6c4edeca4289e4'
def git(*args, cwd=ROOT, data=None):
    return subprocess.check_output(['git', *args], cwd=cwd, input=data)
assert git('rev-parse', 'HEAD', cwd=WORKTREE).decode().strip() == BASE
assert not git('diff', '--name-only', cwd=WORKTREE)
assert not (HERE / 'baseline.json').exists()
inputs = ['CLAUDE.md', 'AGENTS.md', 'Cargo.toml', 'Cargo.lock',
          'src/tools/symbol/call_graph/mod.rs', 'src/tools/symbol/call_graph/traversal.rs',
          'src/tools/symbol/call_edges/cache.rs', 'src/tools/symbol/call_edges/resolver.rs',
          'src/lsp/ops.rs', 'src/lsp/mock.rs', 'src/symbol/query.rs']
records = []
destination = HERE / 'baseline-inputs'
destination.mkdir(exist_ok=True)
for path in inputs:
    data = git('show', BASE + ':' + path)
    assert (WORKTREE / path).read_bytes() == data
    name = path.replace('/', '__') + '.txt'
    (destination / name).write_bytes(data)
    records.append({'path': path, 'snapshot': 'baseline-inputs/' + name,
                    'git_blob': git('rev-parse', BASE + ':' + path).decode().strip(),
                    'sha256': hashlib.sha256(data).hexdigest()})
(destination / 'candidate-brief.txt').write_text('Investigate the reported path-pinned call_graph lookup for duplicated Rust function names. Reproduce with a fresh current-source executable and isolated state, identify the responsible boundary, and correct only demonstrated contract violations. Preserve valid empty results, direction/depth semantics and error reporting. Keep shared servers/configuration untouched. Record executed verification and exact replay provenance; no no-ECC repair replay now.\n')
skills = []
controller = HERE / 'controller-only'
controller.mkdir(exist_ok=True)
for name in ['ecc-spec-miner', 'ecc-silent-failure-review']:
    source = Path('/home/marius/work/claude/claude-plugins.worktrees/ecc-codex-codescout/ecc-codescout/skills') / name / 'SKILL.md'
    installed = Path('/home/marius/.codex/plugins/cache/ecc-codescout-pilot/ecc-codescout/0.1.0/skills') / name / 'SKILL.md'
    data = source.read_bytes()
    assert data == installed.read_bytes()
    (controller / (name + '.txt')).write_bytes(data)
    skills.append({'name': name, 'source': str(source), 'installed': str(installed), 'installed_bytes_match': True, 'sha256': hashlib.sha256(data).hexdigest(), 'use': 'explicit body use; no automatic residency claim'})
ref = 'refs/ecc-replay/callgraph-identity-20261109'
subprocess.run(['git', 'update-ref', ref, BASE, '0' * 40], cwd=ROOT, check=True)
staged = git('diff', '--cached', '--name-only').decode().splitlines()
foreign = {p: git('ls-files', '--stage', '--', p).decode().strip() for p in staged}
(HERE / 'foreign-index-before.json').write_text(json.dumps(foreign, indent=2) + '\n')
private = ROOT / '.codescout/ecc-callgraph-identity-private-20261109'
private.mkdir(mode=0o700)
(private / '.gitignore').write_text('*\n')
source = Path('/home/marius/.codex/sessions/2026/11/09/rollout-2026-11-09T10-24-45-01a1bf68-e650-7f20-911e-b53f86597c27.jsonl')
lines = source.read_bytes().splitlines(keepends=True)
rows = [json.loads(line) for line in lines]
matches = [i for i, row in enumerate(rows) if row.get('type') == 'response_item' and row.get('payload', {}).get('role') == 'user' and '\n'.join(block.get('text', '') for block in row['payload'].get('content', []) if block.get('type') == 'input_text').strip() == 'ok, go']
assert matches
start = matches[-1]
end = next(i for i in range(start + 1, len(rows)) if rows[i].get('type') == 'response_item' and rows[i].get('payload', {}).get('role') == 'assistant')
data = b''.join(lines[:end])
snapshot = private / 'pre-action-controller-input.jsonl'
snapshot.write_bytes(data)
snapshot.chmod(0o600)
context = [row['payload'] for row in rows[:end] if row.get('type') == 'turn_context'][-1]
provenance = {'source': str(source), 'private_snapshot': str(snapshot), 'bytes': len(data), 'sha256': hashlib.sha256(data).hexdigest(), 'mode': '0600', 'cutoff': 'before first assistant response to latest exact ok, go user event', 'user_event_timestamp': rows[start].get('timestamp'), 'configured_model': context.get('model'), 'configured_effort': context.get('effort'), 'limits': 'Host transcript history, not a byte-exact provider request or claim that all history was resident. Contains earlier ECC conditioning and solution history; retain privately, exclude solutions/evaluators/skill bodies from future repair candidate input. Future skill-free protocol remains to be specified; no replay ran.'}
(HERE / 'session-input-provenance.json').write_text(json.dumps(provenance, indent=2) + '\n')
result = {'base_commit': BASE, 'base_tree': git('rev-parse', BASE + '^{tree}').decode().strip(), 'replay_ref': ref, 'workspace': str(WORKTREE), 'branch': git('branch', '--show-current', cwd=WORKTREE).decode().strip(), 'source_clean_before_edits': True, 'inputs': records, 'skills': skills, 'shared_runtime': {'build_id': '9e4db9b94323fb5bc65e3af403904f883459979206077488e448a362291620dc', 'pid': 1520813, 'exe_deleted': True, 'current_source_oracle': False}, 'scope': 'Fresh current-source private-server dogfood. Not causal ECC A/B or no-ECC replay.'}
(HERE / 'baseline.json').write_text(json.dumps(result, indent=2) + '\n')
print(json.dumps({'base': BASE, 'inputs': len(records), 'skills': len(skills), 'private_history_bytes': len(data), 'replay_ran': False}))
