//! 行为设置只有逐项明确选择才进入原策略；使用原修订与更新原语，不触碰既有 Run。
use super::*;
use rusqlite::{Connection,Transaction};

pub(crate) fn owns(target:&str)->bool{target=="delegationPolicy"}
pub(crate) fn patch(entry:&Value)->Result<Option<(&'static str,Value)>>{
    let key=entry["key"].as_str().context("configuration.preference_key_required")?;
    let raw=entry["value"].as_str().context("configuration.preference_value_required")?;
    let field=match key{
        "delegation.enabled"=>"enabled","delegation.depth_limit"=>"depthLimit",
        "delegation.agent_defaults"=>"perRuntimeDefaults","delegation.completed_cache_max_mb"=>"completedCacheMaxMb",
        _=>return Ok(None),
    };
    let value:Value=serde_json::from_str(raw).map_err(|_|anyhow::anyhow!("configuration.delegation_preference_invalid"))?;
    let candidate=json!({field:value});
    let typed:crate::delegation_settings::SettingsPatch=serde_json::from_value(candidate.clone()).map_err(|_|anyhow::anyhow!("configuration.delegation_preference_invalid"))?;
    ensure!(typed.depth_limit.is_none_or(|depth|(1..=8).contains(&depth))&&typed.completed_cache_max_mb.is_none_or(|budget|(0..=9007199254740991).contains(&budget)),"configuration.delegation_preference_out_of_range");
    bounded(&candidate,0)?;Ok(Some(("delegationPolicy",candidate)))
}
pub(crate) fn basis(connection:&Connection,target:&str)->Result<Value>{
    ensure!(owns(target),"configuration.policy_target_invalid");
    let value=crate::delegation_settings::read(connection)?;
    Ok(json!({"version":value.version,"value":value}))
}
pub(crate) fn merge(original:&Value,patch:&Value)->Result<Value>{
    let mut value=original.clone();let object=value.as_object_mut().context("configuration.policy_invalid")?;
    for(key,field)in patch.as_object().context("configuration.policy_patch_invalid")?{ensure!(key!="version","configuration.policy_version_immutable");object.insert(key.clone(),field.clone());}
    let _:crate::delegation_settings::DelegationSettings=serde_json::from_value(value.clone())?;Ok(value)
}
pub(crate) fn apply(tx:&Transaction<'_>,target:&str,before:&Value,after:&Value)->Result<Value>{
    ensure!(owns(target)&&basis(tx,target)?==*before,"configuration.policy_changed");
    let mut patch=after.clone();patch.as_object_mut().context("configuration.policy_invalid")?.remove("version");
    let input=crate::delegation_settings::UpdateSettingsParams{expected_version:before["version"].as_i64().context("configuration.policy_version_missing")?,patch:serde_json::from_value(patch)?};
    crate::delegation_settings::update(tx,&input)?;basis(tx,target)
}
