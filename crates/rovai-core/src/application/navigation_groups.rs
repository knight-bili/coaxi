//! Shared presentation identities only. Paths here never register projects or grant filesystem access.
use super::*;
use anyhow::ensure;
use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
struct Group { id:String, name:String, color:Option<String>, project_paths:Vec<String> }
#[derive(Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
struct Layout { project_groups:Vec<Group>, project_order:Option<Vec<String>> }
#[derive(Serialize, Deserialize)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
struct Stored { schema_version:u32, project_groups_revision:u64, layout:Layout }
#[derive(Deserialize)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
struct Request { action:String, expected_revision:Option<u64>, value:Option<Layout> }

fn project_path(path:&str)->bool {
    let bytes=path.as_bytes();
    !path.is_empty() && path.encode_utf16().count()<=8192 && !path.contains('\0')
        && (path.starts_with('/') || path.starts_with("\\\\") || bytes.len()>=3 && bytes[0].is_ascii_alphabetic() && bytes[1]==b':' && matches!(bytes[2],b'/'|b'\\'))
        && !path.split(['/', '\\']).any(|part|matches!(part,"."|".."))
}
fn validate(layout:&Layout)->Result<()> {
    let mut ids=HashSet::new();let mut paths=HashSet::new();
    for group in &layout.project_groups {
        ensure!(group.id.len()==36 && uuid::Uuid::parse_str(&group.id).is_ok() && ids.insert(group.id.clone()),"navigation.invalid_group_identity");
        let normalized=group.name.split_whitespace().collect::<Vec<_>>().join(" ");
        ensure!(!normalized.is_empty() && normalized.chars().count()<=80 && normalized==group.name,"navigation.invalid_group_name");
        crate::canvas_board::color(group.color.as_deref()).map_err(|_|anyhow::anyhow!("navigation.invalid_group_color"))?;
        for path in &group.project_paths {ensure!(project_path(path) && paths.insert(path.clone()),"navigation.invalid_group_path");}
    }
    if let Some(order)=&layout.project_order {
        let mut seen=HashSet::new();
        for key in order {
            let valid=if let Some(id)=key.strip_prefix("folder-group:"){ids.contains(&id.to_string())}
                else if let Some(path)=key.strip_prefix("directory:"){project_path(path) && !paths.contains(&path.to_string())}else{false};
            ensure!(valid && seen.insert(key),"navigation.invalid_group_order");
        }
    }
    Ok(())
}
fn view(stored:Option<&Stored>)->Value {
    match stored {
        Some(value)=>json!({"schemaVersion":1,"initialized":true,"projectGroupsRevision":value.project_groups_revision,"projectGroups":value.layout.project_groups,"projectOrder":value.layout.project_order}),
        None=>json!({"schemaVersion":1,"initialized":false,"projectGroupsRevision":0,"projectGroups":[],"projectOrder":null})
    }
}
impl Core {
    pub(super) async fn navigation_project_groups(&self,value:Value)->Result<Value> {
        let request:Request=serde_json::from_value(value)?;
        ensure!(matches!(request.action.as_str(),"get"|"replace"),"navigation.invalid_group_action");
        let _lock=workbench::MUTATIONS.lock().await;
        let path=self.data_dir.join("navigation-project-groups-v1.json");
        let stored:Option<Stored>=match std::fs::read(&path) {
            Ok(bytes)=>{ensure!(bytes.len()<=32*1024*1024,"navigation.group_file_limit");let value:Stored=serde_json::from_slice(&bytes)?;ensure!(value.schema_version==1 && value.project_groups_revision>0 && value.project_groups_revision<=9_007_199_254_740_991,"navigation.invalid_group_storage");validate(&value.layout)?;Some(value)},
            Err(error) if error.kind()==std::io::ErrorKind::NotFound=>None,
            Err(error)=>return Err(error.into())
        };
        if request.action=="get" {ensure!(request.value.is_none() && request.expected_revision.is_none(),"navigation.invalid_group_read");return Ok(view(stored.as_ref()))}
        let next=request.value.context("navigation.group_value_required")?;validate(&next)?;
        let expected=request.expected_revision.context("navigation.group_revision_required")?;
        let revision=stored.as_ref().map_or(0,|value|value.project_groups_revision);
        // An immediately repeated full-value CAS can reconcile a lost response without incrementing twice.
        if stored.as_ref().is_some_and(|value|value.layout==next) && (revision==expected || expected.checked_add(1)==Some(revision)){return Ok(view(stored.as_ref()))}
        ensure!(revision==expected && revision<9_007_199_254_740_991,"navigation.project_groups_changed");
        let next=Stored{schema_version:1,project_groups_revision:revision+1,layout:next};
        crate::platform::private_storage::atomic_write_private_json(&path,&next)?;
        emit(&self.output,"navigation.projectGroups.changed",json!({"revision":next.project_groups_revision}));
        Ok(view(Some(&next)))
    }
}
