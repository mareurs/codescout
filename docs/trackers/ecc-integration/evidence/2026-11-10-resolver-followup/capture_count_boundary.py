"""Retain the inspected instrument population separately from completed top-level counts."""
import hashlib,json,re,subprocess
from pathlib import Path
HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[4]
path='scripts/mutation-probe.sh'
source=(ROOT/path).read_bytes()
base=subprocess.check_output(['git','show','5b40efc48d31ccdcebbf9a9a2429c768d7aa6ca3:'+path],cwd=ROOT)
assert source==base
log=HERE/'review/fixture-mutation-disable-unavailable-retry.log'
text=log.read_text()
selected=[int(n) for n in re.findall(r'^running (\d+) tests?$',text,re.M)]
assert selected==[20,1],selected
summaries=re.findall(r'^test result: FAILED\. (\d+) passed; (\d+) failed; (\d+) ignored;',text,re.M)
assert summaries[-1]==('17','2','1'),summaries
assert 'mutation-probe: KILLED (rc=101, 21 test(s) ran)' in text
(HERE/'instrument-source.txt').write_bytes(source)
record=dict(source_path=path,source_sha256=hashlib.sha256(source).hexdigest(),unchanged_from_base=True,inspection='Post-observation snapshot, not a pre-action input',literal_predicate='Sum of all visible anchored running N tests lines',visible_selected_counts=selected,rendered_count=21,final_top_level=dict(passed=17,failed=2,ignored=1),log_sha256=hashlib.sha256(log.read_bytes()).hexdigest(),scope='Not a count of completed top-level tests; no universal count of hidden child execution; no false kill verdict demonstrated.')
(HERE/'instrument-count-boundary.json').write_text(json.dumps(record,indent=2)+'\n')
print(json.dumps(record,indent=2))
