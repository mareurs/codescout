"""Actual private stdio calls; compare complete returned edges, not endpoint words."""
import argparse
import hashlib
import importlib.util
from importlib.machinery import SourceFileLoader
import json
from pathlib import Path
import re
import sqlite3
import subprocess

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[4]
parser = argparse.ArgumentParser()
parser.add_argument('label')
args = parser.parse_args()
runtime = json.loads((HERE / (args.label + '-binary.json')).read_text())
binary = Path(runtime['snapshot'])
assert hashlib.sha256(binary.read_bytes()).hexdigest() == runtime['sha256']
helper_path = HERE.parent / '2026-11-09-doc-create-dogfood/transport-helper.py.txt'
loader = SourceFileLoader('identity_mcp', str(helper_path))
spec = importlib.util.spec_from_loader(loader.name, loader)
helper = importlib.util.module_from_spec(spec)
loader.exec_module(helper)
private = ROOT / '.codescout/ecc-callgraph-identity-private-20261109'
fixture = {
    'Cargo.toml': '[package]\nname="identity_fixture"\nversion="0.1.0"\nedition="2021"\n',
    'src/lib.rs': 'pub mod a;\npub mod b;\npub fn entry() { a::call(); b::call(); }\n',
    'src/a.rs': 'pub fn call() { leaf_a(); other_a(); }\npub fn leaf_a() {}\npub fn other_a() {}\n',
    'src/b.rs': 'pub fn call() { leaf_b(); other_b(); }\npub fn leaf_b() {}\npub fn other_b() {}\n',
    '.codescout/project.toml': '[project]\nname="identity-fixture"\nlanguages=["rust"]\n',
}
fixture_hashes = {p: hashlib.sha256(t.encode()).hexdigest() for p, t in fixture.items()}
cases = []

def create_project(name):
    root = private / (args.label + '-' + name)
    root.mkdir()
    project = root / 'project'
    project.mkdir()
    for rel, text in fixture.items():
        path = project / rel
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)
    subprocess.run(['git', 'init', '-q', str(project)], check=True)
    return root, project, helper.isolated_server_env(root / 'state')

def request(probe, label, path, symbol, direction, expected):
    arguments = {'path': path, 'symbol': symbol, 'direction': direction, 'max_depth': 1, 'detail_level': 'full'}
    response = probe.request('tools/call', {'name': 'call_graph', 'arguments': arguments}, timeout=120)
    text = '\n'.join(block.get('text', '') for block in response.get('content', []) if block.get('type') == 'text')
    edges = []
    # The current MCP renderer uses rows of the form line: caller → callee (depth=N, source).
    for match in re.finditer(r'^\s*(\d+):\s+(.+?)\s+→\s+(.+?)\s+\(depth=(\d+),\s*(\w+)\)', text, re.M):
        edges.append({'line': int(match[1]), 'caller': match[2], 'callee': match[3], 'depth': int(match[4]), 'source': match[5]})
    actual = {(edge['caller'], edge['callee'], edge['depth']) for edge in edges}
    wanted = {(a, b, 1) for a, b in expected}
    record = {'label': label, 'arguments': arguments, 'response': response, 'edges': edges, 'expected': sorted(wanted), 'actual': sorted(actual), 'matches_expected': actual == wanted and len(edges) == len(wanted) and not response.get('isError', False)}
    cases.append(record)
    (HERE / (args.label + '-mcp.json')).write_text(json.dumps({'binary': runtime, 'fixture': fixture, 'fixture_sha256': fixture_hashes, 'cases': cases}, indent=2) + '\n')
    print(json.dumps({'label': label, 'expected': sorted(wanted), 'actual': sorted(actual), 'matches_expected': record['matches_expected']}), flush=True)

def leaves(letter):
    return [('call', 'leaf_' + letter), ('call', 'other_' + letter)]

root, project, env = create_project('a-then-b')
with helper.McpProbe(binary, project, env=env) as probe:
    request(probe, 'cold-a', 'src/a.rs', 'call', 'callees', leaves('a'))
    request(probe, 'warm-b-after-a', 'src/b.rs', 'call', 'callees', leaves('b'))
with helper.McpProbe(binary, project, env=env) as probe:
    request(probe, 'restart-b-after-a', 'src/b.rs', 'call', 'callees', leaves('b'))
root, project, env = create_project('cold-b')
with helper.McpProbe(binary, project, env=env) as probe:
    request(probe, 'cold-b', 'src/b.rs', 'call', 'callees', leaves('b'))
root, project, env = create_project('incoming-then-outgoing')
with helper.McpProbe(binary, project, env=env) as probe:
    request(probe, 'incoming-leaf-a', 'src/a.rs', 'leaf_a', 'callers', [('call', 'leaf_a')])
    request(probe, 'outgoing-a-after-incoming-leaf', 'src/a.rs', 'call', 'callees', leaves('a'))
summary = {'cases': len(cases), 'matching': sum(case['matches_expected'] for case in cases), 'mismatches': [case['label'] for case in cases if not case['matches_expected']]}
(HERE / (args.label + '-mcp-summary.json')).write_text(json.dumps(summary, indent=2) + '\n')
print(json.dumps(summary), flush=True)
