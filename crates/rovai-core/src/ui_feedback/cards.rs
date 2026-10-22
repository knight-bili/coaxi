//! 仅从 Core 已发布的队员回复定位卡片。前端不提供原消息正文或权限。
use super::*;
use rusqlite::{params,Connection,OptionalExtension};

pub fn source_body(connection:&Connection,scope:&Scope,agent:&str,message:&str)->Result<String>{
    storage::scope(connection,scope,agent)?;
    let body:Option<String>=if scope.scope_kind=="public"{
        let publication=crate::camp_message_publication::public_camp_message_publication_cte();
        connection.query_row(&format!("WITH {publication} SELECT m.body FROM camp_message m JOIN public_camp_message_publication p ON p.message_id=m.id JOIN agent_run r ON r.id=m.source_agent_run_id WHERE m.id=?1 AND m.camp_id=?2 AND m.author_type='agent' AND m.author_id=?3 AND r.agent_id=m.author_id AND m.tombstoned_at IS NULL AND m.recall_state NOT IN ('recallable','withdrawn')"),params![message,scope.thread_id,agent],|row|row.get(0)).optional()?
    }else{
        connection.query_row("SELECT m.body FROM conversation_message m JOIN conversation c ON c.id=m.conversation_id JOIN agent_run r ON r.id=m.source_agent_run_id WHERE m.id=?1 AND m.conversation_id=?2 AND c.kind=?3 AND c.camp_id=?4 AND m.author_type='agent' AND m.author_id=?5 AND c.agent_id=m.author_id AND m.source_camp_message_id IS NULL AND m.source_inbox_message_id IS NULL AND r.conversation_id=c.id AND r.agent_id=m.author_id AND r.final_conversation_message_id=m.id AND r.status IN ('succeeded','failed','cancelled')",params![message,scope.conversation_id,scope.scope_kind,scope.thread_id,agent],|row|row.get(0)).optional()?
    };
    let body=body.context("feedback.card_source_unavailable")?;ensure!(body.len()<=1024*1024,"feedback.card_source_limit");Ok(body)
}

/// CommonMark 的 fenced code 规则；只定位 spec，其他代码块不会贡献控件。
pub fn fences(body:&str)->Result<Vec<String>>{
    let body=body.replace("\r\n","\n").replace('\r',"\n");
    let mut result=Vec::new();let mut active:Option<(char,usize,usize,bool,Vec<String>)>=None;
    for line in body.split('\n'){
        let indent=line.chars().take_while(|ch|*ch==' ').count();let text=line.trim_start_matches(' ');
        if let Some((mark,length,_,_,_))=&active{
            let closing=text.chars().take_while(|ch|ch==mark).count();
            if indent<=3&&closing>=*length&&text[closing..].trim().is_empty(){
                let(_,_,_,spec,lines)=active.take().expect("active fence");
                if spec{let value=lines.join("\n")+"\n";ensure!(value.len()<=256*1024&&result.len()<128,"feedback.card_source_limit");result.push(value)}
                continue
            }
            if let Some((_,_,opening_indent,_,lines))=&mut active{let remove=indent.min(*opening_indent);lines.push(line[remove..].to_string())}
            continue
        }
        if indent>3{continue}
        let Some(mark)=text.chars().next().filter(|ch|matches!(ch,'`'|'~'))else{continue};let length=text.chars().take_while(|ch|*ch==mark).count();
        if length<3{continue}let info=text[length..].trim();if mark=='`'&&info.contains('`'){continue}
        active=Some((mark,length,indent,info.split_whitespace().next()==Some("spec"),Vec::new()));
    }
    // 未闭合的流式围栏不提供回传资格。
    Ok(result)
}
fn pointer(path:&str)->Result<Vec<String>>{
    if path.is_empty(){return Ok(Vec::new())}ensure!(path.starts_with('/')&&path.len()<=512,"feedback.invalid_pointer");
    path[1..].split('/').map(|part|{
        let mut result=String::new();let mut chars=part.chars();while let Some(ch)=chars.next(){if ch=='~'{result.push(match chars.next(){Some('0')=>'~',Some('1')=>'/',_=>anyhow::bail!("feedback.invalid_pointer")})}else{result.push(ch)}}
        ensure!(!matches!(result.as_str(),"__proto__"|"constructor"|"prototype"),"feedback.field_key_forbidden");Ok(result)
    }).collect()
}
fn node<'a>(value:&'a Value,path:&[String])->Result<&'a Value>{let mut current=value;for part in path{current=if let Some(rows)=current.as_array(){rows.get(part.parse::<usize>()?).context("feedback.patch_path")?}else{current.get(part).context("feedback.patch_path")?}}Ok(current)}
fn parent<'a>(value:&'a mut Value,path:&[String])->Result<&'a mut Value>{let mut current=value;for part in path{current=if let Some(rows)=current.as_array_mut(){rows.get_mut(part.parse::<usize>()?).context("feedback.patch_path")?}else{current.get_mut(part).context("feedback.patch_path")?}}Ok(current)}
fn add(value:&mut Value,path:&[String],item:Value,replace:bool)->Result<()>{
    let Some((key,path))=path.split_last()else{*value=item;return Ok(())};let target=parent(value,path)?;
    if let Some(rows)=target.as_array_mut(){let index=if key=="-"&&!replace{rows.len()}else{key.parse::<usize>()?};if replace{*rows.get_mut(index).context("feedback.patch_path")?=item}else{ensure!(index<=rows.len(),"feedback.patch_path");rows.insert(index,item)}}
    else{let object=target.as_object_mut().context("feedback.patch_path")?;ensure!(!replace||object.contains_key(key),"feedback.patch_path");object.insert(key.clone(),item);}Ok(())
}
fn remove(value:&mut Value,path:&[String])->Result<Value>{let Some((key,path))=path.split_last()else{return Ok(std::mem::replace(value,Value::Null))};let target=parent(value,path)?;if let Some(rows)=target.as_array_mut(){let index=key.parse::<usize>()?;ensure!(index<rows.len(),"feedback.patch_path");Ok(rows.remove(index))}else{target.as_object_mut().context("feedback.patch_path")?.remove(key).context("feedback.patch_path")}}
pub fn compile(source:&str)->Result<Value>{
    let mut spec=json!({"root":"","elements":{}});let mut patches=0;
    for line in source.lines().filter(|line|!line.trim().is_empty()){
        let Ok(patch)=serde_json::from_str::<Value>(line)else{continue};let Some(op)=patch["op"].as_str()else{continue};let Some(raw_path)=patch["path"].as_str()else{continue};
        // 与原 Codeg 的防护一致：到达 __proto__ 的整条 patch 跳过。
        if raw_path.contains("__proto__")||patch["from"].as_str().is_some_and(|from|from.contains("__proto__")){continue}
        let path=pointer(raw_path)?;patches+=1;ensure!(patches<=4096,"feedback.card_patch_limit");
        match op{
            "add"=>add(&mut spec,&path,patch.get("value").context("feedback.patch_value")?.clone(),false)?,
            "replace"=>add(&mut spec,&path,patch.get("value").context("feedback.patch_value")?.clone(),true)?,
            "remove"=>{remove(&mut spec,&path)?;},
            "copy"|"move"=>{let from=pointer(patch["from"].as_str().context("feedback.patch_path")?)?;ensure!(op!="move"||!path.starts_with(&from)||path==from,"feedback.patch_cycle");let item=if op=="move"{remove(&mut spec,&from)?}else{node(&spec,&from)?.clone()};add(&mut spec,&path,item,false)?},
            "test"=>ensure!(node(&spec,&path)?==patch.get("value").context("feedback.patch_value")?,"feedback.patch_test"),
            _=>anyhow::bail!("feedback.patch_operation")
        }
        ensure!(serde_json::to_vec(&spec)?.len()<=1024*1024,"feedback.card_spec_limit");
    }
    ensure!(patches>0&&spec["root"].is_string()&&spec["elements"].is_object(),"feedback.card_spec_unavailable");Ok(spec)
}
#[derive(Clone)]struct FieldTemplate{name:String,label:Option<String>,path:Vec<String>}
fn templates(spec:&Value)->Result<Vec<FieldTemplate>>{
    fn visit(spec:&Value,key:&str,repeat:&[String],active:&mut BTreeSet<String>,count:&mut usize,result:&mut Vec<FieldTemplate>)->Result<()>{
        *count+=1;ensure!(*count<=4096&&active.len()<64&&active.insert(key.into()),"feedback.card_cycle");
        let element=spec["elements"].get(key).context("feedback.card_element")?;let props=&element["props"];
        if matches!(element["type"].as_str(),Some("Input"|"Textarea"|"Select"|"Checkbox"|"Radio"|"Switch"))&&props["type"].as_str()!=Some("password")&&(!props.get("type").is_some_and(Value::is_object)){
            let binding=if matches!(element["type"].as_str(),Some("Checkbox"|"Switch")){&props["checked"]}else{&props["value"]};
            let path=if let Some(path)=binding["$bindState"].as_str(){Some(pointer(path)?)}else if let Some(path)=binding["$bindItem"].as_str(){if repeat.is_empty(){None}else{let mut full=repeat.to_vec();if !path.is_empty(){full.extend(pointer(&format!("/{path}"))?)}Some(full)}}else{None};
            if let(Some(path),Some(name))=(path,props["name"].as_str()){if !path.is_empty(){result.push(FieldTemplate{name:name.into(),label:props["label"].as_str().map(str::to_string),path})}}
        }
        let mut next_repeat=repeat.to_vec();
        if let Some(repetition)=element.get("repeat"){
            let path=&repetition["statePath"];
            next_repeat=if let Some(path)=path.as_str(){pointer(path)?}else if let Some(path)=path["$item"].as_str(){ensure!(!repeat.is_empty(),"feedback.card_repeat_scope");let mut full=repeat.to_vec();if !path.is_empty(){full.extend(pointer(&format!("/{path}"))?)}full}else{anyhow::bail!("feedback.card_repeat_path")};next_repeat.push("*".into());
        }
        let mut children=element["children"].as_array().cloned().unwrap_or_default();if let Some(slots)=element["slots"].as_object(){for slot in slots.values(){if let Some(items)=slot.as_array(){children.extend(items.iter().cloned())}}}
        for child in children{if let Some(key)=child.as_str(){visit(spec,key,&next_repeat,active,count,result)?}}
        active.remove(key);Ok(())
    }
    let mut result=Vec::new();visit(spec,spec["root"].as_str().context("feedback.card_root")?,&[],&mut BTreeSet::new(),&mut 0,&mut result)?;Ok(result)
}
pub fn require_card_fields(connection:&Connection,input:&PreviewRequest,origin:&CardOrigin)->Result<()>{
    ensure!(origin.conversation_id==input.scope.conversation_id&&origin.message_kind==input.scope.scope_kind,"scope_changed");
    let body=source_body(connection,&input.scope,&input.agent_id,&origin.message_id)?;let sources=fences(&body)?;let raw=sources.get(origin.card_ordinal as usize).context("feedback.card_source_changed")?;
    // Markdown 围栏的单个结尾换行可能由原 pre 组件移除；只接受这两个等价呈现入口。
    let source=if digest(&Value::String(raw.clone()))?==origin.source_digest{raw.as_str()}else{let without=raw.strip_suffix('\n').unwrap_or(raw);ensure!(digest(&Value::String(without.into()))?==origin.source_digest,"feedback.card_source_changed");without};
    let spec=compile(source)?;let fields=templates(&spec)?;
    for field in &input.fields{let actual=pointer(&field.state_path)?;ensure!(fields.iter().any(|candidate|candidate.name==field.name&&candidate.label.as_ref().is_none_or(|label|label==&field.label)&&candidate.path.len()==actual.len()&&candidate.path.iter().zip(&actual).all(|(pattern,part)|pattern==part||pattern=="*"&&part.parse::<usize>().is_ok_and(|index|index<32))),"feedback.field_source_changed")}
    Ok(())
}
