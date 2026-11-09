import hashlib
import json
from pathlib import Path
import subprocess
HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[4]
WORK=ROOT/".worktrees/ecc-navigation-gate"
BASE="038fffb3447b55900e617a251b6472486259e07c"
def git(*args):return subprocess.check_output(["git",*args],cwd=WORK)
assert git("rev-parse","HEAD").decode().strip()==BASE
paths=["CLAUDE.md","AGENTS.md","Cargo.toml","Cargo.lock","src/librarian/tools/create.rs","src/librarian/tools/worktree.rs","src/librarian/catalog/artifact.rs","src/librarian/catalog/augmentation.rs","src/librarian/catalog/worktree.rs","src/librarian/current_project.rs"]
records=[]
for path in paths:
    data=git("show",BASE+":"+path)
    output=HERE/"baseline-inputs"/(path.replace("/","__")+".txt")
    output.parent.mkdir(exist_ok=True);output.write_bytes(data)
    records.append(dict(path=path,git_blob=git("rev-parse",BASE+":"+path).decode().strip(),sha256=hashlib.sha256(data).hexdigest()))
skills=Path("/home/marius/work/claude/claude-plugins.worktrees/ecc-codex-codescout/ecc-codescout")
skill_records=[]
for name in ["ecc-spec-miner","ecc-silent-failure-review"]:
    data=(skills/"skills"/name/"SKILL.md").read_bytes()
    output=HERE/"controller-only"/(name+".txt");output.parent.mkdir(exist_ok=True);output.write_bytes(data)
    skill_records.append(dict(name=name,source=str(skills/"skills"/name/"SKILL.md"),sha256=hashlib.sha256(data).hexdigest()))
brief="Inspect doc(create) finalization when a catalog/augmentation/worktree operation fails. Reproduce any user-visible contract violation using isolated state, preserve normal behavior and existing disk-failure error handling, and correct only a demonstrated defect. Do not change shared catalogs, profiles or running MCP services. Record exact evidence and verification.\n"
(HERE/"baseline-inputs/candidate-brief.txt").write_text(brief)
record=dict(base=BASE,tree=git("rev-parse",BASE+"^{tree}").decode().strip(),branch=git("branch","--show-current").decode().strip(),workspace=str(WORK),baseline_inputs=records,skills=skill_records,user_request_context="Continue the established ECC dogfood workflow; retain exact baseline and inputs for a future no-skill repair replay, but do not run that replay now.",brief_note="Neutral replay brief authored before failure tests or fixes; not a full host transcript. Candidate input excludes controller-only, new solutions/tests/reports/evaluator artifacts.",dirty_source=git("status","--porcelain","--","src/librarian/tools/create.rs").decode(),shared_mcp_runtime={"pid":1520813,"build_id":"9e4db9b94323fb5bc65e3af403904f883459979206077488e448a362291620dc","git_sha":"a52d68284","dirty":True,"note":"Observed workspace(status); running shared binary differs from frozen source. Use separately built private binary for current-source runtime probes."})
assert not record["dirty_source"]
(HERE/"baseline.json").write_text(json.dumps(record,indent=2)+"\n")
print(json.dumps(dict(base=BASE,tree=record["tree"],source_inputs=len(records),skill_hashes=len(skill_records),source_clean=True)))
