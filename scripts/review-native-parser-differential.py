from pathlib import Path
import argparse,hashlib,json

arguments=argparse.ArgumentParser(description="Review exact and explicit full-content native-parser differential evidence")
arguments.add_argument("--directory",type=Path,required=True)
arguments.add_argument("--report",type=Path,required=True)
args=arguments.parse_args()
directory=args.directory.resolve()
assert (directory/".native-parser-differential-root").read_text("utf-8")=="controlled-native-parser-fixtures-v1"
reports=[]
for path in sorted(directory.glob('*-result.json')):
    report=json.loads(path.read_text('utf-8'))
    assert not report['unexpected_differences'],report['label']
    name=path.name[:-len('-result.json')]
    core_path=directory/(name+'-core.json');reference_path=directory/(name+'-reference.json')
    core=json.loads(core_path.read_text('utf-8'));reference=json.loads(reference_path.read_text('utf-8'))
    assert hashlib.sha256(reference_path.read_bytes()).hexdigest()==report['reference_stdout_sha256']
    if report['exact']:assert core==reference,report['label']
    reports.append(report)
assert len(reports)==31
assert sum(item['adapter']=='codex-cli' for item in reports)==24
assert sum(item['adapter']=='grok-build' for item in reports)==7
assert sum(item['exact'] for item in reports)==30
intentional=[item for item in reports if not item['exact']]
assert len(intentional)==1 and intentional[0]['label']=='grok-long-complete'
required={'grok-compact-tools','grok-cache-priority','grok-config-fallback','grok-id-inference','grok-unknown-window','grok-unreadable-advisory','codex-file-long-complete'}
assert required <= {item['label'] for item in reports}
long=next(item for item in reports if item['label']=='grok-long-complete')
name=next(path.name[:-len('-result.json')] for path in directory.glob('*-result.json') if json.loads(path.read_text('utf-8'))['label']==long['label'])
core=json.loads((directory/(name+'-core.json')).read_text('utf-8'))
reference=json.loads((directory/(name+'-reference.json')).read_text('utf-8'))
def blocks(projection):
    return {block['type']:block for turn in projection['turns'] for block in turn['blocks'] if block.get('tool_use_id')=='mcp'}
actual=blocks(core);original=blocks(reference)
marker='\u5f00\u59cb'+'\u7532\u4e59\u4e19'*50000+'\u7ed3\u675f'
input_full=json.loads(actual['tool_use']['input_preview'])
output_full=json.loads(actual['tool_result']['output_preview'])
assert input_full=={'task':marker}
assert output_full=={'type':'TaskOutput','Result':{'exit_code':0,'output':marker}}
input_original=json.loads(original['tool_use']['input_preview'])
output_original=json.loads(original['tool_result']['output_preview'])
for text in [input_original['task'],output_original['Result']['output']]:
    assert text.endswith('...') and marker.startswith(text[:-3]) and len(text)<len(marker)
assert output_original['type']=='TaskOutput' and output_original['Result']['exit_code']==0
assert {item['pointer'] for item in long['intentional_differences']}=={'/turns/1/blocks/4/input_preview','/turns/1/blocks/5/output_preview'}
records=[]
for path in sorted(directory.rglob('*')):
    if path.is_file():records.append({'path':str(path),'bytes':path.stat().st_size,'sha256':hashlib.sha256(path.read_bytes()).hexdigest()})
value={'schema_version':1,'cases':31,'codex_cases':24,'grok_cases':7,'exact_projection_cases':30,'intentional_complete_content_cases':1,
    'unexpected_differences':0,'full_core_long_input_and_output_match_exact_fixture':True,
    'reference_long_fields_are_proven_prefix_truncations':True,'core_content_not_truncated_or_normalized_for_comparison':True,
    'projection_scope':'all turns/blocks/tool fields + session_stats + explicitly selected display summary; Core ownership/path fields are outside the source display DTO',
    'limits':['controlled fixtures, not every future native format or user corpus','original parser components and pinned dependency graph, not complete original App','no provider/model/Host/HTTP/GUI/platform qualification'],
    'case_reports':reports,'files':records}
args.report.write_text(json.dumps(value,ensure_ascii=False,indent=2)+'\n',encoding='utf-8',newline='\n')
print(json.dumps({key:value[key] for key in ['cases','codex_cases','grok_cases','exact_projection_cases','intentional_complete_content_cases','unexpected_differences','full_core_long_input_and_output_match_exact_fixture']},ensure_ascii=False))
