"""Verify logical file bytes independently of tar regular/hard-link representation."""
from pathlib import Path
import gzip, hashlib, importlib.util, json, shutil, stat, tarfile

DEST = Path(__file__).resolve().parent
RAW = Path('/home/marius/.cache/codescout-resident/codescout/execution-profiles-20261109')
ORIGINAL = Path('/var/tmp/ep9-01a15/real-source-probe-4e959f3765/preserve-execution-evidence.py')

def sha(data):
    return hashlib.sha256(data).hexdigest()

def verify(archive, entries):
    seen = set()
    counts = {'regular_headers': 0, 'hardlink_headers': 0, 'symlink_headers': 0, 'directory_headers': 0}
    with tarfile.open(archive, 'r:gz') as bundle:
        for item in bundle:
            assert item.name in entries and item.name not in seen
            row = entries[item.name]
            assert item.mode == row['mode']
            if row['kind'] == 'file':
                assert item.isfile() or item.islnk()
                if item.islnk():
                    target = entries[item.linkname]
                    assert target['kind'] == 'file'
                    assert target['sha256'] == row['sha256'] and target['bytes'] == row['bytes']
                    counts['hardlink_headers'] += 1
                else:
                    counts['regular_headers'] += 1
                data = bundle.extractfile(item).read()
                assert len(data) == row['bytes'] and sha(data) == row['sha256']
            elif row['kind'] == 'symlink':
                assert item.issym() and item.linkname == row['target']
                counts['symlink_headers'] += 1
            else:
                assert item.isdir()
                counts['directory_headers'] += 1
            seen.add(item.name)
    assert seen == set(entries)
    return counts

def compact_member(name, row):
    if name.startswith(('source/', 'review/', 'tests/', 'jobs/')):
        return True
    if name.startswith('real-source/') and row['kind'] == 'file':
        relative = Path(name).relative_to('real-source')
        if len(relative.parts) == 1:
            return relative.suffix in {'.json', '.log', '.py'} or relative.name in {'stdout.jsonl', 'stderr'}
        return relative.name in {'receipt.json', 'container-before.json', 'container-after.json',
                                 'stdout', 'stderr', 'failure.json'}
    return False

def main():
    manifest_path = DEST / 'manifest.json'
    archive = RAW / 'execution-profiles.tar.gz'
    compact = DEST / 'source-and-review.tar.gz'
    if manifest_path.exists():
        result = json.loads(manifest_path.read_text())
        assert sha(archive.read_bytes()) == result['archive_sha256']
        verify(archive, result['members'])
        assert sha(compact.read_bytes()) == result['compact_sha256']
        verify(compact, {name: result['members'][name] for name in result['compact_members']})
        print(json.dumps({'reverified': True, 'logical_entries': result['logical_entries']}))
        return
    RAW.mkdir(parents=True, exist_ok=True)
    old = DEST / archive.name
    assert old.exists() and not archive.exists()
    shutil.move(old, archive)
    shutil.copyfile('/var/tmp/ep9-01a15/profile-preservation.log', DEST / 'failed-verifier.log')
    spec = importlib.util.spec_from_file_location('original_packager', ORIGINAL)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    paths = module.members()
    entries = {name: module.describe(path) for name, path in sorted(paths.items())}
    raw_counts = verify(archive, entries)
    selected = {name: row for name, row in entries.items() if compact_member(name, row)}
    assert not compact.exists()
    with compact.open('xb') as raw:
        with gzip.GzipFile(fileobj=raw, mode='wb', mtime=0) as zipped:
            with tarfile.open(fileobj=zipped, mode='w') as bundle:
                for name in selected:
                    bundle.add(paths[name], arcname=name, recursive=False)
    compact_counts = verify(compact, selected)
    for name, path in paths.items():
        assert module.describe(path) == entries[name], 'original changed: ' + name
    result = {
        'schema': 2, 'scope': 'uncommitted profile code, all retained real-source attempts, test logs and applied review; logical file bytes verified',
        'session_id': 'codex:01a15ca9-f939-7603-acb5-01c6b02f68e2',
        'source_branch': 'research/evidence-protocol-20261109',
        'source_base': 'fe8bb4b34f5e7912f9b16c4c0bf1ff73a3a10a23',
        'archive_path': str(archive), 'archive_bytes': archive.stat().st_size,
        'archive_sha256': sha(archive.read_bytes()), 'archive_readback_verified': True,
        'archive_header_counts': raw_counts,
        'logical_entries': len(entries),
        'logical_regular_files': sum(row['kind'] == 'file' for row in entries.values()),
        'source_files': {name: row for name, row in entries.items() if name.startswith('source/') and row['kind'] == 'file'},
        'compact_archive': compact.name, 'compact_bytes': compact.stat().st_size,
        'compact_sha256': sha(compact.read_bytes()), 'compact_header_counts': compact_counts,
        'compact_members': list(selected), 'members': entries,
        'companions': {name: sha((DEST / name).read_bytes()) for name in ['verify_archive.py', 'failed-verifier.log']},
        'limits': [
            'Large source/build closure stays outside the repository at archive_path; compact archive retains code/review/logs/receipts, not all replay inputs.',
            'No dependency image export; exact existing image ID remains required.',
            'Receipts contain original absolute host mount paths; preservation does not assert replay at a different path without reconstruction.',
            'Excludes __pycache__ and review .git directories.',
            'Initial verifier failed on valid tar hard-links; the completed archive bytes were relocated unchanged and fully reverified.',
            'No paid cohort admission, generic efficacy, peer deletion or unidentified orphan volume cleanup.'
        ]}
    manifest_path.write_text(json.dumps(result, indent=2) + '\n')
    assert json.loads(manifest_path.read_text()) == result
    print(json.dumps({k: v for k, v in result.items() if k not in {'members', 'source_files', 'compact_members'}}, indent=2))

if __name__ == '__main__':
    main()
