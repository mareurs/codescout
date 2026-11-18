"""Read-only actor/cost/control audit; never reads private verdict contents."""
import collections
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
DEST = MAIN / 'docs/trackers/effort-steering/evidence/closed-campaign-audit-20261111'
sys.path.insert(0, str(WORK / 'tools/effort_protocol'))
from campaign import accounting_totals
from runner import _verify_closed


def main():
    bindings = {}

    def read(path):
        path = Path(path)
        data = path.read_bytes()
        bindings[str(path)] = hashlib.sha256(data).hexdigest()
        return json.loads(data)

    reg = read(ROOT / 'registration.json')
    closed = read(ROOT / 'actor-batch-closed.json')
    _verify_closed(reg, ROOT, closed)
    assert len(closed['attempts']) == len(reg['schedule']) == 8
    ledger = read(ROOT / 'ledger-final.json')
    old_path = MAIN / 'docs/trackers/effort-steering/evidence/headroom-accounting-20261111/reconciliation.json'
    old = read(old_path)
    assert ledger['holds'] == old['ledger']['holds']
    assert len(ledger['holds']) == 1
    assert ledger['holds'][0]['settled'] is False
    assert ledger['holds'][0]['reserved_usd'] == '4'
    assert ledger['reservations'] == []
    charges = {item['id']: item for item in ledger['charges']}
    assert len(charges) == len(ledger['charges'])
    for old_charge in old['ledger']['charges']:
        assert charges[old_charge['id']] == old_charge
        read(old_charge['source'])
        assert bindings[old_charge['source']] == old_charge['receipt_sha256']
    shared_id = reg['campaign_id'] + '::shared-charge'
    assert charges[shared_id] == {'id': shared_id, **reg['shared_preparation']}
    assert Decimal(charges[shared_id]['cost_usd']) == 0
    attempts, phases, native_sessions = [], [], []
    with localcontext() as ctx:
        ctx.prec = 128
        total = Decimal(0)
        for closure, scheduled in zip(closed['attempts'], reg['schedule']):
            path = Path(closure['attempt_receipt'])
            row = read(path)
            assert bindings[str(path)] == closure['receipt_sha256']
            assert row['arm'] == scheduled['arm'] and row['task_id'] == scheduled['task_id']
            assert row['measurement_invalid'] is False and row['cost']['unknown'] is False
            phase_sum = Decimal(0)
            for decision in row['invocations']:
                record = decision['record']
                native = Path(record['native_path'])
                invocation_path = native.parent / 'invocation.json'
                invocation = read(invocation_path)
                assert invocation == record
                assert bindings[str(invocation_path)] == decision['receipt_sha256']
                assert decision['measurement_invalid'] is False and decision['faults'] == []
                assert all(record[key] is True for key in
                           ('context_valid', 'capture_complete', 'cleanup_complete', 'cost_is_final'))
                assert record['measurement_invalid'] is False
                receipt = read(native / 'receipt.json')
                assert receipt['scripted'] is False and receipt['unknown_provider_cost'] is False
                assert receipt['context_valid'] is True
                assert receipt['cleanup_error'] is None and receipt['supervision_error'] is None
                coverage = receipt['cost_coverage']
                assert coverage['forwarded_at_terminal'] == coverage['forwarded_total'] == receipt['forwarded']
                native_sessions.append(receipt['session_id'])
                cost = Decimal(receipt['cost_usd'])
                assert cost == Decimal(record['cost_usd']) == Decimal(decision['cost']['known_usd'])
                assert decision['cost']['unknown'] is False
                charge = charges[record['invocation_id']]
                assert Decimal(charge['cost_usd']) == cost
                assert charge['receipt_sha256'] == bindings[str(invocation_path)]
                phase_sum += cost
                expected = 'xhigh' if row['arm'] == 'X' else 'low'
                efforts, models = collections.Counter(), collections.Counter()
                requests = sorted(native.glob('request-*.json'))
                for request_path in requests:
                    payload = read(request_path)
                    effort = payload['output_config']['effort']
                    model = payload['model']
                    efforts[effort] += 1
                    models[model] += 1
                    assert effort == expected and model == 'claude-sonnet-5-5'
                assert len(requests) == receipt['requests'] == receipt['forwarded']
                lifecycle = read(native / 'lifecycle-receipt.json')
                phases.append({'attempt_index': row['attempt_index'], 'task_id': row['task_id'],
                    'arm': row['arm'], 'phase': native.parent.name, 'cost_usd': str(cost),
                    'normalized_cost_is_final': record['cost_is_final'],
                    'raw_lifecycle_cost_is_final': lifecycle['cost_is_final'],
                    'raw_lifecycle_status': lifecycle['status'],
                    'raw_lifecycle_error': lifecycle['error'],
                    'proxy_cost_coverage': coverage, 'requests': len(requests),
                    'effort_counts': dict(efforts), 'model_counts': dict(models),
                    'context_capture_cleanup_verified': True,
                    'native_receipt_path': str(native / 'receipt.json')})
            assert phase_sum == Decimal(row['cost']['known_usd'])
            total += phase_sum
            attempts.append({key: row[key] for key in
                ('attempt_index', 'task_id', 'arm', 'workflow_state', 'measurement_invalid', 'cost', 'source_digest')})
        assert len(phases) == len(native_sessions) == len(set(native_sessions)) == 16
        known = sum((Decimal(item['cost_usd']) for item in ledger['charges']), Decimal(0))
        assert known == Decimal(old['known_client_cost_usd']) + total
        totals = accounting_totals(ledger)
        assert Decimal(totals['committed_usd']) == known + Decimal('4')
    _verify_closed(reg, ROOT, closed)
    # A second read detects input drift across the audit itself.
    assert all(hashlib.sha256(Path(path).read_bytes()).hexdigest() == digest
               for path, digest in bindings.items())
    report = {'schema': 'effort-closed-actor-audit-v1',
        'captured_at': datetime.datetime.now(datetime.timezone.utc).isoformat(),
        'scope': 'All eight actor receipts, sixteen native phase charges and every captured model-request effort/model field; source/input closure reopened before and after. Private verdict contents not read.',
        'attempts': attempts, 'native_phases': phases,
        'captured_model_request_files': sum(p['requests'] for p in phases),
        'phase_control_mismatches': [], 'batch_client_cost_usd': str(total),
        'prior_known_series_client_cost_usd': old['known_client_cost_usd'],
        'known_series_client_cost_usd': str(known),
        'new_shared_preparation_client_cost_usd': reg['shared_preparation']['cost_usd'],
        'accounting_totals': totals, 'reservations': ledger['reservations'],
        'private_entry_markers_at_capture': len(list(ROOT.glob('grades/*/private-entry-started.json'))),
        'private_verdict_files_at_capture': len(list(ROOT.glob('grades/*/verdict.json'))),
        'result_written_at_capture': (ROOT / 'result.json').exists(),
        'bindings': bindings,
        'limits': ['Client proxy charges are not invoice reconciliation.',
            'Historical opening balance is carried, not newly reconstructed from raw billing.',
            'Root/review/embedding/compute costs remain unmeasured and excluded.',
            'Raw lifecycle flags are preserved; final client cost is supported by terminal proxy coverage.',
            'Protocol incompleteness and private patch quality are different outcomes.']}
    DEST.mkdir(exist_ok=False)
    shutil.copy2(__file__, DEST / 'producer.py')
    (DEST / 'audit.json').write_text(json.dumps(report, indent=2, sort_keys=True) + '\n')
    print(json.dumps({key: report[key] for key in ('captured_model_request_files',
        'batch_client_cost_usd', 'known_series_client_cost_usd', 'accounting_totals',
        'private_verdict_files_at_capture', 'result_written_at_capture')}))


if __name__ == '__main__':
    main()
