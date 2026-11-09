"""Bind the reviewed, gated tree to the local integration without touching peer paths."""
import hashlib
import json
import pathlib
import subprocess

ROOT = pathlib.Path(__file__).resolve().parents[5]
EVIDENCE = pathlib.Path(__file__).resolve().parent
CANDIDATE = ROOT / '.worktrees/ecc-navigation-gate'

def git(*args, cwd=ROOT, data=None):
    return subprocess.check_output(['git', *args], cwd=cwd, input=data)

def rev(value, cwd=ROOT):
    return git('rev-parse', value, cwd=cwd).decode().strip()

def identity(cwd):
    commit = rev('HEAD', cwd)
    patch_id = subprocess.check_output(['git', 'patch-id', '--stable'], input=git('show', commit, cwd=cwd)).decode().split()[0]
    paths = git('show', '--format=', '--name-only', commit, cwd=cwd).decode().splitlines()
    assert paths == ['src/librarian/tools/create.rs'], paths
    return {'commit': commit, 'tree': rev('HEAD^{tree}', cwd), 'parent': rev('HEAD^', cwd), 'stable_patch_id': patch_id, 'paths': paths}

source = identity(CANDIDATE)
integration = identity(ROOT)
assert source['stable_patch_id'] == integration['stable_patch_id']
assert source['parent'] == integration['parent'] == '038fffb3447b55900e617a251b6472486259e07c'
assert source['tree'] == integration['tree']
expected = json.loads((EVIDENCE / 'final-source.json').read_text())['sha256']
for cwd in [ROOT, CANDIDATE]:
    committed = git('show', 'HEAD:src/librarian/tools/create.rs', cwd=cwd)
    assert hashlib.sha256(committed).hexdigest() == expected
    assert (cwd / 'src/librarian/tools/create.rs').read_bytes() == committed
before = json.loads((EVIDENCE / 'foreign-index-before.json').read_text())
after = {path: git('ls-files', '--stage', '--', path).decode().strip() for path in before}
assert after == before
assert set(git('diff', '--cached', '--name-only').decode().splitlines()) == set(before)
gate = json.loads((EVIDENCE / 'final-gate-result.json').read_text())
module = json.loads((EVIDENCE / 'final-module-result.json').read_text())
mutant = json.loads((EVIDENCE / 'final-M7-result.json').read_text())
mcp = json.loads((EVIDENCE / 'final-mcp-results.json').read_text())
assert gate['exit_code'] == module['exit_code'] == 0
assert gate['source_before'] == gate['source_after'] == expected
assert mutant['exit_code'] != 0 and any('KILLED' in v for v in mutant['verdicts'])
assert len(mcp['cases']) == 5
for case in mcp['cases']:
    assert case['before'] == case['after_failure']
    assert case['file_absent_after_failure'] and case['successful_retry_read']
result = {'source': source, 'integration': integration, 'source_sha256': expected,
          'identical_committed_trees': True, 'gate_binding': 'Exact source and full committed tree equal the gated isolated candidate; no integration-source delta.',
          'foreign_staged_paths_unchanged': sorted(before), 'private_mcp_cases': len(mcp['cases']),
          'replay_base_ref': 'refs/ecc-replay/create-finalization-20261109',
          'replay_base': rev('refs/ecc-replay/create-finalization-20261109'),
          'limits': 'No no-ECC replay, push, release or shared-MCP rollout. Uncommitted peer documents are outside the committed source tree.'}
(EVIDENCE / 'integration.json').write_text(json.dumps(result, indent=2) + '\n')
for number in [0x7c, 0x7d, 0x7f, 0x81, 0x82, 0x83, 0x84]:
    job = pathlib.Path(f'/tmp/codescout-bg/1520813-b902d740-bg_{number:08x}.json')
    if job.exists():
        target = EVIDENCE / 'jobs' / job.name
        target.parent.mkdir(exist_ok=True)
        target.write_bytes(job.read_bytes())
print(json.dumps(result, indent=2))
