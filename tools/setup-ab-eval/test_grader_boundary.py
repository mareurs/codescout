"""Exercise the grader with real Git operations on synthetic, trusted source."""
import contextlib
import json
from pathlib import Path
import subprocess
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

import baseline


class GraderBoundaryTests(unittest.TestCase):
    def exercise(self, exit_code=0):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d)
            repo, data = root / 'repo', root / 'data'
            repo.mkdir()
            data.mkdir()
            def git(*args, input=None):
                return subprocess.run(['git', '-C', str(repo), *args], input=input,
                                      text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, check=True)
            git('init', '-q')
            git('config', 'user.email', 'fixture@example.invalid')
            git('config', 'user.name', 'Fixture')
            tests = repo / 'pkg' / 'tests'
            tests.mkdir(parents=True)
            source = repo / 'pkg' / 'source.txt'
            source.write_text('broken\n')
            owned = tests / 'owned.txt'
            owned.write_text('visible base test\n')
            git('add', '.')
            git('commit', '-qm', 'trusted fixture')
            owned.write_text('visible base test\nhidden regression\n')
            hidden_patch = git('diff').stdout
            git('checkout', 'HEAD', '--', '.')
            source.write_text('fixed\n')
            owned.write_text('agent changed test\n')
            extra = tests / 'agent-extra.txt'
            extra.write_text('agent extra test\n')
            git('add', '-N', '.')
            agent_patch = git('diff').stdout
            git('reset', '--hard', 'HEAD')
            git('clean', '-fd')
            (data / 'selected.json').write_text(json.dumps({'test_patch': hidden_patch}))
            manifest = {'test_files': ['pkg/tests/owned.txt'], 'test_command': ['fixture-tests']}
            observed = []
            def docker(*args, **kwargs):
                if args[0] == 'exec':
                    at = args.index('git') if 'git' in args else None
                    if at is not None:
                        return git(*args[at + 1:], input=kwargs.get('data'))
                    if args[-1] == 'fixture-tests':
                        # This boundary observes the real filesystem produced by grade_task.
                        observed.append((source.read_text(), owned.read_text(), extra.exists()))
                        valid = observed[-1] == ('fixed\n', 'visible base test\nhidden regression\n', False)
                        return SimpleNamespace(stdout='test regression ... ' + ('ok' if valid else 'FAILED') + '\n',
                                               returncode=exit_code if valid else 101)
                return SimpleNamespace(stdout='', returncode=0)
            with contextlib.ExitStack() as stack:
                stack.enter_context(patch.object(baseline.pilot, 'DATA', data))
                stack.enter_context(patch.object(baseline.pilot, 'docker', docker))
                result = baseline.grade_task(manifest, agent_patch, 'fixture', expected=['regression'])
            return result, observed

    def test_restores_owned_tests_cleans_added_tests_and_preserves_source_fix(self):
        result, observed = self.exercise()
        self.assertTrue(result['pass'])
        self.assertEqual(observed, [('fixed\n', 'visible base test\nhidden regression\n', False)])

    def test_grader_nonzero_exit_reaches_validation(self):
        with self.assertRaisesRegex(ValueError, 'unexplained grader exit'):
            self.exercise(exit_code=2)
