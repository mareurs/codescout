import collections
import contextlib
import importlib.util
import io
import json
import pathlib
import random
import sys
import tempfile
import threading
import unittest
from unittest.mock import patch

ROOT = pathlib.Path(__file__).resolve().parents[6]
BASE = ROOT / 'docs/evals/data/2026-09-24-rule-tell/phase1b'
spec = importlib.util.spec_from_file_location('review_tests', ROOT / 'tests/test_phase1b_audit.py')
t = importlib.util.module_from_spec(spec)
spec.loader.exec_module(t)
rl = t.rl
# Replay the reviewed pre-fix runner after the live implementation has changed.
# Keep __file__ at the real runner path so its imports and registered inputs resolve.
reviewed_source = pathlib.Path(__file__).with_name('run_labellers.before.txt').read_text()
exec(compile(reviewed_source, str(BASE / 'run_labellers.py'), 'exec'), rl.__dict__)

def rows(path):
    return [json.loads(x) for x in path.read_text().splitlines()]

items = rows(BASE / 'audit/items.jsonl')
key = rows(BASE / 'audit/key.jsonl')
sample = rows(BASE / 'audit/sample.jsonl')
candidates = rows(BASE / 'counterexamples/candidates.jsonl')
clean = t.li.ct.NEW_CLEAN + t.li.ct.codex_clean()
expected_items, expected_key = t.li.blind(t.li.entries(sample, clean, candidates))
result = {'data': {
    'items': len(items), 'unique_ids': len({i['id'] for i in items}),
    'item_fields': sorted({tuple(sorted(i)) for i in items}),
    'item_key_exact_reconstruction': (items, key) == (expected_items, expected_key),
    'audit_sample_exact_reconstruction': sample == t.da.draw(t.da.population()),
    'source_counts': dict(collections.Counter(k['source'] for k in key)),
    'audit_folds': dict(collections.Counter(s['set'] for s in sample)),
    'candidate_head_counts': dict(collections.Counter(c['head'] for c in candidates)),
    'items_bytes': (BASE / 'audit/items.jsonl').stat().st_size,
}}

def fake_answer(i):
    return dict(id=i, rules=[], unsure=False, unsure_rules=[], reason='fake review control')

def run_fake(mode):
    started, after_stop = [], []
    lock = threading.Lock()
    failed = threading.Event()
    last = threading.Event()
    failures = 0
    class FakeJudge:
        def __init__(self, *args, **kwargs):
            pass
        def complete(self, prompt):
            nonlocal failures
            batch = [json.loads(x) for x in prompt.split('## Items\n\n')[1].splitlines()]
            n = (int(batch[0]['id'][1:]) - 1) // rl.BATCH
            with lock:
                started.append(n)
                if failed.is_set():
                    after_stop.append(n)
            if mode == 'ordered-stop':
                if n == 0:
                    if not last.wait(10):
                        raise AssertionError('review scheduling control timed out')
                elif n == 1:
                    failures += 1
                    if failures == 2:
                        failed.set()
                    raise RuntimeError('review: batch 1 fails twice')
                elif n == len(rl.batches(items)) - 1:
                    last.set()
            return '\n'.join(json.dumps(fake_answer(i['id'])) for i in batch), 0.0

    with tempfile.TemporaryDirectory(prefix='codex-labeller-review-') as tmp:
        d = pathlib.Path(tmp)
        for name in ('items.jsonl', 'menu.json'):
            (d / name).write_bytes((BASE / 'audit' / name).read_bytes())
        if mode == 'overwrite':
            (d / 'labels-claude.jsonl').write_text('previous accepted labels\n')
            (d / 'run-header.json').write_text('{"previous":true}\n')
            (d / 'raw').mkdir()
            (d / 'raw/claude-b00.a1.txt').write_text('previous accepted raw answer\n')
        with patch.object(sys, 'argv', ['run_labellers.py', '--audit', tmp, '--only', 'claude', '--workers', '4']), \
             patch.object(rl, 'check_channel', lambda _: None), \
             patch.object(rl.gs, 'ClaudeGen', FakeJudge), \
             patch.object(rl.subprocess, 'run', side_effect=AssertionError('NO EXTERNAL CALLS IN REVIEW')), \
             contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
            code = rl.main()
        return {
            'exit_code': code,
            'distinct_batches_started': len(set(started)),
            'total_calls_including_retries': len(started),
            'distinct_batches_started_after_second_failure': sorted(set(after_stop)),
            'label_file_exists': (d / 'labels-claude.jsonl').exists(),
            'previous_raw_overwritten': mode == 'overwrite' and (d / 'raw/claude-b00.a1.txt').read_text() != 'previous accepted raw answer\n',
            'previous_labels_overwritten': mode == 'overwrite' and (d / 'labels-claude.jsonl').read_text() != 'previous accepted labels\n',
        }

result['ordered_stop'] = run_fake('ordered-stop')
result['overwrite'] = run_fake('overwrite')

# Run only the labeller guard tests. Two actual mutations of the production
# main function are compiled in memory; no source or shared artifacts change.
import ast
source = reviewed_source
main_node = next(n for n in ast.parse(source).body if isinstance(n, ast.FunctionDef) and n.name == 'main')
main_source = '\n'.join(source.splitlines()[main_node.lineno-1:main_node.end_lineno])
original_main = rl.main
mutations = {
    'baseline': None,
    'remove_pending_cancellation': ('cancel_futures=True', 'cancel_futures=False'),
    'swallow_stop_as_success': ('return 4', 'return 0'),
}
result['focused_tests'] = {}
for name, replacement in mutations.items():
    rl.main = original_main
    if replacement:
        old, new = replacement
        assert main_source.count(old) == 1
        exec(compile(main_source.replace(old, new), str(BASE / 'run_labellers.py'), 'exec'), rl.__dict__)
    suite = unittest.defaultTestLoader.loadTestsFromTestCase(t.Labellers)
    with patch.object(rl.subprocess, 'run', side_effect=AssertionError('NO EXTERNAL CALLS IN REVIEW')):
        outcome = unittest.TextTestRunner(stream=io.StringIO()).run(suite)
    result['focused_tests'][name] = {'ran': outcome.testsRun, 'failures': len(outcome.failures), 'errors': len(outcome.errors)}
rl.main = original_main
target = pathlib.Path('/tmp/codex-phase1b-preflight-review.json')
target.write_text(json.dumps(result, indent=2) + '\n')
print(json.dumps(result, indent=2))
