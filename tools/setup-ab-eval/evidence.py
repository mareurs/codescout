"""Read complete S5 JSONL cohorts; never infer causality across protocols."""

import argparse
from collections import Counter
from decimal import Decimal
import hashlib
import json
import math
from pathlib import Path


def nonnegative(value, field):
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise ValueError(f'{field}: expected a finite nonnegative number')
    if not math.isfinite(value) or value < 0:
        raise ValueError(f'{field}: expected a finite nonnegative number')
    return value


def checked_pass(grade):
    if grade is None or grade.get('pass') is None:
        return None
    if type(grade['pass']) is not bool:
        raise ValueError('grade.pass must be boolean or null')
    if grade['pass']:
        valid = grade.get('build') == 'ok' and grade.get('infra') is None
        for key in ('f2p', 'p2p'):
            tests = grade.get(key, {})
            valid = valid and all(isinstance(tests.get(k), list)
                                  for k in ('passed', 'failed', 'notRun'))
            valid = valid and not tests.get('failed') and not tests.get('notRun')
        valid = valid and bool(grade.get('f2p', {}).get('passed'))
        if not valid:
            raise ValueError('inconsistent pass: build, infra or test outcomes disagree')
    return grade['pass']


def summarize(rows):
    counts = Counter(records=len(rows))
    costs = []
    wall = []
    finals = set()
    tasks = {}
    efforts = Counter()
    for r in rows:
        if not isinstance(r, dict) or not r.get('runId') or not r.get('taskId'):
            raise ValueError('every record must identify runId and taskId')
        costs.append(Decimal(str(nonnegative(r.get('costUsd'), 'costUsd'))))
        wall.append(nonnegative(r.get('wallMs'), 'wallMs'))
        task = tasks.setdefault(r['taskId'], {'records': 0, 'final_runs': 0,
                                           'passes': 0, 'failures': 0,
                                           'ungraded_final_runs': 0})
        task['records'] += 1
        counts['infra_records'] += r.get('infra') is not None
        if type(r.get('final')) is not bool:
            raise ValueError('final must be boolean')
        if not r['final']:
            continue
        if r['runId'] in finals:
            raise ValueError(f'duplicate final run: {r["runId"]}')
        finals.add(r['runId'])
        task['final_runs'] += 1
        counts['final_runs'] += 1
        outcome = checked_pass(r.get('grade'))
        if outcome and r.get('infra') is not None:
            raise ValueError('inconsistent pass: infrastructure failure')
        label = 'ungraded_final_runs' if outcome is None else 'passes' if outcome else 'failures'
        counts[label] += 1
        task[label] += 1
        # This is a requested/row effort label, not measured per-request delivery.
        efforts[str(r.get('effort'))] += 1
    cost = sum(costs, Decimal(0))
    result = {key: counts[key] for key in ('records', 'final_runs', 'passes',
              'failures', 'ungraded_final_runs', 'infra_records')}
    result.update(tasks=len(tasks), cost_usd=float(cost),
                  mean_cost_per_final_run_usd=float(cost / counts['final_runs'])
                  if counts['final_runs'] else None,
                  cost_per_pass_usd=float(cost / counts['passes']) if counts['passes'] else None,
                  summed_wall_ms=sum(wall), row_effort_labels=dict(efforts),
                  per_task=dict(sorted(tasks.items())))
    return result


def report(manifest):
    result = {'schema': 1, 'interpretation': 'descriptive historical cohorts; no pooled causal verdict',
              'cohorts': {}}
    seen = set()
    for source in manifest['cohorts']:
        name = source['id']
        if name in result['cohorts']:
            raise ValueError(f'duplicate cohort: {name}')
        path = Path(source['path']).expanduser().resolve()
        if path in seen:
            raise ValueError(f'repeated source: {path}')
        seen.add(path)
        raw = path.read_bytes()
        rows = [json.loads(line) for line in raw.splitlines() if line.strip()]
        result['cohorts'][name] = {
            'source': str(path), 'sha256': hashlib.sha256(raw).hexdigest(),
            'protocol': source['protocol'], **summarize(rows)}
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('manifest', type=Path)
    parser.add_argument('--out', type=Path)
    args = parser.parse_args()
    result = report(json.loads(args.manifest.read_text()))
    rendered = json.dumps(result, indent=2, allow_nan=False) + '\n'
    if args.out:
        args.out.write_text(rendered)
    else:
        print(rendered, end='')


if __name__ == '__main__':
    main()
