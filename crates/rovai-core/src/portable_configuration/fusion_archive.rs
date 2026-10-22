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
        for(index,value)in indexed{push(result,source,domain,index,None,domain,hydrate(source,domain,value)?)?;}
    }else if let Some(map)=value.as_object(){
        for(index,(key,value))in map.iter().enumerate(){push(result,source,domain,index,Some(key),domain,json!({"key":key,"value":value}))?;}
    }
    Ok(())
}

fn secret(source:&archive::ParsedArchive,id:&str)->Result<String>{
    ensure!(source.source_kind=="fusion_v2"&&source.original["secretsIncluded"]==true,"configuration.archive_secret_not_included");
    let rows=source.original["secrets"].as_array().context("configuration.archive_secrets_invalid")?;
    let mut found=rows.iter().filter(|row|row["archiveSecretId"]==id);let value=found.next().context("configuration.archive_secret_missing")?;
    ensure!(found.next().is_none(),"configuration.archive_secret_ambiguous");Ok(value["value"].as_str().context("configuration.archive_secret_invalid")?.to_owned())
}
fn hydrate(source:&archive::ParsedArchive,domain:&str,original:&Value)->Result<Value>{
    if source.source_kind!="fusion_v2"{return Ok(original.clone())}
    let mut value=original.clone();
    if domain=="modelProviders"{
        if let Some(id)=value["archiveCredentialId"].as_str(){value["apiKey"]=json!(secret(source,id)?);}
        else{ensure!(value["apiKey"].as_str().is_none_or(str::is_empty)||source.original["secretsIncluded"]==true,"configuration.unexpected_archive_credential");}
    }else if domain=="agentSettings"{
        if let Some(rows)=value["archiveEnvironment"].as_array(){
            let mut environment=serde_json::Map::new();for row in rows{let name=row["name"].as_str().context("configuration.environment_name_required")?;ensure!(!environment.contains_key(name),"configuration.duplicate_environment_name");environment.insert(name.into(),json!(secret(source,row["archiveSecretId"].as_str().context("configuration.archive_secret_id_required")?)?));}
            value["envJson"]=if environment.is_empty(){Value::Null}else{json!(serde_json::to_string(&environment)?)};
        }
    }else if domain=="customAgents"&&value["fusionDefinition"].is_object(){
        let paths=crate::custom_runtime::environments(&value["fusionDefinition"]);let mut definition=value["fusionDefinition"].clone();
        for row in value["archiveEnvironment"].as_array().into_iter().flatten(){
            let path=row["path"].as_str().context("configuration.custom_environment_path_required")?;ensure!(paths.iter().any(|allowed|allowed==path),"configuration.custom_environment_path_invalid");
            let name=row["name"].as_str().context("configuration.environment_name_required")?;let plain=secret(source,row["archiveSecretId"].as_str().context("configuration.archive_secret_id_required")?)?;
            let fields=definition.pointer_mut(path).and_then(Value::as_object_mut).context("configuration.custom_environment_invalid")?;ensure!(fields.insert(name.into(),json!(plain)).is_none(),"configuration.duplicate_environment_name");
        }
        value["specJson"]=json!(serde_json::to_string(&definition["spec"])?);value["fusionDefinition"]=definition;
    }
    Ok(value)
}
