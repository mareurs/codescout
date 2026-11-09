import hashlib
import json
from pathlib import Path
import re
import subprocess
import yaml
root=Path("/home/marius/work/claude/codescout")
evidence=root/"docs/trackers/ecc-integration/evidence/2026-11-09-callgraph-prompt-surface"
explicit=["docs/trackers/ecc-integration/README.md","docs/trackers/ecc-integration/evidence.md","docs/trackers/ecc-integration/issues.md","docs/trackers/ecc-integration/plans/README.md","docs/trackers/ecc-integration/plans/2026-11-09-callgraph-prompt-surface.md","docs/trackers/ecc-integration/reports/README.md","docs/trackers/ecc-integration/reports/2026-11-09-callgraph-prompt-surface.md","docs/trackers/prompt-hamsa-audit-log.md","docs/trackers/bug-fix-session-log.md"]
raw=[p for p in evidence.rglob("*") if p.is_file() and "__pycache__" not in p.parts and p.suffix!=".pyc"]
files=sorted(set(explicit+[str(p.relative_to(root)) for p in raw]))
for relative in explicit:
    path=root/relative
    text=path.read_text()
    assert text.startswith("---\n"),relative
    header=yaml.safe_load(text.split("---",2)[1])
    if "id" in header:assert isinstance(header["id"],str) and len(header["id"])==16,relative
    else:assert relative=="docs/trackers/bug-fix-session-log.md",relative  # existing catalog-only identity
    assert header["kind"] and header["title"],relative
    # New documents: all links. Existing ledgers: only links introduced by this work.
    diff=subprocess.check_output(["git","diff","--",relative],cwd=root,text=True)
    new=not subprocess.run(["git","ls-files","--error-unmatch",relative],cwd=root,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL).returncode==0
    inspected=text if new else "\n".join(line[1:] for line in diff.splitlines() if line.startswith("+") and not line.startswith("+++"))
    for target in re.findall(r"\]\(([^)\s]+)\)",inspected):
        if re.match(r"[a-z]+://",target) or target.startswith("#"):continue
        dest=target.split("#",1)[0]
        if dest:assert (path.parent/dest).resolve().exists(),(relative,target)
# Leak-pattern tests express their own positive and negative controls.
patterns={
"jwt":r"eyJ[A-Za-z0-9_-]{15,}\.[A-Za-z0-9_-]{15,}\.[A-Za-z0-9_-]{15,}",
"key":r"sk-[A-Za-z0-9_-]{24,}",
"token_field":r'"(?:access_token|refresh_token|id_token)"\s*:',
"personal_header":r"(?m)^#{1,3} (?:Personal Codex Configuration|Three Codex Instances)\s*$"}
positive={"jwt":"eyJ"+"A"*20+"."+"B"*20+"."+"C"*20,"key":"sk-"+"A"*30,"token_field":'"access_token":',"personal_header":"## Three Codex Instances"}
for key,pattern in patterns.items():
    assert re.search(pattern,positive[key])
    assert not re.search(pattern,"ordinary fixture navigation")
    for path in raw:
        assert path.name not in ("auth.json","AGENTS.md","config.toml"),path
        if path.name!="capture_validation.py":  # this scanner contains the declared positive-control token-field literal
            assert not re.search(pattern,path.read_text(errors="replace")),(key,str(path.relative_to(root)))
manifest=[dict(path=str(p.relative_to(root)),bytes=p.stat().st_size,sha256=hashlib.sha256(p.read_bytes()).hexdigest()) for p in sorted(raw) if p.name!="manifest.json"]
(evidence/"manifest.json").write_text(json.dumps({"excludes":"manifest itself, __pycache__ and pyc; private binary/profile outside evidence","files":manifest},indent=2)+"\n")
if "docs/trackers/ecc-integration/evidence/2026-11-09-callgraph-prompt-surface/manifest.json" not in files:files.append("docs/trackers/ecc-integration/evidence/2026-11-09-callgraph-prompt-surface/manifest.json")
Path("/tmp/ecc-prompt-owned-files.json").write_text(json.dumps(sorted(files),indent=2)+"\n")
# Snapshot exact staged entries for post-commit preservation checks.
foreign=[p for p in subprocess.check_output(["git","diff","--cached","--name-only"],cwd=root,text=True).splitlines() if p not in files]
assert foreign, "Expected existing foreign staged entries; inspect before replacing snapshot"
Path("/tmp/ecc-prompt-foreign-index-before").write_bytes(subprocess.check_output(["git","ls-files","--stage","--",*foreign],cwd=root))
print(json.dumps(dict(owned_files=len(files),manifest_files=len(manifest),checked_managed_docs=len(explicit),leak_patterns=len(patterns),inline_links="new documents and newly added links in existing documents passed")))
