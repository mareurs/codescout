"""Validate bounded owned docs/raw provenance, freeze manifests and stage only explicit paths."""
import hashlib,json,os,re,subprocess,sys
from pathlib import Path
HERE=Path(__file__).resolve().parent;ROOT=HERE.parents[4]
DOCS={
'docs/trackers/ecc-integration/README.md':'9f27965f08caa909',
'docs/trackers/ecc-integration/evidence.md':'99fb56409faba3a2',
'docs/trackers/ecc-integration/issues.md':'d28f5200f736af9c',
'docs/trackers/ecc-integration/notes/README.md':'e0c447b3e19f2fba',
'docs/trackers/ecc-integration/notes/2026-11-10-bfs-identity-replay.md':'7767da3e63a10fa0',
'docs/trackers/ecc-integration/plans/README.md':'103e7a3b0cf03cf6',
'docs/trackers/ecc-integration/plans/2026-11-10-bfs-identity-dogfood.md':'dc8c75578af622dd',
'docs/trackers/ecc-integration/plans/2026-11-10-resolver-followup.md':'8ab869f3985be002',
'docs/trackers/ecc-integration/reports/README.md':'4cf48cb6b2600db1',
'docs/trackers/ecc-integration/reports/2026-11-10-bfs-identity-contract.md':'a61dcd3c21709b76',
'docs/trackers/ecc-integration/reports/2026-11-10-bfs-identity-dogfood.md':'8d175194a77d679c',
'docs/trackers/ecc-integration/reports/2026-11-10-bfs-identity-independent-review.md':'951f62c22db50699',
'docs/trackers/ecc-integration/reports/2026-11-10-resolver-followup.md':'58ae601a4aec9b40',
'docs/trackers/bug-fix-session-log.md':'2dd9d90bc83f9f49',
'docs/issues/archive/2026-11-10-call-graph-multihop-collapses-same-name-definitions.md':'382466d3e15a3d1d',
'docs/issues/archive/2026-11-10-call-graph-ast-fallback-ignores-lexical-shadowing.md':'dcecb5f9c3475b38',
'docs/issues/archive/2026-11-10-call-graph-display-dedupe-erases-endpoints.md':'278ed8254992c41a',
'docs/issues/archive/2026-11-10-call-graph-new-identity-guards-unpinned.md':'09a013c4d59dfc24',
'docs/issues/2026-11-10-symbols-stale-ranges-after-source-formatting.md':'4e7ec7bfc047ac7b',
'docs/issues/2026-11-10-call-graph-collector-includes-nested-function-bodies.md':'0f9e7f9b87799265',
}
DELETED='docs/issues/2026-11-10-call-graph-multihop-collapses-same-name-definitions.md'
OLD_IDS=['fed27cae5971c526','fd52f94e3abcee11','c5a8392b9db622a1','7a968049bb26b84e']
FILES=['2026-11-10-call-graph-multihop-collapses-same-name-definitions.md','2026-11-10-call-graph-ast-fallback-ignores-lexical-shadowing.md','2026-11-10-call-graph-display-dedupe-erases-endpoints.md','2026-11-10-call-graph-new-identity-guards-unpinned.md']
def git(*a):return subprocess.check_output(['git',*a],cwd=ROOT)
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
foreign=json.loads((HERE/'foreign-index-before.json').read_text())
assert {p:git('ls-files','--stage','--',p).decode().strip() for p in foreign}==foreign
integration=json.loads((HERE/'integration.json').read_text())
frozen=json.loads((HERE/'final-source.json').read_text())
assert {p:sha(ROOT/p) for p in frozen['paths']}==frozen['paths']
assert git('rev-parse',integration['integration']+'^{tree}').decode().strip()==integration['expected_gated_tree']
subprocess.run(['git','merge-base','--is-ancestor',integration['integration'],'HEAD'],cwd=ROOT,check=True)
assert json.loads((HERE/'final-gate-result.json').read_text())['exit_code']==0
assert json.loads((HERE/'integrated-gate-result.json').read_text())['exit_code']==0
baseline=json.loads((HERE/'baseline.json').read_text())
for item in baseline['inputs']:
    assert sha(HERE/item['snapshot'])==item['sha256']
assert git('rev-parse',baseline['replay_ref']).decode().strip()==baseline['base_commit']
prefix=json.loads((HERE/'session-input-provenance.json').read_text());private=Path(prefix['private_snapshot'])
assert private.stat().st_size==prefix['bytes'] and sha(private)==prefix['sha256']
assert private.stat().st_mode&0o777==0o600
subprocess.run(['git','check-ignore','-q',str(private)],cwd=ROOT,check=True)
for name in ['baseline','candidate','final']:
    binary=json.loads((HERE/(name+'-binary.json')).read_text())
    assert sha(Path(binary['snapshot']))==binary['sha256']
for location in ['review/mutation-results.json','final-mutations/results.json']:
    records=json.loads((HERE/location).read_text())
    folder=HERE/Path(location).parent
    for record in records:
        assert sha(folder/(record['id']+'.log'))==record['log_sha256']
assert len(json.loads((HERE/'review/mutation-results.json').read_text()))==13
final_mut=json.loads((HERE/'final-mutations/results.json').read_text())
assert len(final_mut)==15 and all(m['verdict']=='killed' and m['source_restored'] and m['mutation_tree_restored'] for m in final_mut)
links=[];archive_controls={}
for name,id in DOCS.items():
    path=ROOT/name;text=path.read_text()
    match=re.search(r"(?m)^id:\s*['\"]?([0-9a-f]{16})",text)
    if name=='docs/trackers/bug-fix-session-log.md':
        assert match is None, 'legacy ledger identity is catalog-derived, verified by doc.get'
    else:
        assert match and match[1]==id,(name,'id')
    if name=='docs/trackers/bug-fix-session-log.md':
        text='\n'.join(re.search(r'(?ms)^## '+fid+r' — .*?(?=^## |\Z)',text)[0] for fid in ['F-231','F-232','F-233','F-234'])
    for target in re.findall(r'(?<!!)\[[^\]]*\]\(([^)\n]+)\)',text):
        target=target.split('#',1)[0]
        if not target or '://' in target or target.startswith(('mailto:','@')):continue
        target=target.strip('<>')
        assert (path.parent/target).exists(),(name,'missing Markdown target',target)
        links.append((name,target))
    if '/issues/archive/' not in name:
        assert not any(old in text for old in OLD_IDS),(name,'old id')
        assert not any('issues/'+file in text for file in FILES),(name,'old path')
    else:
        assert 'status: fixed' in text and '## Fix provenance' in text
        assert '**SHA:** `'+integration['integration']+'` (`experiments`)' in text
        assert '**patch-id:** `'+integration['patch_id']+'`' in text
# Unrestricted tracked live-text sweep; historical raw/archive/audit surfaces are named exclusions.
patterns=[arg for file in FILES for arg in ['-e','docs/issues/'+file]]
search=subprocess.run(['git','grep','-I','-n','-F',*patterns,'--','.',':(exclude)docs/issues/archive/**',':(exclude)**/evidence/**',':(exclude).codescout/audit/**',':(exclude)**/*.jsonl'],cwd=ROOT,capture_output=True,text=True)
assert search.returncode in (0,1),search.stderr
hits=search.stdout.splitlines()
assert not hits,hits
combined='\n'.join((ROOT/p).read_text() for p in DOCS if '/issues/archive/' not in p)
for file in FILES:
    # Positive archive-form control among current Markdown links/ids, rather than a slug-only search.
    assert 'issues/archive/'+file in combined,file
    archive_controls[file]=True
# Only the selected evidence population; calibration checks the exact secret-like token predicate.
secret=re.compile(rb'(?:ghp_[A-Za-z0-9]{30,}|sk-[A-Za-z0-9]{40,})')
assert secret.search(b'ghp_'+b'A'*35) and secret.search(b'sk-'+b'B'*45)
selected=json.loads((HERE/'selected-raw-paths.json').read_text())
for name in selected:
    path=HERE/name
    if not path.exists():continue
    assert not secret.search(path.read_bytes()),('selected secret-like token',name)
    assert path.resolve()!=private.resolve()
    assert not any(x in {'.git','state','target','__pycache__'} for x in Path(name).parts)
record=dict(managed_documents=len(DOCS),local_markdown_targets=len(links),baseline_snapshots=len(baseline['inputs']),initial_mutations=13,final_mutations=15,all_final_mutants_killed=True,private_prefix_sha256=prefix['sha256'],private_prefix_bytes=prefix['bytes'],private_prefix_mode='0600',private_prefix_gitignored=True,foreign_index_unchanged=True,source_hashes_match=True,exact_integration_tree=integration['tree'],old_full_path_live_hits=hits,archive_positive_controls=archive_controls,privacy_predicate='two calibrated high-confidence token shapes on selected evidence bytes only; no exhaustive secret scan claim',limits='Managed docs+changed ledger sections, baseline snapshots, selected raw evidence, exact source/integration, private snapshot. Historical raw/archive/audit data excluded from live citation sweep.')
(HERE/'validation.json').write_text(json.dumps(record,indent=2)+'\n')
selected=sorted(set(selected+['validation.json','owned-paths.json','raw-manifest.json','finalize_evidence.py','evidence-message.txt']))
message=HERE/'evidence-message.txt'
message.write_text('docs(ecc): record BFS identity repair and replay evidence\n\nRetain exact baseline/input cuts, initial independent counterexamples and survivors, bounded correction, final15 mutation kills, native populations and both source/exact integration gates. Archive four verified bugs and preserve separate collector/navigation/count-instrument follow-ups.\n\nSource537fa820d integrateddc769e430; fix patch-id52c9b1d039bbdd7de6183eefca168f32318d8cd8. Preserve private input snapshots and six foreign staged blobs; no repair replay or runtime rollout.\n\nSession-Id: codex:01a1bf68-e650-7f20-911e-b53f86597c27\n')
owned=sorted(set(DOCS)|{str(HERE.relative_to(ROOT)/p) for p in selected}|{DELETED})
(HERE/'owned-paths.json').write_text(json.dumps(owned,indent=2)+'\n')
manifest=[dict(path=p,bytes=(ROOT/p).stat().st_size,sha256=sha(ROOT/p)) for p in owned if (ROOT/p).is_file() and str(ROOT/p)!=str(HERE/'raw-manifest.json') and str(ROOT/p).startswith(str(HERE)+'/')]
(HERE/'raw-manifest.json').write_text(json.dumps(dict(records=manifest,hash_mode='raw read_bytes; manifest excludes itself',excluded_local_runtime='excluded-local-runtime-artifacts.json'),indent=2)+'\n')
print(json.dumps(dict(validation=record,owned_paths=len(owned),raw_manifest_records=len(manifest)),indent=2))
if len(sys.argv)>1 and sys.argv[1]=='stage':
    # Include a tracked deletion once; retries stage existing paths only (F-227).
    existing=[p for p in owned if (ROOT/p).exists()]
    subprocess.run(['git','add','--',*existing],cwd=ROOT,check=True)
    if DELETED in git('diff','--name-only','--',DELETED).decode().splitlines():
        subprocess.run(['git','add','-u','--',DELETED],cwd=ROOT,check=True)
    assert {p:git('ls-files','--stage','--',p).decode().strip() for p in foreign}==foreign
    subprocess.run(['git','diff','--cached','--check','--',*DOCS],cwd=ROOT,check=True)
    print(git('diff','--cached','--stat','--',*owned).decode())
