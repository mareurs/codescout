import os,json,sqlite3,subprocess,tempfile,select,time,pathlib,hashlib,datetime
BIN='/home/marius/work/claude/codescout/target/release/codescout'
DB='/home/marius/work/claude/codescout/.codescout/usage.db'
results=[]
for debug in [False,True]:
 root=pathlib.Path(tempfile.mkdtemp(prefix='codex-telemetry-probe-'));(root/'.codescout').mkdir();(root/'small.txt').write_text('small control\n')
 original=sqlite3.connect('file:'+DB+'?mode=ro',uri=True);schema=original.execute("select sql from sqlite_master where name='tool_calls'").fetchone()[0];original.close()
 c=sqlite3.connect(root/'.codescout/usage.db');c.execute(schema);c.execute("insert into tool_calls(tool_name,latency_ms,outcome) values('legacy_probe_seed',0,'success')");c.commit();c.close()
 log=open(root/'stderr.log','w');p=subprocess.Popen([BIN,'start','--project',str(root)]+(['--debug'] if debug else []),stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=log,text=True,bufsize=1)
 n=0; calls=[]
 def rpc(method,params):
  global n
  n+=1;p.stdin.write(json.dumps({'jsonrpc':'2.0','id':n,'method':method,'params':params})+'\n');p.stdin.flush();end=time.monotonic()+60
  while time.monotonic()<end:
   if not select.select([p.stdout],[],[],max(0,end-time.monotonic()))[0]:break
   line=p.stdout.readline()
   if not line:raise RuntimeError('server exited')
   v=json.loads(line)
   if v.get('id')==n:
    if 'error' in v:raise RuntimeError(v['error'])
    return v['result']
  raise TimeoutError(method)
 def call(label,name,args):
  r=rpc('tools/call',{'name':name,'arguments':args});calls.append({'label':label,'name':name,'input':args,'result':r});return r
 def handle(r):
  for b in r.get('content',[]):
   try:
    v=json.loads(b.get('text',''))
    if 'output_id' in v:return v['output_id']
   except (ValueError,TypeError):pass
  raise RuntimeError('no output_id '+str(r)[:300])
 try:
  rpc('initialize',{'protocolVersion':'2024-11-05','capabilities':{},'clientInfo':{'name':'codex-telemetry-controlled-probe','version':'1'}})
  p.stdin.write(json.dumps({'jsonrpc':'2.0','method':'notifications/initialized'})+'\n');p.stdin.flush()
  a=handle(call('produce_A','run_command',{'command':"python3 -c \"print('\\n'.join('A%04d '%i+'x'*120 for i in range(400)))\""}))
  b=handle(call('produce_B','run_command',{'command':"python3 -c \"print('\\n'.join('B%04d '%i+'y'*120 for i in range(400)))\""}))
  call('intermediate','read_file',{'path':'small.txt'})
  call('other_buffer','read_file',{'path':b,'start_line':1,'end_line':3})
  call('mention_only','grep',{'path':'small.txt','pattern':a})
  call('partial_A','read_file',{'path':a,'start_line':1,'end_line':3})
  call('correct_A','read_file',{'path':a,'start_line':398,'end_line':400})
  c=sqlite3.connect(root/'.codescout/usage.db');c.row_factory=sqlite3.Row
  cols=[x[1] for x in c.execute('pragma table_info(tool_calls)')]
  rows=[dict(x) for x in c.execute('select id,tool_name,outcome,emitted_output_id,read_output_ids,input_json,output_json,codescout_sha,codescout_dirty,session_id from tool_calls order by id')];c.close()
  assert len(rows)==8
  assert rows[0]['emitted_output_id'] is None and rows[0]['read_output_ids'] is None
  assert rows[1]['emitted_output_id']==a and rows[2]['emitted_output_id']==b
  assert json.loads(rows[4]['read_output_ids'])==[b]
  assert all(json.loads(x['read_output_ids'])==[a] for x in rows[5:])
  assert all(x['outcome']=='success' for x in rows)
  assert all((x['input_json'] is not None)==debug and (x['output_json'] is not None)==debug for x in rows[1:])
  texts={x['label']:'\n'.join(b.get('text','') for b in x['result'].get('content',[])) for x in calls}
  assert texts['mention_only']=='0 matches'
  assert 'B0000 ' in texts['other_buffer'] and 'A0000 ' not in texts['other_buffer']
  assert 'A0000 ' in texts['partial_A'] and 'A0399 ' not in texts['partial_A']
  assert 'A0399 ' in texts['correct_A'] and 'A0000 ' not in texts['correct_A']
  item={'pid':p.pid,'observed_utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'binary_sha256':hashlib.sha256(pathlib.Path(BIN).read_bytes()).hexdigest(),'debug':debug,'root':str(root),'A':a,'B':b,'columns':cols,'rows':rows,'calls':calls};results.append(item)
  print(json.dumps({'debug':debug,'root':str(root),'A':a,'B':b,'rows':[{k:v for k,v in x.items() if k not in ['input_json','output_json']} for x in rows]}),flush=True)
 finally:
  p.terminate()
  try:p.wait(timeout=10)
  except subprocess.TimeoutExpired:p.kill();p.wait()
  log.close()
pathlib.Path('/tmp/codex-telemetry-live-probe-results.json').write_text(json.dumps(results,indent=2))
