"""Descriptive paired results even when the registered screen exits early."""
import datetime
import hashlib
import json
from decimal import Decimal, localcontext
from pathlib import Path
import shutil
import sys

WORK = Path('/home/marius/.codex/worktrees/cf66/codescout')
MAIN = Path('/home/marius/work/claude/codescout')
ROOT = Path('/var/tmp/ep11-01a15/paid-campaign-headroom-v3')
DEST = MAIN / 'docs/trackers/effort-steering/evidence/paired-pilot-results-20261111'
sys.path.insert(0, str(WORK / 'tools/effort_protocol'))
from runner import _verify_closed, analyze


def main():
    bindings = {}

    def read(path):
        path = Path(path)
        data = path.read_bytes()
        bindings[str(path)] = hashlib.sha256(data).hexdigest()
        return json.loads(data)

    # No waiting, polling, actors or provider calls; absent result means not ready.
    result = read(ROOT / 'result.json')
    reg = read(ROOT / 'registration.json')
    closed = read(ROOT / 'actor-batch-closed.json')
    _verify_closed(reg, ROOT, closed)
    assert len(result['attempts']) == len(closed['attempts']) == 8
    assert result['unexecuted'] == []
    assert result['analysis'] == analyze(result)
    cost_audit = read(MAIN / 'docs/trackers/effort-steering/evidence/closed-campaign-audit-20261111/audit.json')
    assert bindings[str(MAIN / 'docs/trackers/effort-steering/evidence/closed-campaign-audit-20261111/audit.json')] == '838b384d8b0c871f32d98324da4686063cdedc3cab5509f5a3a6e620492309af'
    rows, table, uninterpretable = [], {}, []
    with localcontext() as ctx:
        ctx.prec = 128
        totals = {arm: Decimal(0) for arm in ('L', 'R', 'P', 'X')}
        for row, closure in zip(result['attempts'], closed['attempts']):
            original = read(closure['attempt_receipt'])
            assert bindings[closure['attempt_receipt']] == closure['receipt_sha256']
            assert all(row[key] == value for key, value in original.items() if key != 'grade')
            key = (row['task_id'], row['arm'])
            assert key not in table
            grade = row['grade']
            if grade is not None:
                on_disk = read(ROOT / 'grades' / f"{row['attempt_index']:02}" / 'verdict.json')
                assert on_disk == grade
            interpretable = (row['measurement_invalid'] is False and row['cost']['unknown'] is False
                and isinstance(grade, dict) and grade.get('valid') is True
                and type(grade.get('pass')) is bool)
            if not interpretable:
                uninterpretable.append(row['attempt_index'])
            assert row['cost']['known_usd'] is not None and row['cost']['unknown'] is False
            totals[row['arm']] += Decimal(row['cost']['known_usd'])
            summary = {name: row[name] for name in ('attempt_index', 'task_id', 'arm',
                'workflow_state', 'measurement_invalid', 'cost', 'source_digest')}
            summary.update(interpretable=interpretable, private_pass=grade.get('pass') if grade else None,
                private_valid=grade.get('valid') if grade else None,
                private_kind=grade.get('kind') if grade else None,
                private_adjusted=grade.get('adjusted') if grade else None,
                private_reason=grade.get('reason') if grade else None)
            rows.append(summary)
            table[key] = summary
        task_ids = list(dict.fromkeys(row['task_id'] for row in rows))
        assert len(task_ids) == 2
        assert set(table) == {(task, arm) for task in task_ids for arm in totals}
        batch_cost = sum(totals.values(), Decimal(result['setup_cost_usd']))
        assert batch_cost == Decimal(cost_audit['batch_client_cost_usd'])
        paired, hidden_passes, closed_and_passes, per_pass = {}, {}, {}, {}
        screen = {'all_eight_interpretable': not uninterpretable,
            'both_P_protocols_closed': all(table[(task, 'P')]['workflow_state'] == 'closed' for task in task_ids),
            'P_total_cost_below_X': totals['P'] < totals['X']}
        if not uninterpretable:
            for arm in totals:
                hidden_passes[arm] = sum(table[(task, arm)]['private_pass'] for task in task_ids)
                closed_and_passes[arm] = sum(table[(task, arm)]['private_pass'] and
                    table[(task, arm)]['workflow_state'] == 'closed' for task in task_ids)
                per_pass[arm] = str(totals[arm] / hidden_passes[arm]) if hidden_passes[arm] else None
            for arm in ('L', 'R', 'X'):
                outcomes = []
                for task in task_ids:
                    p, other = table[(task, 'P')]['private_pass'], table[(task, arm)]['private_pass']
                    outcomes.append({'task_id': task, 'P_pass': p, 'other_pass': other,
                        'outcome': 'tie' if p == other else 'win' if p else 'loss'})
                paired['P-' + arm] = {'task_outcomes': outcomes,
                    'wins': sum(item['outcome'] == 'win' for item in outcomes),
                    'losses': sum(item['outcome'] == 'loss' for item in outcomes),
                    'ties': sum(item['outcome'] == 'tie' for item in outcomes)}
            screen['P_gains_over_L_and_R_without_losses'] = all(paired['P-' + arm]['wins'] >= 1
                and paired['P-' + arm]['losses'] == 0 for arm in ('L', 'R'))
            screen['P_hidden_pass_count_at_least_X'] = hidden_passes['P'] >= hidden_passes['X']
        advance = len(screen) == 5 and all(screen.values()) and result['status'] == 'completed'
        assert result['analysis']['advance'] == advance
    _verify_closed(reg, ROOT, closed)
    assert all(hashlib.sha256(Path(path).read_bytes()).hexdigest() == digest for path, digest in bindings.items())
    report = {'schema': 'effort-paired-pilot-descriptive-v1',
        'captured_at': datetime.datetime.now(datetime.timezone.utc).isoformat(),
        'controller_status': result['status'], 'controller_reason': result.get('reason'),
        'rows': rows, 'task_ids': task_ids, 'unit': 'task; two tasks, no resamples',
        'uninterpretable_attempt_indices': uninterpretable, 'model_usd': {k: str(v) for k, v in totals.items()},
        'shared_preparation_usd': result['setup_cost_usd'], 'batch_client_cost_usd': str(batch_cost),
        'hidden_pass_candidate_counts': hidden_passes, 'closed_and_hidden_pass_workflow_counts': closed_and_passes,
        'client_usd_per_hidden_pass_candidate': per_pass, 'paired_private_patch_outcomes': paired,
        'necessary_screen_conditions': screen, 'advance': advance,
        'decision': 'ADVANCE_TO_REGISTERED_VALIDATION' if advance else 'NO-GO',
        'original_runner_analysis': result['analysis'], 'bindings': bindings,
        'limits': ['Two tasks are a diagnostic development screen, not generic validation or equivalence.',
            'Incomplete attempts and all their charges remain in primary results.',
            'Hidden patch acceptance and protocol closure are separately reported.',
            'Lean X captured an active whole-change mutant at its continuation limit; no corrected candidate is substituted.',
            'Client proxy charges exclude unmeasured root/review/embedding/compute and are not invoices.',
            'Historical unresolved 4 USD hold remains outside this batch in the standing series ledger.']}
    DEST.mkdir(exist_ok=False)
    shutil.copy2(__file__, DEST / 'producer.py')
    (DEST / 'analysis.json').write_text(json.dumps(report, indent=2, sort_keys=True) + '\n')
    print(json.dumps({key: report[key] for key in ('controller_status', 'model_usd',
        'hidden_pass_candidate_counts', 'closed_and_hidden_pass_workflow_counts',
        'necessary_screen_conditions', 'decision')}))


if __name__ == '__main__':
    main()
