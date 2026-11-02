"""Task adapters and cross-pilot accounting for the fixed-effort extension."""
import argparse
import hashlib
import json
from pathlib import Path, PurePosixPath
import re
import uuid

import pilot
from evidence import nonnegative


def previous_costs(directories):
    costs, seen_paths, seen_runs = [], set(), set()
    for directory in directories:
        root = Path(directory).resolve()
        if root in seen_paths:
            raise ValueError('duplicate history source')
        seen_paths.add(root)
        def read(name):
            return [json.loads(line) for line in (root / name).read_text().splitlines() if line.strip()]
        journal = read('pilot-journal.jsonl')
        spend = read('pilot-spend.jsonl')
        rows = read('pilot-rows.jsonl')
        pending, graded = {}, {}
        for event in journal:
            target = pending if event['status'] == 'pending' else graded if event['status'] == 'graded' else None
            if target is None or event['run'] in target:
                raise ValueError('invalid or duplicate journal event')
            target[event['run']] = event
        paid = {r['run']: r for r in spend}
        results = {r['runId']: r for r in rows}
        if not pending or len(paid) != len(spend) or len(results) != len(rows):
            raise ValueError('empty or duplicated accounting')
        if not (pending.keys() == graded.keys() == paid.keys() == results.keys()):
            raise ValueError('unresolved attempt or missing accounting')
        for run, payment in paid.items():
            if run in seen_runs:
                raise ValueError('duplicate run across history')
            seen_runs.add(run)
            cost = nonnegative(payment.get('costUsd'), 'costUsd')
            if cost != nonnegative(results[run].get('costUsd'), 'costUsd') or cost != nonnegative(graded[run].get('costUsd'), 'costUsd'):
                raise ValueError('inconsistent recorded costs')
            if payment['session'] != pending[run]['session'] or payment['session'] != results[run]['sessionId']:
                raise ValueError('inconsistent session')
            row = results[run]
            pilot.require_run_valid({'is_error': row['isError']}, row['cliExit'], row['usage'], row['effort'])
            costs.append(cost)
    return costs


def grade_output(log, exit_code, expected):
    matches = re.findall(r'^test (\S+)(?: - should panic)? \.\.\. (ok|FAILED|ignored)\s*$', log, re.M)
    outcomes = dict(matches)
    if len(outcomes) != len(matches):
        raise ValueError('duplicate test identity')
    missing = sorted(set(expected) - outcomes.keys())
    failed = sorted(name for name, status in outcomes.items() if status != 'ok')
    compile_failure = bool(re.search(r'error(?:\[E\d+\])?:', log)) and 'could not compile' in log
    if exit_code and not failed and not compile_failure:
        raise ValueError('unexplained grader exit')
    if missing and not (exit_code and compile_failure):
        raise ValueError(f'missing required tests: {missing}')
    return {'pass': not exit_code and not failed and not missing,
            'failure_kind': 'build_failure' if compile_failure and exit_code else 'test_failure' if failed else None,
            'tests': len(outcomes), 'failed': failed, 'missing': missing,
            'required_tests': {name: outcomes.get(name) for name in expected},
            'observed_tests': outcomes, 'exit_code': exit_code}


def restore_paths(test_files):
    roots = set()
    for name in test_files:
        path = PurePosixPath(name)
        parts = path.parts
        if path.is_absolute() or '..' in parts:
            raise ValueError(f'not a dedicated test path: {name}')
        end = None
        if 'tests' in parts:
            end = parts.index('tests') + 1
        else:
            for index in range(len(parts)):
                if parts[index:index + 3] == ('resources', 'test', 'fixtures'):
                    end = index + 3
                    break
                if (parts[index:index + 2] == ('src', 'rules')
                        and len(parts) > index + 3 and parts[index + 3] == 'snapshots'):
                    end = index + 4
                    break
        if end is None or end == len(parts):
            raise ValueError(f'not a dedicated test path: {name}')
        roots.add(str(PurePosixPath(*parts[:end])))
    return sorted(roots)


def configure(manifest_path):
    path = Path(manifest_path).resolve()
    manifest = json.loads(path.read_text())
    pilot.DATA = path.parent
    pilot.IMAGE = manifest['image']
    pilot.PREFIX = manifest['prefix']
    pilot.NETWORK = pilot.PREFIX + '-internal'
    pilot.PROXY = pilot.PREFIX + '-proxy'
    pilot.SUBNET = manifest['subnet']
    pilot.ORDER = [tuple(item) for item in manifest['order']]
    return manifest


def grade_task(manifest, patch, name, expected=None):
    selected = json.loads((pilot.DATA / 'selected.json').read_text())
    if expected is None:
        expected = json.loads((pilot.DATA / 'controls.json').read_text())['expected']
    container = f'{pilot.PREFIX}-grade-{uuid.uuid4().hex[:8]}'
    pilot.docker('run', '-d', '--name', container, '--network', 'none', '--cpus', '4',
                 '--memory', '8g', pilot.IMAGE, 'sleep', 'infinity')
    try:
        if patch:
            pilot.docker('exec', '-i', container, 'git', 'apply', '--binary', '-', data=patch)
        paths = restore_paths(manifest['test_files'])
        pilot.docker('exec', container, 'git', 'checkout', 'HEAD', '--', *paths)
        pilot.docker('exec', container, 'git', 'clean', '-fd', '--', *paths)
        pilot.docker('exec', '-i', container, 'git', 'apply', '-', data=selected['test_patch'])
        result = pilot.docker('exec', container, *manifest['test_command'], timeout=900, check=False)
        (pilot.DATA / f'{name}-grade.log').write_text(result.stdout)
        return grade_output(result.stdout, result.returncode, expected)
    finally:
        pilot.docker('rm', '-f', container)

def grade_agent(manifest, patch, name):
    result = grade_task(manifest, patch, name)
    if not manifest.get('require_verification'):
        return result
    try:
        run = pilot.DATA / name
        session = json.loads((run / 'stdout.json').read_text())['session_id']
        if not isinstance(session, str) or not re.fullmatch(r'[A-Za-z0-9_-]{1,128}', session):
            raise ValueError('invalid verification session')
        state = json.loads((run / 'config' / 'verified-gate' / (session + '.json')).read_text())
        status = state['status']
        count = state['repairs_requested']
        events = state['events']
        if (state['session_id'] != session or state['mode'] != 'hook'
                or status not in ['passed', 'failed', 'repair_required']
                or type(count) is not int or not 0 <= count <= 3
                or not isinstance(events, list) or not events
                or events[-1]['status'] != status or events[-1]['mode'] != 'hook'
                or events[-1]['repairs_requested'] != count):
            raise ValueError('inconsistent verification evidence')
        if type(state['pass']) is not bool or state['pass'] != (status == 'passed'):
            raise ValueError('invalid verification pass field')
        if status == 'passed' and state['check_exit'] != 0:
            raise ValueError('passing verification has a failing exit')
    except (OSError, KeyError, TypeError, ValueError) as error:
        raise ValueError(f'verification evidence invalid or missing: {error}') from error
    result['verification'] = {key: state[key] for key in
                              ['status', 'repairs_requested', 'check_exit', 'events']}
    result['pass'] = result['pass'] and state['pass']
    return result


def validate(manifest):
    selected = json.loads((pilot.DATA / 'selected.json').read_text())
    required = selected['FAIL_TO_PASS'] + selected['PASS_TO_PASS']
    positive = grade_task(manifest, selected['patch'], 'reference', required)
    if not positive['pass']:
        raise ValueError('reference control failed')
    expected = sorted(positive['observed_tests'])
    negative = grade_task(manifest, '', 'base', expected)
    if negative['pass']:
        raise ValueError('unchanged source passed hidden checks')
    controls = {'positive': positive, 'negative': negative, 'expected': expected,
                'image_id': pilot.docker('image', 'inspect', pilot.IMAGE, '--format', '{{.Id}}').stdout.strip(),
                'cli_sha256': hashlib.sha256(pilot.CLI.read_bytes()).hexdigest(),
                'cli_version': pilot.command([str(pilot.CLI), '--version']).stdout.strip(),
                'input_hashes': {name: hashlib.sha256((pilot.DATA / name).read_bytes()).hexdigest()
                                 for name in ['manifest.json', 'selected.json', 'task.txt', 'agent.Dockerfile']}}
    pilot.save('controls.json', controls)
    print(json.dumps({'task': manifest['task_id'], 'negative': negative['failure_kind'],
                      'positive_tests': len(expected), 'cli': controls['cli_version']}), flush=True)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('operation', choices=['validate', 'run'])
    parser.add_argument('manifest')
    args = parser.parse_args()
    manifest = configure(args.manifest)
    if args.operation == 'validate':
        validate(manifest)
        return
    controls = json.loads((pilot.DATA / 'controls.json').read_text())
    for name, digest in controls['input_hashes'].items():
        if hashlib.sha256((pilot.DATA / name).read_bytes()).hexdigest() != digest:
            raise ValueError(f'input changed after validation: {name}')
    costs = previous_costs(manifest['prior_directories'])
    pilot.run_pilot(prior_costs=costs, task_id=manifest['task_id'],
                    grader=lambda patch, name: grade_agent(manifest, patch, name),
                    settings=manifest.get('settings'),
                    artifacts=[Path(__file__), Path(args.manifest), pilot.DATA / 'controls.json'])


if __name__ == '__main__':
    main()
