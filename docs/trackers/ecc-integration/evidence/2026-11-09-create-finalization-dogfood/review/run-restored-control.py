import pathlib,subprocess,os,hashlib,json,re,datetime
e=pathlib.Path(__file__).parent
p=pathlib.Path('src/librarian/tools/create.rs')
before=p.read_bytes()
before_mtime=p.stat().st_mtime_ns
os.utime(p,None)
env=os.environ.copy();env['CARGO_BUILD_JOBS']='4'
with (e/'restored-control-rebuilt.log').open('wb') as log:
 r=subprocess.run(['cargo','test','--lib','librarian::tools::create::tests::','--','--test-threads=4'],stdout=log,stderr=subprocess.STDOUT,env=env)
out=(e/'restored-control-rebuilt.log').read_text()
result={'exit_code':r.returncode,'source_unchanged':p.read_bytes()==before,'sha256':hashlib.sha256(before).hexdigest(),'old_mtime_ns':before_mtime,'new_mtime_ns':p.stat().st_mtime_ns,'summaries':re.findall(r'^test result:.*$',out,re.M)}
(e/'restored-control-rebuilt-result.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps(result),flush=True)
