//! Thread tag assignments reuse the organizer's existing Core-owned scope records.
use super::*;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
};

const CHILDREN: [&str; 9] = [
    "quickChat",
    "projects",
    "recentThreads",
    "recentCamps",
    "threads",
    "camps",
    "thread",
    "camp",
    "navigation",
];
type MissionRoots = BTreeMap<String, (String, Option<String>)>;

fn thread_ids(value: &Value, ids: &mut BTreeSet<String>) {
    match value {
        Value::Array(items) => {
            for item in items {
                thread_ids(item, ids);
            }
        }
        Value::Object(object) => {
            if object.get("projectPath").and_then(Value::as_str).is_some() {
                if let Some(id) = object.get("id").and_then(Value::as_str) {
                    ids.insert(id.to_owned());
                }
            }
            for key in CHILDREN {
                if let Some(child) = object.get(key) {
                    thread_ids(child, ids);
                }
            }
        }
        _ => {}
    }
}

fn thread_tags(
    directory: &Path,
    thread_id: &str,
    project_path: &str,
    mission_id: Option<&str>,
    cache: &mut BTreeMap<PathBuf, Vec<String>>,
) -> Result<Vec<String>> {
    let mut roots = vec![project_path.to_owned()];
    #[cfg(windows)]
    {
        // Match the same permitted Windows representation difference as workspace validation.
        if let Some(path) = project_path.strip_prefix(r"\\?\UNC\") {
            roots.push(format!(r"\\{path}"));
        } else if let Some(path) = project_path.strip_prefix(r"\\?\") {
            roots.push(path.to_owned());
        } else if let Some(path) = project_path.strip_prefix(r"\\") {
            roots.insert(0, format!(r"\\?\UNC\{path}"));
        } else if Path::new(project_path).is_absolute() {
            roots.insert(0, format!(r"\\?\{project_path}"));
        }
    }
    let scopes = if let Some(id) = mission_id {
        vec![Some(id), None]
    } else {
        vec![None]
    };
    for mission in scopes {
        for root in &roots {
            let identity = crate::command::canonical_json_digest(&json!({
                "scope": {"threadId":thread_id,"missionId":mission}, "root": PathBuf::from(root)
            }))?;
            let path = directory.join(format!("{identity}.json"));
            if let Some(tags) = cache.get(&path) {
                return Ok(tags.clone());
            }
            let tags = match fs::read(&path) {
                Ok(bytes) => {
                    let value: Value = serde_json::from_slice(&bytes)?;
                    value
                        .get("tags")
                        .and_then(Value::as_array)
                        .context("workbench.invalid_tags")?
                        .iter()
                        .map(|tag| {
                            tag.as_str()
                                .map(str::to_owned)
                                .context("workbench.invalid_tag")
                        })
                        .collect::<Result<Vec<_>>>()?
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => return Err(error.into()),
            };
            cache.insert(path, tags.clone());
            return Ok(tags);
        }
    }
    Ok(Vec::new())
}

fn project(
    directory: &Path,
    value: &mut Value,
    missions: &MissionRoots,
    cache: &mut BTreeMap<PathBuf, Vec<String>>,
) -> Result<()> {
    match value {
        Value::Array(items) => {
            for item in items {
                project(directory, item, missions, cache)?;
            }
        }
        Value::Object(object) => {
            if let (Some(id), Some(path)) = (
                object.get("id").and_then(Value::as_str),
                object.get("projectPath").and_then(Value::as_str),
            ) {
                let tags = match missions.get(id) {
                    Some((mission, Some(root))) => {
                        thread_tags(directory, id, root, Some(mission), cache)?
                    }
                    Some((_, None)) => Vec::new(),
                    None => thread_tags(directory, id, path, None, cache)?,
                };
                object.insert("tags".into(), json!(tags));
            }
            for key in CHILDREN {
                if let Some(child) = object.get_mut(key) {
                    project(directory, child, missions, cache)?;
                }
            }
        }
        _ => {}
    }
    Ok(())
}

impl Core {
    pub(super) async fn navigation_with_tags(&self, mut value: Value) -> Result<Value> {
        let directory = self.data_dir.join("workspace-metadata");
        let mut ids = BTreeSet::new();
        thread_ids(&value, &mut ids);
        let missions: MissionRoots = if ids.is_empty() {
            BTreeMap::new()
        } else {
            let database = self.database.lock().await;
            let mut query = database.connection().prepare(
                "SELECT m.camp_id,m.id,c.project_path,w.working_directory,w.state,
                 EXISTS(SELECT 1 FROM mission_workspace x WHERE x.mission_id=m.id)
                 FROM mission m JOIN camp c ON c.id=m.camp_id
                 LEFT JOIN mission_workspace w ON w.mission_id=m.id AND w.camp_id=m.camp_id
                 AND w.execution_host_id=(SELECT id FROM mission_execution_host WHERE singleton=1)
                 WHERE m.camp_id IN (SELECT value FROM json_each(?1))",
            )?;
            let rows = query
                .query_map([serde_json::to_string(&ids)?], |row| {
                    let original: String = row.get(2)?;
                    let directory: Option<String> = row.get(3)?;
                    let state: Option<String> = row.get(4)?;
                    let exists: bool = row.get(5)?;
                    let root = if state.as_deref() == Some("ready") {
                        directory
                    } else if !exists {
                        Some(original)
                    } else {
                        None
                    };
                    Ok((row.get::<_, String>(0)?, (row.get::<_, String>(1)?, root)))
                })?
                .collect::<rusqlite::Result<BTreeMap<_, _>>>()?;
            rows
        };
        tokio::task::spawn_blocking(move || {
            project(&directory, &mut value, &missions, &mut BTreeMap::new())?;
            Ok(value)
        })
        .await?
    }
}
