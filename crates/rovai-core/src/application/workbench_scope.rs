//! Engineering paths are resolved from current Mission ownership even for older clients.
use super::*;
use anyhow::ensure;
use rusqlite::OptionalExtension;

impl Core {
    pub(super) async fn workbench_directory(&self, scope: &workbench::Scope) -> Result<String> {
        let (project, mission, workspace, has_workspace) = {
            let database = self.database.lock().await;
            let (project, mission): (String, Option<String>) = database.connection().query_row(
                "SELECT c.project_path,m.id FROM camp c LEFT JOIN mission m ON m.camp_id=c.id
                 WHERE c.id=?1 AND c.deletion_operation_id IS NULL",
                [&scope.thread_id], |row| Ok((row.get(0)?,row.get(1)?)),
            ).context("workbench.thread_unavailable")?;
            ensure!(scope.mission_id.is_none() || scope.mission_id == mission, "workbench.mission_owner_changed");
            let (workspace, has_workspace) = if let Some(id) = &mission {
                let workspace: Option<(String,String)> = database.connection().query_row(
                    "SELECT working_directory,state FROM mission_workspace
                     WHERE mission_id=?1 AND camp_id=?2 AND execution_host_id=(SELECT id FROM mission_execution_host WHERE singleton=1)",
                    rusqlite::params![id,scope.thread_id], |row| Ok((row.get(0)?,row.get(1)?)),
                ).optional()?;
                let has_workspace: bool = database.connection().query_row(
                    "SELECT EXISTS(SELECT 1 FROM mission_workspace WHERE mission_id=?1)",
                    [id], |row| row.get(0))?;
                (workspace,has_workspace)
            } else { (None,false) };
            (project,mission,workspace,has_workspace)
        };
        if mission.is_some() {
            if let Some((directory,state)) = workspace {
                ensure!(state == "ready", "workbench.mission_unavailable");
                return Ok(directory);
            }
            // Non-Git Missions use the original cwd and do not require a fabricated worktree.
            ensure!(!has_workspace && !mission_workspace::has_git_marker(Path::new(&project)),
                "workbench.mission_unavailable");
        }
        Ok(project)
    }
}
