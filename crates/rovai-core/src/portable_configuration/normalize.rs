use super::*;

pub(crate) struct ModelNormalization{pub value:Value,pub warnings:Vec<String>}

pub(crate) fn model(agent:&str,raw:Option<&str>)->Result<ModelNormalization>{
    let mut warnings=Vec::new();
    let value=match agent{
        "claude_code"=>{
            let input=match raw.filter(|raw|!raw.trim().is_empty()){
                Some(raw)=>{let value:Value=serde_json::from_str(raw).map_err(|_|anyhow::anyhow!("configuration.claude_model_json_invalid"))?;ensure!(value.is_object(),"configuration.claude_model_object_required");value},
                None=>json!({}),
            };
            let keys=["main","reasoning","haiku","sonnet","opus","fable","customOption","customOptionName","customOptionDescription"];
            let mut normalized=serde_json::Map::new();normalized.insert("kind".into(),json!("claude"));
            for key in keys{
                let value=match input.get(key){Some(Value::String(value))=>value.trim(),None|Some(Value::Null)=>"",_=>anyhow::bail!("configuration.claude_model_field_invalid")};
                normalized.insert(key.into(),json!(value));
            }
            for key in input.as_object().unwrap().keys(){if !keys.contains(&key.as_str()){warnings.push(format!("未应用原模型字段 {key}，原文保留"));}}
            Value::Object(normalized)
        },
        "codex"=>{
            let parsed=codex_catalog::parse_model_config(raw);
            if let Some(raw)=raw.filter(|raw|!raw.trim().is_empty()){
                if let Ok(input)=serde_json::from_str::<Value>(raw){
                    if let Some(map)=input.as_object(){
                        for key in map.keys(){if !["customs","models","excludedOfficials","default"].contains(&key.as_str()){warnings.push(format!("未应用原模型字段 {key}，原文保留"));}}
                        if let Some(rows)=input.get("customs").or_else(||input.get("models")).and_then(Value::as_array){
                            if rows.len()!=parsed.customs.len(){warnings.push("原自定义模型含无法解析的条目，确认前须处理；原文保留".into());}
                        }
                    }
                }
            }
            let customs=parsed.customs.iter().map(|item|json!({"slug":item.slug,"displayName":item.display_name.as_deref().unwrap_or(""),"contextWindow":item.context_window,"base":item.base,"overrides":item.overrides})).collect::<Vec<_>>();
            json!({"kind":"codex","customs":customs,"excludedOfficials":parsed.excluded_officials,"default":parsed.default})
        },
        "gemini"=>json!({"kind":"gemini","model":raw.map(str::trim).filter(|raw|!raw.is_empty())}),
        _=>anyhow::bail!("configuration.agent_type_unsupported"),
    };
    super::model::validate_model(agent,&value)?;Ok(ModelNormalization{value,warnings})
}

pub(crate) struct ProviderNormalization{pub definition:Value,pub credential:Option<String>,pub warnings:Vec<String>}
pub(crate) fn provider(id:&str,entry:&Value)->Result<ProviderNormalization>{
    uuid::Uuid::parse_str(id)?;
    let source:archive::CodegProvider=serde_json::from_value(entry.clone()).map_err(|_|anyhow::anyhow!("configuration.provider_row_invalid"))?;
    let model=model(&source.agent_type,source.model.as_deref())?;
    super::model::validate_url(&source.api_url)?;
    let definition=json!({"schemaVersion":1,"id":id,"name":source.name,"agentType":source.agent_type,"apiUrl":source.api_url,"credential":null,"rawAgentTypesJson":source.agent_types_json,"rawModel":source.model,"model":model.value,"enabled":entry["enabled"].as_bool().unwrap_or(true)&&entry["queryRequiresSecret"]!=true});
    validate("ProviderDefinitionV1",&definition)?;
    Ok(ProviderNormalization{definition,credential:(!source.api_key.is_empty()).then_some(source.api_key),warnings:model.warnings})
}

/// 实际原生目录展开后明确报告被原算法拒绝的覆盖，不能将丢失配置说成完整应用。
pub(crate) fn codex_projection(model:&Value,snapshot:&[Value])->Result<(Value,Vec<String>)>{
    ensure!(model["kind"]=="codex","configuration.model_kind_mismatch");
    let config:codex_catalog::CodexModelConfig=serde_json::from_value(json!({"customs":model["customs"],"excludedOfficials":model["excludedOfficials"],"default":model["default"]})).map_err(|_|anyhow::anyhow!("configuration.codex_model_invalid"))?;
    let projected=codex_catalog::expand_to_catalog(&config,snapshot);let mut warnings=Vec::new();
    for custom in &config.customs{
        let observed=projected["models"].as_array().and_then(|models|models.iter().find(|value|value["slug"]==custom.slug));
        if let Some(observed)=observed{for(key,value)in &custom.overrides{if observed.get(key)!=Some(value){warnings.push(format!("模型 {} 的覆盖字段 {key} 未应用；原文保留",custom.slug));}}}
        else{warnings.push(format!("模型 {} 没有获得可用目录条目",custom.slug));}
    }
    Ok((projected,warnings))
}

pub(crate) fn custom_agent(id:&str,entry:&Value)->Result<Value>{
    if entry["fusionDefinition"].is_object(){
        let mut definition=entry["fusionDefinition"].clone();definition["id"]=json!(id);
        crate::custom_runtime::validate_definition(&definition,true)?;return Ok(definition)
    }
    let source:archive::CodegCustomAgent=serde_json::from_value(entry.clone()).map_err(|_|anyhow::anyhow!("configuration.custom_agent_invalid"))?;
    let mut spec:Value=serde_json::from_str(&source.spec_json).map_err(|_|anyhow::anyhow!("configuration.custom_spec_invalid"))?;
    let object=spec.as_object_mut().context("configuration.custom_spec_invalid")?;
    for key in ["npx","uvx"]{object.entry(key.to_owned()).or_insert(Value::Null);}
    object.entry("binary".to_owned()).or_insert_with(||json!({}));
    for key in ["npx","uvx"]{
        if let Some(block)=spec[key].as_object_mut(){
            block.entry("args".to_owned()).or_insert_with(||json!([]));block.entry("env".to_owned()).or_insert_with(||json!({}));block.entry("cmd".to_owned()).or_insert(Value::Null);
            for optional in if key=="npx"{vec!["nodeRequired"]}else{vec!["uvRequired","python"]}{block.entry(optional.to_owned()).or_insert(Value::Null);}
        }
    }
    if let Some(platforms)=spec["binary"].as_object_mut(){for block in platforms.values_mut(){if let Some(block)=block.as_object_mut(){for(key,value)in[("args",json!([])),("env",json!({})),("sha256",Value::Null)]{block.entry(key.to_owned()).or_insert(value);}}}}
    let mut definition=crate::custom_runtime::default_definition();
    for(key,value)in[("id",json!(id)),("registryId",json!(source.registry_id)),("name",json!(source.name)),("description",json!(source.description)),("version",json!(source.version)),("distributionKind",json!(source.distribution_kind)),("spec",spec),("iconUrl",json!(source.icon_url)),("skillsSharedStore",json!(source.skills_shared_store)),("source",json!(source.source)),("versionProbe",json!(source.version_probe)),("supportsMcp",json!(source.supports_mcp))]{definition[key]=value;}
    crate::custom_runtime::validate_definition(&definition,true)?;Ok(definition)
}
