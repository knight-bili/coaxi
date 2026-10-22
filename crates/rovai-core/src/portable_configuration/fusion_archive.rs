//! 旧融合偏好格式1与完整格式2沿同一逐条选择入口；源定位仍指向原域、原序号与键。
use super::*;

pub(crate) fn entries(source:&archive::ParsedArchive)->Result<Vec<archive::Entry>>{
    let mut result=Vec::new();
    for(domain,value)in &source.domains{
        if matches!(domain.as_str(),"appearance"|"general"|"windowClose"){
            let map=value.as_object().context("configuration.fusion_preference_invalid")?;
            for(index,(key,value))in map.iter().enumerate(){
                push(&mut result,source,domain,index,Some(key),"preferences",json!({"key":format!("rovai.{domain}.{key}"),"value":serde_json::to_string(value)?}))?;
            }
        }else if domain=="messageLibrary"{
            let map=value.as_object().context("configuration.fusion_library_invalid")?;
            for(index,(key,value))in map.iter().enumerate(){
                if key=="quickMessages"{
                    let rows=value.as_array().context("configuration.fusion_messages_invalid")?;
                    for(row_index,row)in rows.iter().enumerate(){
                        push(&mut result,source,domain,row_index,Some("quickMessages"),"quickMessages",json!({"title":row["title"],"content":row["text"],"sortOrder":row_index}))?;
                    }
                }else{
                    push(&mut result,source,domain,index,Some(key),"preferences",json!({"key":format!("rovai.messageLibrary.{key}"),"value":serde_json::to_string(value)?}))?;
                }
            }
        }else{
            append_rows(&mut result,source,domain,value)?;
        }
    }
    ensure!(result.len()<=8192,"configuration.too_many_entries");Ok(result)
}
fn push(result:&mut Vec<archive::Entry>,source:&archive::ParsedArchive,source_domain:&str,index:usize,key:Option<&str>,domain:&str,value:Value)->Result<()>{
    result.push(archive::Entry{id:digest(&json!({"source":source.source_digest,"domain":source_domain,"index":index,"key":key}))?,domain:domain.into(),index,value});Ok(())
}
fn append_rows(result:&mut Vec<archive::Entry>,source:&archive::ParsedArchive,domain:&str,value:&Value)->Result<()>{
    if let Some(rows)=value.as_array(){
        let mut indexed=rows.iter().enumerate().collect::<Vec<_>>();
        if matches!(domain,"quickMessages"|"agentSettings"){indexed.sort_by_key(|(index,value)|(value["sortOrder"].as_i64().unwrap_or(0),*index));}
        for(index,value)in indexed{push(result,source,domain,index,None,domain,value.clone())?;}
    }else if let Some(map)=value.as_object(){
        for(index,(key,value))in map.iter().enumerate(){push(result,source,domain,index,Some(key),domain,json!({"key":key,"value":value}))?;}
    }
    Ok(())
}
