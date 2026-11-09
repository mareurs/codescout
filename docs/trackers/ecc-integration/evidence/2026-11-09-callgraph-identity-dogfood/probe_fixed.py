"""Qualified native controls, including persisted legacy state; retain original probe separately."""
import copy
import hashlib
import importlib.util
from importlib.machinery import SourceFileLoader
import json
from pathlib import Path
import re
import sqlite3
import subprocess
import sys

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[4]
label = sys.argv[1]
runtime = json.loads((HERE / (label + '-binary.json')).read_text())
binary = Path(runtime['snapshot'])
assert hashlib.sha256(binary.read_bytes()).hexdigest() == runtime['sha256']
baseline = json.loads((HERE / 'baseline-mcp.json').read_text())
fixture = baseline['fixture']
loader = SourceFileLoader('fixed_identity_mcp', str(HERE.parent / '2026-11-09-doc-create-dogfood/transport-helper.py.txt'))
spec = importlib.util.spec_from_loader(loader.name, loader)
helper = importlib.util.module_from_spec(spec)
loader.exec_module(helper)
private = ROOT / '.codescout/ecc-callgraph-identity-private-20261109'
records = []

def expected(letter):
    return [('call', 'leaf_' + letter, 'src/' + letter + '.rs', 1), ('call', 'other_' + letter, 'src/' + letter + '.rs', 1)]

def inspect(response, symbol, wanted):
    text = response.get('content', [{}])[0].get('text', '')
    edges = []
    file = None
    for line in text.splitlines():
        header = re.fullmatch(r'\s+(\S+) \(\d+\)', line)
        if header:
            file = header[1]
        edge = re.fullmatch(r'\s*\d+:\s+(.+?) → (.+?) \(depth=(\d+), (lsp|ts)\)', line)
        if edge:
            edges.append((edge[1], edge[2], file, int(edge[3])))
    totals = re.search(r'callees: (\d+) edges across (\d+) files', text)
    matches = (not response.get('isError', False) and text.startswith('call_graph for `' + symbol + '`\n')
               and totals is not None and int(totals[1]) == len(edges)
               and int(totals[2]) == len({edge[2] for edge in edges})
               and len(edges) == len(wanted) and set(edges) == set(wanted)
               and 'warning:' not in text and 'completeness_warning' not in text)
    return {'edges': edges, 'matches_expected': bool(matches)}

def project(name):
    root = private / (label + '-v2-' + name)
    root.mkdir()
    path = root / 'project'
    path.mkdir()
    for rel, text in fixture.items():
        out = path / rel
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_text(text)
    subprocess.run(['git', 'init', '-q', str(path)], check=True)
    return root, path, helper.isolated_server_env(root / 'state')

def request(probe, name, letter, symbol='call', wanted=None):
    wanted = expected(letter) if wanted is None else wanted
    arguments = {'path': 'src/' + letter + '.rs', 'symbol': symbol, 'direction': 'callees', 'max_depth': 1, 'detail_level': 'full'}
    response = probe.request('tools/call', {'name': 'call_graph', 'arguments': arguments}, timeout=120)
    record = dict(label=name, arguments=arguments, response=response, expected=wanted, **inspect(response, symbol, wanted))
    records.append(record)
    (HERE / (label + '-mcp.json')).write_text(json.dumps({'binary': runtime, 'fixture_sha256': baseline['fixture_sha256'], 'cases': records}, indent=2) + '\n')
    print(json.dumps({k: record[k] for k in ['label', 'edges', 'matches_expected']}), flush=True)
    return response

root, path, env = project('a-then-b')
with helper.McpProbe(binary, path, env=env) as probe:
    cold = request(probe, 'cold-a', 'a')
    request(probe, 'warm-b-after-a', 'b')
with helper.McpProbe(binary, path, env=env) as probe:
    request(probe, 'restart-b-after-a', 'b')
root, path, env = project('cold-b')
with helper.McpProbe(binary, path, env=env) as probe:
    request(probe, 'cold-b', 'b')
    request(probe, 'quiet-leaf', 'b', symbol='leaf_b', wanted=[])
with sqlite3.connect(path / '.codescout/call_edges.db') as conn:
    empty_cached = conn.execute("SELECT count(*) FROM call_edge_resolutions WHERE symbol='leaf_b'").fetchone()[0]
assert empty_cached == 0
legacy = private / 'baseline-a-then-b'
path = legacy / 'project'
assert all((path / rel).read_text() == text for rel, text in fixture.items())
with sqlite3.connect(path / '.codescout/call_edges.db') as conn:
    before = conn.execute('SELECT caller_sym,callee_sym,file,line,col FROM call_edges ORDER BY caller_sym,callee_sym,file,line,col').fetchall()
    had_snapshot_table = bool(conn.execute("SELECT 1 FROM sqlite_master WHERE name='call_edge_resolutions'").fetchone())
assert any(row[1] == 'leaf_a' for row in before)
with helper.McpProbe(binary, path, env=helper.isolated_server_env(legacy / 'state')) as probe:
    request(probe, 'persisted-legacy-cache-b', 'b')
with sqlite3.connect(path / '.codescout/call_edges.db') as conn:
    scoped = conn.execute('SELECT symbol,definition_file,definition_line,definition_col,direction FROM call_edge_resolutions ORDER BY symbol,definition_file').fetchall()
legacy_record = {'raw_rows_before': before, 'had_snapshot_table_before': had_snapshot_table, 'completed_resolution_keys_after': scoped, 'fixture_bytes_unchanged': True}
(HERE / (label + '-legacy-cache.json')).write_text(json.dumps(legacy_record, indent=2) + '\n')
# Calibrate the scorer on actual returned data and deliberate single-axis perturbations.
controls = [('actual-cold-a', cold, True)]
for name, old, new in [('wrong-file', 'src/a.rs', 'src/b.rs'), ('wrong-callee', 'leaf_a', 'leaf_b'), ('wrong-depth', 'depth=1', 'depth=2')]:
    mutant = copy.deepcopy(cold)
    mutant['content'][0]['text'] = mutant['content'][0]['text'].replace(old, new)
    controls.append((name, mutant, False))
mutant = copy.deepcopy(cold)
mutant['content'].append(copy.deepcopy(mutant['content'][0]))
mutant['content'][0]['text'] = 'No completed graph; endpoint words in a later guide are insufficient.'
controls.append(('guide-only-edges', mutant, False))
results = [{'name': name, 'expected': want, 'actual': inspect(response, 'call', expected('a'))['matches_expected']} for name, response, want in controls]
assert all(row['actual'] == row['expected'] for row in results)
(HERE / (label + '-scorer-controls.json')).write_text(json.dumps(results, indent=2) + '\n')
summary = {'cases': len(records), 'matching': sum(row['matches_expected'] for row in records), 'scorer_controls': len(results), 'empty_leaf_snapshots': empty_cached}
(HERE / (label + '-mcp-summary.json')).write_text(json.dumps(summary, indent=2) + '\n')
print(json.dumps(summary), flush=True)
assert all(row['matches_expected'] for row in records)
