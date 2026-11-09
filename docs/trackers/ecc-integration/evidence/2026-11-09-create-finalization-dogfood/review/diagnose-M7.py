from pathlib import Path
import subprocess,sys,difflib
e=Path(__file__).parent
p=Path('src/librarian/tools/create.rs')
s=p.read_text()
(e/'M7-mutated.rs').write_text(s)
d=s
for marker in ['simulated slug failure','simulated augmentation failure']:
 find='            "'+marker+'",\n            false,'
 replace='            "'+marker+'",\n            true,'
 assert d.count(find)==1,marker
 d=d.replace(find,replace)
p.write_text(d)
(e/'M7-diagnostic.rs').write_text(d)
(e/'M7-diagnostic-fixtures.patch').write_text(''.join(difflib.unified_diff(s.splitlines(True),d.splitlines(True),fromfile='a/src/librarian/tools/create.rs',tofile='b/src/librarian/tools/create.rs')))
sys.exit(subprocess.call(['cargo','test','--lib','librarian::tools::create::tests::','--','--test-threads=4']))
