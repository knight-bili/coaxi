//! 只映射源应用可迁移的明确枚举；账号、代理、终端和本机目录没有普通偏好入口。
use super::*;

pub(crate) fn file_patch(entry:&Value)->Result<(&'static str,Value)>{
    let key=entry["key"].as_str().context("configuration.preference_key_required")?;
    let raw=entry["value"].as_str().context("configuration.preference_value_required")?;
    match key{
        "appearance_mode"=>{
            let mode=match raw{"system"=>"system","light"=>"day","dark"=>"night",_=>anyhow::bail!("configuration.theme_invalid")};
            Ok(("appearance",json!({"themePreference":mode})))
        },
        "appearance_zoom_level"=>{
            let zoom=raw.parse::<u32>().map_err(|_|anyhow::anyhow!("configuration.zoom_invalid"))?;
            ensure!((50..=300).contains(&zoom),"configuration.zoom_invalid");Ok(("appearance",json!({"zoomPercentage":zoom})))
        },
        "system_language_settings"=>{
            let value:Value=serde_json::from_str(raw).map_err(|_|anyhow::anyhow!("configuration.language_invalid"))?;
            let mode=value["mode"].as_str().unwrap_or("system");ensure!(matches!(mode,"system"|"manual"),"configuration.language_mode_invalid");
            let language=match value["language"].as_str().unwrap_or("en"){"zh_cn"=>"zh-CN","zh_tw"=>"zh-TW",language@("en"|"ja"|"ko"|"es"|"de"|"fr"|"pt"|"ar")=>language,_=>anyhow::bail!("configuration.language_invalid")};
            Ok(("general",json!({"interfaceLanguage":language,"interfaceLanguageMode":mode})))
        },
        "system_close_behavior_settings"=>{
            let value:Value=serde_json::from_str(raw).map_err(|_|anyhow::anyhow!("configuration.close_behavior_invalid"))?;
            let behavior=match value["behavior"].as_str().unwrap_or("ask"){"ask"=>"ask","minimize"=>"tray","exit"=>"exit",_=>anyhow::bail!("configuration.close_behavior_invalid")};
            Ok(("windowClose",json!({"behavior":behavior})))
        },
        _=>anyhow::bail!("configuration.preference_needs_explicit_policy_mapping"),
    }
}
pub(crate) fn before_fields(original:&Value,patch:&Value)->Value{
    Value::Object(patch.as_object().into_iter().flat_map(|map|map.keys()).map(|key|(key.clone(),original.get(key).cloned().unwrap_or(Value::Null))).collect())
}
pub(crate) fn merge(target:&str,original:&Value,patch:&Value)->Result<Value>{
    let mut next=if original.is_null(){match target{
        "appearance"=>json!({"schemaVersion":2,"themePreference":"system","chatFontSize":13,"documentFontSize":15,"codeFontSize":14,"readingDensity":"standard","motionPreference":"system","zoomPercentage":100}),
        "general"=>json!({"schemaVersion":6}),"windowClose"=>json!({"schemaVersion":1,"behavior":"ask"}),_=>anyhow::bail!("configuration.preference_target_invalid"),
    }}else{original.clone()};
    let object=next.as_object_mut().context("configuration.preference_file_invalid")?;
    for(key,value)in patch.as_object().context("configuration.preference_patch_invalid")?{object.insert(key.clone(),value.clone());}
    if target=="general"{crate::backup::light_config::complete_general_preferences(object,patch)?;}
    Ok(next)
}
