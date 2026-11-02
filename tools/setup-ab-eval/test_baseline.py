import json
from pathlib import Path
import tempfile
import unittest

import baseline


class BaselineTests(unittest.TestCase):
    def history(self, root, cost=0.6):
        root.mkdir(exist_ok=True)
        records = {
            'pilot-journal.jsonl': [
                {'run': 'old', 'session': 's', 'status': 'pending', 'costUsd': None},
                {'run': 'old', 'status': 'graded', 'costUsd': cost}],
            'pilot-spend.jsonl': [{'run': 'old', 'session': 's', 'costUsd': cost}],
            'pilot-rows.jsonl': [{'runId': 'old', 'sessionId': 's', 'costUsd': cost,
                                  'cliExit': 0, 'isError': False, 'effort': 'low',
                                  'usage': {'apiError': False, 'efforts': ['low']}}],
        }
        for name, rows in records.items():
            (root / name).write_text(''.join(json.dumps(r) + '\n' for r in rows))
        return root

    def test_prior_failed_or_successful_attempts_use_the_budget(self):
        with tempfile.TemporaryDirectory() as d:
            costs = baseline.previous_costs([self.history(Path(d), 7.1)])
            self.assertEqual(costs, [7.1])
            self.assertFalse(baseline.pilot.admit(costs))

    def test_pending_unknown_spend_blocks_following_tasks(self):
        with tempfile.TemporaryDirectory() as d:
            p = self.history(Path(d))
            with (p / 'pilot-journal.jsonl').open('a') as f:
                f.write(json.dumps({'run': 'unfinished', 'session': 't', 'status': 'pending', 'costUsd': None}) + '\n')
            with self.assertRaisesRegex(ValueError, 'unresolved'):
                baseline.previous_costs([p])

    def test_protocol_violation_blocks_continuation_even_with_known_spend(self):
        with tempfile.TemporaryDirectory() as d:
            p = self.history(Path(d))
            row = json.loads((p / 'pilot-rows.jsonl').read_text())
            for changed in [{'cliExit': 1}, {'isError': True},
                            {'usage': {'apiError': True, 'efforts': ['low']}},
                            {'usage': {'apiError': False, 'efforts': ['xhigh']}}]:
                with self.subTest(changed=changed):
                    (p / 'pilot-rows.jsonl').write_text(json.dumps(row | changed) + '\n')
                    with self.assertRaisesRegex(ValueError, 'agent/effort'):
                        baseline.previous_costs([p])

    def test_duplicate_sources_and_inconsistent_costs_are_rejected(self):
        with tempfile.TemporaryDirectory() as d:
            p = self.history(Path(d))
            with self.assertRaises(ValueError):
                baseline.previous_costs([p, p])
            (p / 'pilot-rows.jsonl').write_text(json.dumps({'runId': 'old', 'sessionId': 's', 'costUsd': 0.1}) + '\n')
            with self.assertRaises(ValueError):
                baseline.previous_costs([p])

    def test_missing_history_cannot_mean_zero_cost(self):
        with tempfile.TemporaryDirectory() as d:
            with self.assertRaises((ValueError, FileNotFoundError)):
                baseline.previous_costs([Path(d)])

    def test_compile_failure_is_a_failed_attempt_not_an_excluded_run(self):
        result = baseline.grade_output("error[E0277]: trait bound not satisfied\nerror: could not compile", 101, ['regression'])
        self.assertFalse(result['pass'])
        self.assertEqual(result['failure_kind'], 'build_failure')
        self.assertEqual(result['missing'], ['regression'])

    def test_zero_exit_missing_or_ignored_required_test_is_invalid_or_failed(self):
        with self.assertRaisesRegex(ValueError, 'missing'):
            baseline.grade_output('test other ... ok\n', 0, ['regression'])
        result = baseline.grade_output('test regression ... ignored\n', 0, ['regression'])
        self.assertFalse(result['pass'])

    def test_nonzero_exit_and_extra_failure_cannot_pass(self):
        with self.assertRaisesRegex(ValueError, 'unexplained'):
            baseline.grade_output('test regression ... ok\n', 2, ['regression'])
        result = baseline.grade_output('test regression ... ok\ntest extra ... FAILED\n', 101, ['regression'])
        self.assertFalse(result['pass'])

    def test_conflicting_duplicate_test_results_are_invalid(self):
        with self.assertRaisesRegex(ValueError, 'duplicate'):
            baseline.grade_output('test regression ... FAILED\ntest regression ... ok\n', 101, ['regression'])

    def test_should_panic_tests_are_counted_and_failures_cannot_disappear(self):
        result = baseline.grade_output('test normal ... ok\ntest panic_case - should panic ... ok\n', 0, ['normal'])
        self.assertEqual(result['tests'], 2)
        self.assertEqual(result['observed_tests']['panic_case'], 'ok')
        result = baseline.grade_output('test normal ... ok\ntest panic_case - should panic ... FAILED\n', 101, ['normal'])
        self.assertFalse(result['pass'])
        self.assertEqual(result['failed'], ['panic_case'])

    def test_test_restore_paths_do_not_consume_source(self):
        self.assertEqual(baseline.restore_paths(['tokio/tests/sync_broadcast.rs']), ['tokio/tests'])
        self.assertEqual(baseline.restore_paths(['axum/src/routing/tests/mod.rs']), ['axum/src/routing/tests'])
        self.assertEqual(baseline.restore_paths([
            'crates/ruff_linter/resources/test/fixtures/pyupgrade/UP028_1.py',
            'crates/ruff_linter/src/rules/pyupgrade/snapshots/UP028.snap']), [
            'crates/ruff_linter/resources/test/fixtures',
            'crates/ruff_linter/src/rules/pyupgrade/snapshots'])
        for path in ['axum/src/routing/mod.rs', '../tests/x.rs', '/tests/x.rs',
                     'src/fixtures/production.rs', 'src/snapshots/production.rs']:
            with self.subTest(path=path), self.assertRaises(ValueError):
                baseline.restore_paths([path])

class RunnerExtensionTests(unittest.TestCase):
    def exercise(self, previous, via_main=False, tamper=False, settings=None,
                 verification=None, grader_pass=False):
        from contextlib import ExitStack
        from types import SimpleNamespace
        from unittest.mock import patch
        import hashlib
        p = baseline.pilot
        with tempfile.TemporaryDirectory() as d, ExitStack() as stack:
            root = Path(d)
            cli, token = root / 'cli', root / 'token'
            cli.write_text('fake CLI')
            token.write_text('FAKE_TOKEN')
            token.chmod(0o600)
            (root / 'task.txt').write_text('fresh task')
            (root / 'controls.json').write_text(json.dumps({
                'positive': {'pass': True}, 'negative': {'pass': False},
                'cli_sha256': hashlib.sha256(cli.read_bytes()).hexdigest(), 'image_id': 'image'}))
            calls = []
            def docker(*args, **kwargs):
                if args[:2] == ('image', 'inspect'):
                    return SimpleNamespace(stdout='image', returncode=0)
                if args[0] == 'exec' and args[2] == '/opt/claude':
                    run = root / ('fresh-task-low-0' if via_main else 'rae-20261023-low-0')
                    if settings is not None:
                        self.assertEqual(json.loads((run / 'settings.json').read_text()), settings)
                    if isinstance(verification, dict):
                        state = run / 'config/verified-gate/session-a.json'
                        state.parent.mkdir(parents=True, exist_ok=True)
                        state.write_text(json.dumps(verification))
                    calls.append(args)
                    return SimpleNamespace(stdout=json.dumps({'total_cost_usd': 0.2, 'is_error': False,
                                                              'session_id': 'session-a'}), returncode=0)
                return SimpleNamespace(stdout='base', returncode=0)
            for key, value in {'DATA': root, 'CLI': cli, 'TOKEN_FILE': token,
                               'ORDER': [('low', 0)], 'start_proxy': lambda: None,
                               'docker': docker, 'extract_patch': lambda *a: '',
                               'transcript_usage': lambda *a: {'apiError': False, 'efforts': ['low'], 'missingThinkingFields': 0}}.items():
                stack.enter_context(patch.object(p, key, value))
            if via_main:
                import sys
                history = BaselineTests().history(root / 'history', sum(previous))
                manifest = root / 'manifest.json'
                manifest.write_text(json.dumps({'image': 'image', 'prefix': 'fresh-task',
                    'subnet': '172.31.248.0/24', 'order': [['low', 0]], 'task_id': 'different/task',
                    'prior_directories': [str(history)], 'settings': settings,
                    'require_verification': verification is not None}))
                controls = json.loads((root / 'controls.json').read_text())
                controls['input_hashes'] = {'task.txt': hashlib.sha256((root / 'task.txt').read_bytes()).hexdigest()}
                (root / 'controls.json').write_text(json.dumps(controls))
                if tamper:
                    (root / 'task.txt').write_text('changed after validation')
                for key in ['IMAGE', 'PREFIX', 'NETWORK', 'PROXY', 'SUBNET']:
                    stack.enter_context(patch.object(p, key, getattr(p, key)))
                stack.enter_context(patch.object(sys, 'argv', ['baseline', 'run', str(manifest)]))
                stack.enter_context(patch.object(baseline, 'grade_task',
                    lambda *a: {'pass': grader_pass, 'failure_kind': 'build_failure'}))
                if verification == 'missing':
                    with self.assertRaisesRegex(ValueError, 'verification'):
                        baseline.main()
                    spend = [json.loads(line) for line in (root / 'pilot-spend.jsonl').read_text().splitlines()]
                    self.assertEqual(spend[-1]['costUsd'], 0.2)
                    return calls, []
                baseline.main()
            else:
                p.run_pilot(prior_costs=previous, task_id='different/task',
                            grader=lambda *a: {'pass': False, 'failure_kind': 'build_failure'}, settings=settings)
            path = root / 'pilot-rows.jsonl'
            rows = [json.loads(line) for line in path.read_text().splitlines()] if path.exists() else []
            return calls, rows

    def test_runner_does_not_reset_prior_cost_when_admitting(self):
        calls, rows = self.exercise([7.1])
        self.assertEqual(calls, [])
        self.assertEqual(rows, [])

    def test_runner_records_selected_task_and_keeps_failed_attempt(self):
        calls, rows = self.exercise([0.658846])
        self.assertEqual(len(calls), 1)
        self.assertEqual(rows[0]['taskId'], 'different/task')
        self.assertEqual(rows[0]['costUsd'], 0.2)
        self.assertFalse(rows[0]['grade']['pass'])

    def test_entrypoint_preserves_prior_spend(self):
        calls, rows = self.exercise([7.1], via_main=True)
        self.assertEqual(calls, [])
        self.assertEqual(rows, [])

    def test_entrypoint_preserves_task_identity(self):
        calls, rows = self.exercise([0.658846], via_main=True)
        self.assertEqual(len(calls), 1)
        self.assertEqual(rows[0]['taskId'], 'different/task')

    def test_entrypoint_rejects_changed_input(self):
        with self.assertRaisesRegex(ValueError, 'input changed'):
            self.exercise([0.658846], via_main=True, tamper=True)

    def test_entrypoint_installs_the_registered_stop_hook_settings(self):
        settings = {'hooks': {'Stop': [{'hooks': [{'type': 'command', 'command': 'python3 /opt/verify_stop.py hook --config /opt/verify.json'}]}]}}
        calls, rows = self.exercise([0.1], via_main=True, settings=settings)
        self.assertEqual(len(calls), 1)

    def test_direct_runner_preserves_explicit_settings(self):
        self.exercise([0.1], settings={'hooks': {'Stop': []}})

    def test_entrypoint_requires_hook_evidence_and_preserves_spend_when_missing(self):
        calls, rows = self.exercise([0.1], via_main=True, verification='missing', grader_pass=True)
        self.assertEqual(len(calls), 1)
        self.assertEqual(rows, [])

    def test_entrypoint_combines_actual_hook_and_grader_verdicts(self):
        for gate_pass, grader_pass in [(True, True), (False, True), (True, False)]:
            with self.subTest(gate_pass=gate_pass, grader_pass=grader_pass):
                status = 'passed' if gate_pass else 'failed'
                report = {'session_id': 'session-a', 'mode': 'hook', 'status': status,
                          'pass': gate_pass, 'check_exit': 0 if gate_pass else 1,
                          'repairs_requested': 1,
                          'events': [{'status': status, 'mode': 'hook', 'repairs_requested': 1}]}
                _, rows = self.exercise([0.1], via_main=True, verification=report,
                                        grader_pass=grader_pass)
                self.assertEqual(rows[0]['grade']['pass'], gate_pass and grader_pass)
