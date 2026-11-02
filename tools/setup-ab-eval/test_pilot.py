import unittest
import json
from pathlib import Path
import tempfile
from unittest.mock import patch

from pilot import admit, parse_tests, append_event, safe_config_files, extract_patch, claim_pilot, require_run_valid
import subprocess


class PilotTests(unittest.TestCase):
    def test_budget_counts_all_costs_and_reserves_next_run(self):
        self.assertTrue(admit([1.25, 1.25], reserve=3, cap=10))
        self.assertTrue(admit([7], reserve=3, cap=10))
        self.assertFalse(admit([4, 4], reserve=3, cap=10))
        with self.assertRaises(ValueError):
            admit([None], reserve=3, cap=10)

    def test_named_test_must_run_and_all_required_tests_pass(self):
        expected = ['regression::issue', 'regression::old']
        log = 'test regression::issue ... ok\ntest regression::old ... ok\n'
        self.assertTrue(parse_tests(log, expected)['pass'])
        self.assertFalse(parse_tests(log.replace('issue ... ok', 'issue ... FAILED'), expected)['pass'])
        with self.assertRaisesRegex(ValueError, 'missing'):
            parse_tests('test result: ok. 0 passed', expected)

    def test_extra_failure_cannot_be_ignored(self):
        log = 'test regression::issue ... ok\ntest regression::extra ... FAILED\n'
        self.assertFalse(parse_tests(log, ['regression::issue'])['pass'])
    def test_spend_journal_preserves_each_attempt_before_grading(self):
        with tempfile.TemporaryDirectory() as root, patch('pilot.DATA', Path(root)):
            append_event('spend.jsonl', {'run': 'one', 'costUsd': 0.5})
            append_event('spend.jsonl', {'run': 'two', 'costUsd': None})
            rows = [json.loads(line) for line in (Path(root) / 'spend.jsonl').read_text().splitlines()]
            self.assertEqual(rows, [{'run': 'one', 'costUsd': 0.5},
                                    {'run': 'two', 'costUsd': None}])

    def test_agent_symlink_cannot_reach_host_file(self):
        with tempfile.TemporaryDirectory() as root:
            root = Path(root)
            secret = root / 'outside'
            secret.write_text('unchanged')
            config = root / 'config'
            config.mkdir()
            (config / 'escape').symlink_to(secret)
            with self.assertRaisesRegex(ValueError, 'symlink'):
                safe_config_files(config)
            self.assertEqual(secret.read_text(), 'unchanged')
    def test_restart_barrier_survives_a_new_invocation(self):
        with tempfile.TemporaryDirectory() as root, patch('pilot.DATA', Path(root)):
            claim_pilot({'test': 'frozen'})
            with self.assertRaises(FileExistsError):
                claim_pilot({'test': 'second invocation'})
            self.assertEqual(json.loads((Path(root) / 'pilot-started.json').read_text()),
                             {'test': 'frozen'})

    def test_committed_agent_fix_is_in_extracted_patch(self):
        with tempfile.TemporaryDirectory() as root:
            def git(*args):
                return subprocess.run(['git', '-c', 'commit.gpgsign=false', '-c', 'core.hooksPath=/dev/null', *args], cwd=root, check=True,
                                      capture_output=True, text=True)
            git('init', '-q')
            git('config', 'user.name', 'Test')
            git('config', 'user.email', 'test@example.invalid')
            source = Path(root) / 'source.txt'
            source.write_text('broken\n')
            git('add', '.')
            git('commit', '-qm', 'base')
            base = git('rev-parse', 'HEAD').stdout.strip()
            source.write_text('fixed\n')
            git('commit', '-qam', 'agent committed fix')
            def fake_docker(*args, **kwargs):
                self.assertEqual(args[0:2], ('exec', 'fixture'))
                self.assertEqual(args[2], 'git')
                return git(*args[3:])
            with patch('pilot.docker', fake_docker):
                result = extract_patch('fixture', base)
            self.assertIn('+fixed', result)
            self.assertIn('-broken', result)

    def test_effective_effort_mismatch_stops_the_pilot(self):
        response = {'is_error': False}
        usage = {'apiError': False, 'efforts': ['medium']}
        require_run_valid(response, 0, usage, 'medium')
        with self.assertRaises(ValueError):
            require_run_valid(response, 0, usage, 'xhigh')
        with self.assertRaises(ValueError):
            require_run_valid({'is_error': True}, 0, usage, 'medium')


if __name__ == '__main__':
    unittest.main()
