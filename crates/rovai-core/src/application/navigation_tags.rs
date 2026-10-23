//! Thread tag assignments reuse the organizer's existing Core-owned scope records.
use super::*;
use std::{collections::BTreeMap, fs};

fn thread_tags(directory: &Path, thread_id: &str, project_path: &str,
    cache: &mut BTreeMap<PathBuf, Vec<String>>) -> Result<Vec<String>> {
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
    for root in roots {
        let identity = crate::command::canonical_json_digest(&json!({
            "scope": {"threadId":thread_id,"missionId":null}, "root": PathBuf::from(root)
        }))?;
        let path = directory.join(format!("{identity}.json"));
        if let Some(tags) = cache.get(&path) { return Ok(tags.clone()); }
        let tags = match fs::read(&path) {
        Ok(bytes) => {
            let value: Value = serde_json::from_slice(&bytes)?;
            value.get("tags").and_then(Value::as_array).context("workbench.invalid_tags")?
                .iter().map(|tag| tag.as_str().map(str::to_owned).context("workbench.invalid_tag"))
                .collect::<Result<Vec<_>>>()?
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
        Err(error) => return Err(error.into())
    };
        cache.insert(path, tags.clone());
        return Ok(tags);
    }
    Ok(Vec::new())
}

fn project(directory: &Path, value: &mut Value, cache: &mut BTreeMap<PathBuf, Vec<String>>) -> Result<()> {
    match value {
        Value::Array(items) => for item in items { project(directory, item, cache)?; },
        Value::Object(object) => {
            if let (Some(id), Some(path)) = (object.get("id").and_then(Value::as_str),
                object.get("projectPath").and_then(Value::as_str)) {
                let tags = thread_tags(directory, id, path, cache)?;
                object.insert("tags".into(), json!(tags));
            }
            for key in ["quickChat", "projects", "recentThreads", "recentCamps", "threads", "camps", "thread", "camp"] {
                if let Some(child) = object.get_mut(key) { project(directory, child, cache)?; }
            }
        },
        _ => {}
    }
    Ok(())
}

impl Core {
    pub(super) async fn navigation_with_tags(&self, mut value: Value) -> Result<Value> {
        let directory = self.data_dir.join("workspace-metadata");
        tokio::task::spawn_blocking(move || {
            project(&directory, &mut value, &mut BTreeMap::new())?;
            Ok(value)
        }).await?
    }
}
