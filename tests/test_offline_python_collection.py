"""Offline collection must never inherit another test's dummy credentials."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]

class OfflineCollection(unittest.TestCase):
    def run_child(self, args, home):
        env = os.environ.copy()
        for key in list(env):
            if key.startswith(("ANTHROPIC_", "OPENAI_", "CODEX_", "CLAUDE_")) or key == "JUDGE_CONFIG_DIR":
                env.pop(key)
        env["HOME"] = str(home)
        env["PYTHONPATH"] = os.pathsep.join(p for p in sys.path if "site-packages" in p)
        env["PYTEST_DISABLE_PLUGIN_AUTOLOAD"] = "1"
        env["PYTHONDONTWRITEBYTECODE"] = "1"
        return subprocess.run([sys.executable, *args], cwd=ROOT, env=env,
                              capture_output=True, text=True, timeout=60)

    def test_pure_imports_and_model_pins_need_no_credentials_or_client_calls(self):
        program = r"""
import importlib.util,json,pathlib,socket,subprocess,sys
root=pathlib.Path(sys.argv[1])
def forbidden(*args,**kwargs):
    raise AssertionError("offline import attempted a process or network call")
subprocess.run=forbidden
subprocess.Popen=forbidden
socket.create_connection=forbidden
socket.socket.connect=forbidden
def load(name,relative):
    path=root/relative
    spec=importlib.util.spec_from_file_location(name,path)
    module=importlib.util.module_from_spec(spec)
    sys.modules[name]=module
    spec.loader.exec_module(module)
    return module
sc=load("offline_scorer","scripts/phase2-score-dp1.py")
selector=load("offline_selector","scripts/phase1-span-selector.py")
miner=load("offline_miner","docs/evals/data/2026-09-24-rule-tell/stage2/mine_pairs.py")
generator=load("offline_generator","docs/evals/data/2026-09-24-rule-tell/stage2/generate_synthetic.py")
judge=load("offline_measure_judge","scripts/measure/judge.py")
pins=judge._gs()
print(json.dumps({"model":pins.CODEX_MODEL,"selector":str(pathlib.Path(miner.sel.__file__).resolve()),
                  "scorer":str(pathlib.Path(miner.sel._sc.__file__).resolve())}))
"""
        with tempfile.TemporaryDirectory() as tmp:
            result = self.run_child(["-c", program, str(ROOT)], Path(tmp))
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        facts = json.loads(result.stdout)
        self.assertEqual(Path(facts["selector"]), ROOT / "scripts/phase1-span-selector.py")
        self.assertEqual(Path(facts["scorer"]), ROOT / "scripts/phase2-score-dp1.py")
        self.assertTrue(facts["model"])

    def test_selected_files_collect_independently_in_an_empty_home(self):
        for name in ["test_measure_judge.py", "test_stage2_synthetic.py",
                     "test_phase1_span_selector_report.py"]:
            with self.subTest(file=name), tempfile.TemporaryDirectory() as tmp:
                result = self.run_child(["-m", "pytest", "--collect-only", "-q",
                                         "-p", "no:cacheprovider", "tests/" + name], Path(tmp))
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                self.assertIn("tests collected", result.stdout)

    def test_live_completion_refuses_missing_credentials_before_launch(self):
        program = r"""
import importlib.util,pathlib,subprocess,sys
root=pathlib.Path(sys.argv[1])
spec=importlib.util.spec_from_file_location("offline_live",root/"scripts/phase2-score-dp1.py")
module=importlib.util.module_from_spec(spec); spec.loader.exec_module(module)
def forbidden(*args,**kwargs):
    raise AssertionError("launched a client without credentials")
subprocess.run=forbidden
client=module.SubscriptionJudge("synthetic-model",str(pathlib.Path.home()/"absent"))
try:
    client.complete("synthetic input")
except SystemExit as error:
    assert "no subscription credentials" in str(error),error
else:
    raise AssertionError("missing credentials accepted")
print("live refusal before launch")
"""
        with tempfile.TemporaryDirectory() as tmp:
            result = self.run_child(["-c", program, str(ROOT)], Path(tmp))
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("live refusal before launch", result.stdout)

    def test_authenticated_completion_keeps_the_subscription_channel_isolated(self):
        program = r"""
import importlib.util,json,os,pathlib,subprocess,sys
root=pathlib.Path(sys.argv[1])
spec=importlib.util.spec_from_file_location("offline_channel",root/"scripts/phase2-score-dp1.py")
module=importlib.util.module_from_spec(spec); spec.loader.exec_module(module)
profile=pathlib.Path.home()/"profile"; profile.mkdir()
(profile/".credentials.json").write_text("{}")
for key in ["ANTHROPIC_API_KEY","ANTHROPIC_AUTH_TOKEN","ANTHROPIC_BASE_URL"]:
    os.environ[key]="synthetic-test-value"
calls=[]
def fake_run(command,**kwargs):
    calls.append(command)
    assert all(key not in kwargs["env"] for key in
               ["ANTHROPIC_API_KEY","ANTHROPIC_AUTH_TOKEN","ANTHROPIC_BASE_URL"])
    assert kwargs["env"]["CLAUDE_CONFIG_DIR"]==str(profile)
    assert command[:3]==["claude","-p","synthetic prompt"]
    assert command[command.index("--model")+1]=="synthetic-model"
    assert command[command.index("--tools")+1]==""
    assert "--strict-mcp-config" in command
    return subprocess.CompletedProcess(command,0,json.dumps({"result":"ANSWER: NO"}),"")
subprocess.run=fake_run
client=module.SubscriptionJudge("synthetic-model",str(profile))
assert client.complete("synthetic prompt")==("ANSWER: NO",0.0)
assert len(calls)==1
print("subscription isolation retained")
"""
        with tempfile.TemporaryDirectory() as tmp:
            result = self.run_child(["-c", program, str(ROOT)], Path(tmp))
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("subscription isolation retained", result.stdout)

if __name__ == "__main__":
    unittest.main()
