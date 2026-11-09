"""Validate only this cycle's owned evidence and explicit document surfaces."""
import hashlib
import json
import pathlib
import re
import subprocess
import yaml

ROOT = pathlib.Path(__file__).resolve().parents[5]
EVIDENCE = pathlib.Path(__file__).resolve().parent
PREFIX = EVIDENCE.relative_to(ROOT).as_posix()
DOC_IDS = {
    'docs/issues/archive/2026-11-09-doc-create-leaves-partial-artifacts-on-finalization-failure.md': '44160e1378b2e3f5',
    'docs/issues/2026-11-09-call-graph-generic-call-returns-unrelated-callees.md': '84029483d2895d9b',
    'docs/trackers/ecc-integration/README.md': '9f27965f08caa909',
    'docs/trackers/ecc-integration/evidence.md': '99fb56409faba3a2',
    'docs/trackers/ecc-integration/issues.md': 'd28f5200f736af9c',
    'docs/trackers/ecc-integration/plans/README.md': '103e7a3b0cf03cf6',
    'docs/trackers/ecc-integration/reports/README.md': '4cf48cb6b2600db1',
    'docs/trackers/ecc-integration/notes/README.md': 'e0c447b3e19f2fba',
    'docs/trackers/ecc-integration/plans/2026-11-09-create-finalization-dogfood.md': '31b7eabea0a3688c',
    'docs/trackers/ecc-integration/reports/2026-11-09-create-finalization-dogfood.md': '2ba861100ce80249',
    'docs/trackers/ecc-integration/reports/2026-11-09-create-finalization-observed.md': 'ed2a2e472dba693d',
    'docs/trackers/ecc-integration/notes/2026-11-09-create-finalization-replay.md': '3b907e4aea4a63bc',
}
OTHER = ['docs/trackers/bug-fix-session-log.md', '.codescout/memories/research/ecc-codex-pilot-20261109.md']

def git(*args, check=True):
    return subprocess.run(['git', *args], cwd=ROOT, check=check, capture_output=True)

links = 0
for rel, artifact_id in DOC_IDS.items():
    text = (ROOT / rel).read_text()
    assert text.startswith('---\n')
    fm = text.split('---\n', 2)[1]
    assert yaml.safe_load(fm)['id'] == artifact_id, rel
    for target in re.findall(r'\[[^\]]*\]\(([^)\n]+)\)', text):
        if re.match(r'^[a-z]+:', target) or target.startswith('#'):
            continue
        path = target.split('#', 1)[0].strip('<>')
        assert (ROOT / rel).parent.joinpath(path).exists(), (rel, target)
        links += 1
    if '/archive/' in rel:
        assert 'status: fixed\n' in fm and 'claimed_by:' not in fm
        assert '6b2befb1eb9344e7a37f1fe660ec7de501276028' in text
        assert 'ba636938caee459b5457133127e8d919038a6758' in text
old_bug = 'docs/issues/2026-11-09-doc-create-leaves-partial-artifacts-on-finalization-failure.md'
assert not (ROOT / old_bug).exists()
log = (ROOT / OTHER[0]).read_text()
for number in range(222, 226):
    assert len(re.findall(rf'^## F-{number} — ', log, re.M)) == 1
    assert len(re.findall(rf'^\| F-{number} \|', log, re.M)) == 1
assert 'entry_high_water_F: 225\n' in log
before = json.loads((EVIDENCE / 'foreign-index-before.json').read_text())
assert {p: git('ls-files', '--stage', '--', p).stdout.decode().strip() for p in before} == before
private = '.codescout/ecc-create-finalization-private-20261109/'
assert not git('ls-files', '--', private).stdout
assert git('check-ignore', '--', private + 'pre-action-controller-input.jsonl', check=False).returncode == 0
raw = sorted(p for p in EVIDENCE.rglob('*') if p.is_file() and '__pycache__' not in p.parts and p.suffix != '.pyc')
patterns = [rb'\bsk-[A-Za-z0-9_-]{32,}\b', rb'\bgh[pousr]_[A-Za-z0-9]{30,}\b', rb'-----BEGIN (?:RSA |OPENSSH )?PRIVATE KEY-----', rb'## Three Codex Instances', rb'Private notes for this machine']
samples = [b'sk-' + b'a' * 40, b'ghp_' + b'b' * 40, b'-----BEGIN OPENSSH PRIVATE KEY-----', b'## Three Codex Instances', b'Private notes for this machine']
assert all(re.search(pattern, sample) for pattern, sample in zip(patterns, samples))
scanned = []
for p in raw + [ROOT / p for p in DOC_IDS] + [ROOT / p for p in OTHER]:
    assert not p.is_symlink()
    if p == pathlib.Path(__file__).resolve():
        continue  # Contains the deliberate known-positive calibration literals above.
    data = p.read_bytes()
    assert not any(re.search(pattern, data) for pattern in patterns), str(p.relative_to(ROOT))
    scanned.append(p)
result = {'managed_documents_checked': len(DOC_IDS), 'local_markdown_targets_checked': links,
          'ledger_entries_checked': ['F-222', 'F-223', 'F-224', 'F-225'],
          'foreign_index_blobs_unchanged': len(before), 'privacy_files_scanned': len(scanned),
          'privacy_pattern_positive_controls': len(patterns), 'privacy_hits': 0,
          'privacy_limits': 'Selected credential/private-policy patterns, not a proof of absence of every possible secret. Scanner excludes its own deliberate positive literals.',
          'private_history_ignored_and_untracked': True, 'old_untracked_bug_path_absent': True}
(EVIDENCE / 'validation-result.json').write_text(json.dumps(result, indent=2) + '\n')
future = {EVIDENCE / 'manifest.json', EVIDENCE / 'owned-paths.json'}
raw = {p for p in EVIDENCE.rglob('*') if p.is_file() and '__pycache__' not in p.parts and p.suffix != '.pyc'} | future
owned = sorted(set(DOC_IDS) | set(OTHER) | {p.relative_to(ROOT).as_posix() for p in raw})
if git('ls-files', '--error-unmatch', '--', old_bug, check=False).returncode == 0:
    owned.append(old_bug)
(EVIDENCE / 'owned-paths.json').write_text(json.dumps(owned, indent=2) + '\n')
manifest = [{'path': p.relative_to(EVIDENCE).as_posix(), 'bytes': p.stat().st_size, 'sha256': hashlib.sha256(p.read_bytes()).hexdigest()}
            for p in sorted(raw) if p.name != 'manifest.json']
(EVIDENCE / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
print(json.dumps(dict(result, manifest_files=len(manifest), owned_paths=len(owned)), indent=2))
