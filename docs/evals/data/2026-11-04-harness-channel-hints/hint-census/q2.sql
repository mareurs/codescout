select 'activate_project calls', count(*) from tool_calls where tool_name like '%activate_project%';
select 'workspace activate calls', count(*) from tool_calls where tool_name='workspace' and input_json like '%activate%';
select 'fg sleep run_command', count(*), sum(latency_ms)/1000 from tool_calls where tool_name='run_command' and json_extract(input_json,'$.command') like 'sleep %' and coalesce(json_extract(input_json,'$.run_in_background'),0)=0;
select 'worktree read errors', tool_name, count(*) from tool_calls where tool_name in ('read_file','symbols') and outcome!='ok' and error_msg like '%worktree%' group by tool_name;
select 'pgrep -f wait loops', count(*), sum(outcome!='ok') from tool_calls where tool_name='run_command' and json_extract(input_json,'$.command') like '%pgrep -f%' and (json_extract(input_json,'$.command') like 'while %' or json_extract(input_json,'$.command') like 'until %');
