"""Preserve the closed Obscura public process without reading private outcomes."""
import datetime
import hashlib
import json
from pathlib import Path
import shutil

ROOT = Path('/var/tmp/ep11-01a15/paid-campaign-headroom-v3')
DEST = Path('/home/marius/work/claude/codescout/docs/trackers/effort-steering/evidence/public-process-audit-20261111/obscura-process')


def main():
    assert (ROOT / 'actor-batch-closed.json').is_file()
    bindings, traces = {}, []

    def read(path):
        data = path.read_bytes()
        bindings[str(path)] = hashlib.sha256(data).hexdigest()
        return json.loads(data)

    for index, arm, phases in ((4, 'X', ('repair',)),
                                (5, 'P', ('investigate', 'repair', 'challenge'))):
        for phase in phases:
            native = ROOT / 'attempts' / f'{index:02}-{arm}' / phase / 'native'
            calls, results = {}, {}
            for request_path in sorted(native.glob('request-*.json'),
                                       key=lambda p: int(p.stem.split('-')[-1])):
                for message in read(request_path)['messages']:
                    content = message.get('content', [])
                    if not isinstance(content, list):
                        continue
                    for block in content:
                        if not isinstance(block, dict):
                            continue
                        if block.get('type') == 'tool_use':
                            key = block['id']
                            if key in calls:
                                assert calls[key] == block
                            calls[key] = block
                        if block.get('type') == 'tool_result':
                            key = block['tool_use_id']
                            if key in results:
                                assert results[key] == block
                            results[key] = block
            assert set(calls) == set(results)
            traces.append({'arm': arm, 'phase': phase,
                'unique_tool_use_ids_in_request_histories': len(calls),
                'matching_result_ids': len(results),
                'calls': [{'call': call, 'result': results[key]} for key, call in calls.items()]})

    selected = []
    for index, arm, phase in ((4, 'X', 'repair'), (5, 'P', 'challenge'),
                              (6, 'R', 'challenge'), (7, 'L', 'repair')):
        selected.append(ROOT / 'attempts' / f'{index:02}-{arm}' / phase / 'native/candidate.patch')
    for phase in ('investigate', 'repair', 'challenge'):
        base = ROOT / 'attempts/05-P' / phase
        selected += [base / 'native/native-executor.json', base / 'protocol/outcome.json']
        for path in sorted((base / 'protocol/commands').glob('*/*')):
            if path.name in ('receipt.json', 'stdout', 'stderr') and path.is_file():
                selected.append(path)
    selected += [ROOT / 'attempts/04-X/repair/native/turn-results.json',
                 ROOT / 'attempts/04-X/repair/native/background-evidence.json']
    DEST.mkdir(exist_ok=False)
    files = []
    for path in selected:
        data = path.read_bytes()
        digest = hashlib.sha256(data).hexdigest()
        relative = path.relative_to(ROOT)
        target = DEST / 'public-files' / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
        assert hashlib.sha256(target.read_bytes()).hexdigest() == digest
        bindings[str(path)] = digest
        files.append({'original': str(path), 'copy': str(target), 'sha256': digest, 'bytes': len(data)})
    report = {'schema': 'obscura-public-process-audit-v1',
        'captured_at': datetime.datetime.now(datetime.timezone.utc).isoformat(),
        'scope': 'All deduplicated tool-use/result blocks in captured P/X request histories, selected public artifacts and complete four-arm candidate patches. No private verdict contents read.',
        'traces': traces, 'files': files, 'bindings': bindings,
        'private_verdict_files_at_capture': len(list(ROOT.glob('grades/*/verdict.json'))),
        'obscura_verdict_files_at_capture': sum((ROOT / f'grades/{i:02}/verdict.json').exists() for i in range(4, 8)),
        'limits': ['Recorded tool results may contain truncated tool output; retained as delivered.',
            'Tool-use/result identity census does not prove semantic adequacy or private acceptance.',
            'An absent standalone JS executable is not absence of an embedded project runtime.',
            'No frozen candidate, score rule or prompt changed.']}
    assert all(hashlib.sha256(Path(path).read_bytes()).hexdigest() == digest
               for path, digest in bindings.items())
    shutil.copy2(__file__, DEST / 'producer.py')
    (DEST / 'audit.json').write_text(json.dumps(report, indent=2, sort_keys=True) + '\n')
    summary = []
    for trace in traces:
        rows = []
        for row in trace['calls']:
            call = row['call']
            command = call.get('input', {}).get('command', '')
            if not command:
                continue
            parsed = []
            content = row['result'].get('content', [])
            for block in content if isinstance(content, list) else []:
                if block.get('type') != 'text':
                    continue
                try:
                    value = json.loads(block['text'])
                except ValueError:
                    continue
                text = value.get('stdout', '') + '\n' + value.get('stderr', '')
                parsed.append({'exit_code': value.get('exit_code'),
                    'producer_exit_code': value.get('producer_exit_code'),
                    'summaries': [line for line in text.splitlines() if 'Summary ' in line or 'test result:' in line]})
            rows.append({'id': call['id'], 'command': command, 'result_summary': parsed})
        summary.append({'arm': trace['arm'], 'phase': trace['phase'], 'commands': rows})
    print(json.dumps({'obscura_verdict_files_at_capture': report['obscura_verdict_files_at_capture'],
        'traces': summary}, indent=2))


if __name__ == '__main__':
    main()
