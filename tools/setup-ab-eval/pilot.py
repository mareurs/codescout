"""Isolated external-task diagnostic. Default operations never invoke a model."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import time
import uuid

from evidence import nonnegative

HERE = Path(__file__).resolve().parent
DATA = Path('/tmp/reliable-agent-eval-20261023')
CLI = Path('/home/marius/.local/share/claude/versions/2.1.309')
TOKEN_FILE = Path('/home/marius/.config/s5/token')
S5 = Path('/home/marius/work/claude/claude-plugins.worktrees/effort-toggle-b')
IMAGE = 'reliable-agent-eval/ripgrep-2576:20261023'
PREFIX = 'rae-20261023'
NETWORK = PREFIX + '-internal'
PROXY = PREFIX + '-proxy'
SUBNET = '172.31.249.0/24'
TEST_COMMAND = ['cargo', 'test', '--offline', '--package', 'ripgrep', '--test',
                'integration', '--', 'regression', '--color', 'never']
ORDER = [('medium', 0), ('xhigh', 0), ('xhigh', 1), ('medium', 1)]


def admit(costs, reserve=3, cap=10):
    return sum(nonnegative(c, 'cost') for c in costs) + reserve <= cap


def parse_tests(log, expected):
    outcomes = dict(re.findall(r'^test (\S+) \.\.\. (ok|FAILED|ignored)\s*$', log, re.M))
    missing = sorted(set(expected) - outcomes.keys())
    if missing:
        raise ValueError(f'missing required tests: {missing}')
    failed = sorted(name for name, outcome in outcomes.items() if outcome != 'ok')
    return {'pass': not failed, 'tests': len(outcomes), 'failed': failed,
            'required_tests': {name: outcomes[name] for name in expected}}


def command(args, *, timeout=120, check=True, data=None):
    p = subprocess.run(args, input=data, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                       text=True, timeout=timeout)
    if check and p.returncode:
        raise RuntimeError(f'{args[0]} failed (exit {p.returncode}): {p.stdout[-4000:]}')
    return p


def docker(*args, **kwargs):
    return command(['docker', *map(str, args)], **kwargs)


def save(name, value):
    (DATA / name).write_text(json.dumps(value, indent=2, allow_nan=False) + '\n')
def append_event(name, event):
    with (DATA / name).open('a') as f:
        f.write(json.dumps(event, allow_nan=False) + '\n')
        f.flush()
        os.fsync(f.fileno())


def safe_config_files(config):
    root = config.resolve()
    files = []
    for path in config.rglob('*'):
        if path.is_symlink() or not path.resolve().is_relative_to(root):
            raise ValueError('agent config contains a symlink or escaped path')
        if path.is_file():
            files.append(path)
    return files
def claim_pilot(registration):
    with (DATA / 'pilot-started.json').open('x') as f:
        json.dump(registration, f)
        f.flush()
        os.fsync(f.fileno())


def extract_patch(container, base):
    docker('exec', container, 'git', 'add', '-A')
    return docker('exec', container, 'git', 'diff', '--cached', '--binary', base).stdout


def require_run_valid(response, exit_code, usage, effort):
    if exit_code or response.get('is_error') or usage['apiError'] or usage['efforts'] != [effort]:
        raise ValueError('agent/effort check failed; stopping without retry')


def grade(patch, name):
    selected = json.loads((DATA / 'selected.json').read_text())
    container = f'{PREFIX}-grade-{uuid.uuid4().hex[:8]}'
    docker('run', '-d', '--name', container, '--network', 'none', '--cpus', '4',
           '--memory', '8g', IMAGE, 'sleep', 'infinity')
    try:
        if patch:
            docker('exec', '-i', container, 'git', 'apply', '--binary', '-', data=patch)
        # Grader-owned tests always originate from the clean base, not agent edits.
        docker('exec', container, 'git', 'checkout', 'HEAD', '--', 'tests')
        docker('exec', container, 'git', 'clean', '-fd', '--', 'tests')
        docker('exec', '-i', container, 'git', 'apply', '-', data=selected['test_patch'])
        result = docker('exec', container, *TEST_COMMAND, timeout=600, check=False)
        (DATA / f'{name}-grade.log').write_text(result.stdout)
        grade_result = parse_tests(result.stdout, selected['FAIL_TO_PASS'] + selected['PASS_TO_PASS'])
        grade_result['exit_code'] = result.returncode
        if result.returncode and grade_result['pass']:
            raise ValueError('test exit code conflicts with parsed passing tests')
        return grade_result
    finally:
        docker('rm', '-f', container)


def prepare():
    result = command(['docker', 'build', '--network', 'none', '-t', IMAGE,
                      '-f', str(HERE / 'agent.Dockerfile'), str(HERE)], timeout=900, check=False)
    (DATA / 'image-build.log').write_text(result.stdout)
    if result.returncode:
        raise RuntimeError('agent image build failed; see image-build.log')
    negative = grade('', 'clean-base')
    positive = grade((DATA / 'reference.patch').read_text(), 'clean-reference')
    if negative['pass'] or not positive['pass'] or negative['failed'] != ['regression::r2574']:
        raise ValueError('no-change/reference control failed')
    image_id = docker('image', 'inspect', IMAGE, '--format', '{{.Id}}').stdout.strip()
    checks = {'negative': negative, 'positive': positive, 'image_id': image_id,
              'cli': str(CLI), 'cli_sha256': hashlib.sha256(CLI.read_bytes()).hexdigest(),
              'cli_version': command([str(CLI), '--version']).stdout.strip()}
    save('controls.json', checks)
    print(json.dumps(checks, indent=2))


def start_proxy():
    config = DATA / 'proxy'
    config.mkdir(exist_ok=True)
    # Reuse the S5 renderer: placeholders in comments must not be substituted.
    module = S5 / 'effort-steering/spikes/s5/proxy.mjs'
    template = S5 / 'effort-steering/spikes/s5/images/tinyproxy.conf.tmpl'
    script = f'import {{renderProxyConfig}} from {json.dumps(module.as_uri())};'
    script += 'import {readFileSync} from "node:fs";'
    script += 'console.log(JSON.stringify(renderProxyConfig(readFileSync(process.argv[1],"utf8"),'
    script += '{mode:"allowlist",allowlist:["api.anthropic.com"],subnets:[process.argv[2]]})));'
    rendered = json.loads(command(['node', '--input-type=module', '-e', script,
                                    str(template), SUBNET]).stdout)
    (config / 'tinyproxy.conf').write_text(rendered['conf'])
    (config / 'filter').write_text(rendered['filter'])
    docker('network', 'create', '--internal', '--subnet', SUBNET, NETWORK)
    docker('run', '-d', '--name', PROXY, '--network', NETWORK,
           '--read-only', '--cap-drop', 'ALL', '--security-opt', 'no-new-privileges',
           '-v', f'{config}:/etc/s5-proxy:ro', 's5/proxy:latest')
    docker('network', 'connect', 'bridge', PROXY)
    # Fail-closed checks: denied CONNECT must reach the proxy and get a 403.
    script = f'''import socket
for host, expected in [("github.com",b"403"),("huggingface.co",b"403"),("api.anthropic.com",b"200")]:
    s=socket.create_connection(("{PROXY}",8888),10)
    request=("CONNECT "+host+":443 HTTP/1.1\\r\\nHost: "+host+":443\\r\\n\\r\\n").encode()
    s.sendall(request)
    assert expected in s.recv(4096)
    s.close()
s=socket.socket();s.settimeout(3)
assert s.connect_ex(("1.1.1.1",443)) != 0
'''
    # The existing Python image is only a network probe, with no benchmark files.
    probe = docker('run', '--rm', '--network', NETWORK, 'python:3.12-slim',
                   'python', '-c', script, check=False)
    if probe.returncode:
        logs = docker('logs', PROXY, check=False).stdout
        raise RuntimeError(f'network probe failed: {probe.stdout}\nproxy log: {logs}')


def transcript_usage(config, session):
    module = S5 / 'effort-steering/spikes/s5/usage.mjs'
    script = f'import {{readRunUsage}} from {json.dumps(module.as_uri())};'
    script += 'console.log(JSON.stringify(readRunUsage(process.argv[1],process.argv[2])));'
    return json.loads(command(['node', '--input-type=module', '-e', script,
                               str(config), session]).stdout)


def run_pilot(prior_costs=(), task_id='burntsushi__ripgrep-2576', grader=None, artifacts=(), settings=None):
    if settings is not None and not isinstance(settings, dict):
        raise ValueError('settings must be an object')
    controls = json.loads((DATA / 'controls.json').read_text())
    if not controls['positive']['pass'] or controls['negative']['pass']:
        raise ValueError('controls are not valid')
    if hashlib.sha256(CLI.read_bytes()).hexdigest() != controls['cli_sha256']:
        raise ValueError('CLI changed after validation')
    if docker('image', 'inspect', IMAGE, '--format', '{{.Id}}').stdout.strip() != controls['image_id']:
        raise ValueError('agent image changed after validation')
    rows_file = DATA / 'pilot-rows.jsonl'
    if rows_file.exists() or (DATA / 'pilot-started.json').exists():
        raise ValueError('pilot already started; inspect rows before any further spending')
    token_stat = TOKEN_FILE.lstat()
    if not TOKEN_FILE.is_file() or TOKEN_FILE.is_symlink() or token_stat.st_mode & 0o077:
        raise ValueError('token file permissions/type invalid')
    token = TOKEN_FILE.read_text().strip()
    if not token or any(c.isspace() for c in token):
        raise ValueError('invalid token file')
    costs = [nonnegative(cost, 'prior cost') for cost in prior_costs]
    # Exclusive creation is a durable restart barrier, including crashes before any result.
    claim_pilot({'order': ORDER, 'task_id': task_id, 'prior_costs': costs.copy(), 'settings': settings or {}, 'controls_sha256': hashlib.sha256((DATA / 'controls.json').read_bytes()).hexdigest(),
                 'artifact_sha256': {str(p): hashlib.sha256(p.read_bytes()).hexdigest()
                     for p in [HERE / 'pilot.py', HERE / 'pilot-protocol.md', HERE / 'agent.Dockerfile', DATA / 'task.txt', *artifacts]}})
    try:
        start_proxy()
        for effort, rep in ORDER:
            if not admit(costs):
                break
            session = str(uuid.uuid4())
            name = f'{PREFIX}-{effort}-{rep}'
            run_dir = DATA / name
            config = run_dir / 'config'
            config.mkdir(parents=True)
            (run_dir / 'settings.json').write_text(json.dumps(settings or {}) + '\n')
            env_file = run_dir / 'private.env'
            env_file.touch(mode=0o600)
            env_file.write_text(f'CLAUDE_CODE_OAUTH_TOKEN={token}\nCLAUDE_CONFIG_DIR=/run/config\n'
                f'HTTPS_PROXY=http://{PROXY}:8888\nNO_PROXY=127.0.0.1,localhost\n'
                'CLAUDE_CODE_MAX_OUTPUT_TOKENS=8192\nDISABLE_AUTOUPDATER=1\n')
            container_started = False
            started = time.monotonic()
            result = None
            try:
                docker('run', '-d', '--name', name, '--network', NETWORK,
                       '--cpus', '4', '--memory', '8g', '--env-file', env_file,
                       '--cap-drop', 'ALL', '--security-opt', 'no-new-privileges',
                       '-v', f'{config}:/run/config',
                       '-v', f'{CLI}:/opt/claude:ro',
                       '-v', f'{run_dir / "settings.json"}:/opt/settings.json:ro',
                       IMAGE, 'sleep', 'infinity')
                container_started = True
                env_file.unlink()
                base = docker('exec', name, 'git', 'rev-parse', 'HEAD').stdout.strip()
                append_event('pilot-journal.jsonl', {'run': name, 'session': session, 'status': 'pending', 'costUsd': None})
                args = ['/opt/claude', '-p', '--model', 'claude-sonnet-5-5',
                        '--effort', effort, '--session-id', session,
                        '--setting-sources', 'local', '--settings', '/opt/settings.json',
                        '--strict-mcp-config', '--dangerously-skip-permissions',
                        '--tools', 'Bash,Read,Write,Edit,Glob,Grep',
                        '--output-format', 'json', '--max-budget-usd', '1.25',
                        '--', (DATA / 'task.txt').read_text()]
                result = docker('exec', name, *args, timeout=600, check=False)
                output = result.stdout.replace(token, '[REDACTED]')
                (run_dir / 'stdout.json').write_text(output)
                response = json.loads(output)
                cost = nonnegative(response.get('total_cost_usd'), 'total_cost_usd')
                costs.append(cost)
                append_event('pilot-spend.jsonl', {'run': name, 'session': session, 'costUsd': cost})
                safe_config_files(config)
                usage = transcript_usage(config, session)
                # Preserve added source files as well as modified tracked files.
                patch = extract_patch(name, base)
                (run_dir / 'agent.patch').write_text(patch)
                grade_result = (grader or grade)(patch, name)
                row = {'runId': name, 'taskId': task_id,
                       'effort': effort, 'rep': rep, 'sessionId': session,
                       'costUsd': cost, 'wallMs': round((time.monotonic() - started) * 1000),
                       'cliExit': result.returncode, 'isError': response.get('is_error'),
                       'numTurns': response.get('num_turns'), 'usage': usage,
                       'grade': grade_result}
                append_event('pilot-rows.jsonl', row)
                append_event('pilot-journal.jsonl', {'run': name, 'status': 'graded', 'costUsd': cost})
                print(json.dumps({'run': name, 'cost': cost, 'cumulative': sum(costs),
                                  'pass': grade_result['pass'], 'efforts': usage['efforts']}), flush=True)
                require_run_valid(response, result.returncode, usage, effort)
            finally:
                env_file.unlink(missing_ok=True)
                if container_started:
                    docker('rm', '-f', name)
                for path in safe_config_files(config):
                    raw = path.read_bytes()
                    if token.encode() in raw:
                        path.write_bytes(raw.replace(token.encode(), b'[REDACTED]'))
    finally:
        docker('rm', '-f', PROXY, check=False)
        docker('network', 'rm', NETWORK, check=False)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('action', choices=['prepare', 'network', 'run'])
    args = parser.parse_args()
    if args.action == 'prepare':
        prepare()
    elif args.action == 'network':
        try:
            start_proxy()
            save('network-controls.json', {'internal_network': NETWORK,
                 'allowed': ['api.anthropic.com'], 'github_connect_denied': True,
                 'direct_external_denied': True})
            print('Network isolation controls passed')
        finally:
            docker('rm', '-f', PROXY, check=False)
            docker('network', 'rm', NETWORK, check=False)
    else:
        run_pilot()


if __name__ == '__main__':
    main()
