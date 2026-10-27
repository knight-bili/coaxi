//! Instance-wide creation preferences. The caller holds the Core database lock
//! across read/modify/atomic publication, so Desktop and Web share one order.
use super::*;
use serde::Serialize;
use std::io::Read;

#[derive(Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Snapshot {
    new_conversation_defaults: Option<Team>,
    new_conversation_defaults_require_confirmation: bool,
    one_click_new_conversation_enabled: bool,
}
#[derive(Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Team {
    member_agent_ids: Vec<String>,
    default_lead_agent_id: String,
}
impl Team {
    fn validate(&self) -> Result<()> {
        let ids = &self.member_agent_ids;
        anyhow::ensure!(
            !ids.is_empty() && ids.len() <= 100,
            "Default team is invalid"
        );
        anyhow::ensure!(
            ids.iter()
                .all(|id| !id.is_empty() && id.chars().count() <= 200)
                && ids.iter().collect::<HashSet<_>>().len() == ids.len()
                && ids.contains(&self.default_lead_agent_id),
            "Default team or Lead is invalid"
        );
        Ok(())
    }
    fn exists(&self, database: &Database) -> Result<bool> {
        for id in &self.member_agent_ids {
            if AgentProfileService::default()
                .get_profile(database, id)?
                .is_none()
            {
                return Ok(false);
            }
        }
        Ok(true)
    }
}
impl Snapshot {
    fn validate(&self) -> Result<()> {
        if let Some(team) = &self.new_conversation_defaults {
            team.validate()?;
        } else {
            anyhow::ensure!(
                !self.new_conversation_defaults_require_confirmation
                    && !self.one_click_new_conversation_enabled,
                "Default team is missing"
            );
        }
        Ok(())
    }
}

pub(super) fn execute(
    root: &Path,
    database: &Database,
    method: &str,
    params: Value,
) -> Result<(Value, bool)> {
    let path = root.join("new-conversation-preferences.json");
    let saved = match std::fs::File::open(&path) {
        Ok(file) => {
            let mut bytes = Vec::new();
            file.take(65_537).read_to_end(&mut bytes)?;
            anyhow::ensure!(
                bytes.len() <= 65_536,
                "Creation preferences exceed the size limit"
            );
            let saved: Snapshot = serde_json::from_slice(&bytes)?;
            saved.validate()?;
            Some(saved)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(error.into()),
    };
    let mut next = saved.clone().unwrap_or_default();
    match method {
        "preferences.newConversation.get" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Empty {}
            let _: Empty = serde_json::from_value(params)?;
            return Ok((serde_json::to_value(next)?, false));
        }
        "preferences.newConversation.initialize" => {
            let legacy: Snapshot = serde_json::from_value(params)?;
            legacy.validate()?;
            if saved.is_some() {
                return Ok((serde_json::to_value(next)?, false));
            }
            next = legacy;
        }
        "preferences.newConversation.setDefaults" => {
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase", deny_unknown_fields)]
            struct Set {
                defaults: Team,
                enable_one_click: bool,
            }
            let input: Set = serde_json::from_value(params)?;
            input.defaults.validate()?;
            anyhow::ensure!(
                input.defaults.exists(database)?,
                "Default team contains an unavailable member"
            );
            next.new_conversation_defaults = Some(input.defaults);
            next.new_conversation_defaults_require_confirmation = false;
            next.one_click_new_conversation_enabled |= input.enable_one_click;
        }
        "preferences.newConversation.setOneClick" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Set {
                enabled: bool,
            }
            let input: Set = serde_json::from_value(params)?;
            if input.enabled {
                anyhow::ensure!(
                    !next.new_conversation_defaults_require_confirmation,
                    "Default team requires confirmation"
                );
                let team = next
                    .new_conversation_defaults
                    .as_ref()
                    .context("Default team is missing")?;
                anyhow::ensure!(team.exists(database)?, "Default team requires confirmation");
            }
            next.one_click_new_conversation_enabled = input.enabled;
        }
        "preferences.newConversation.invalidate" => {
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase", deny_unknown_fields)]
            struct Invalidate {
                expected_defaults: Option<Team>,
            }
            let input: Invalidate = serde_json::from_value(params)?;
            // A stale client must not invalidate a newer saved team.
            if next.new_conversation_defaults == input.expected_defaults
                && next.new_conversation_defaults.is_some()
            {
                next.new_conversation_defaults_require_confirmation = true;
            }
        }
        _ => anyhow::bail!("Unsupported creation preference operation"),
    }
    next.validate()?;
    let changed = saved.as_ref() != Some(&next);
    if changed {
        crate::platform::private_storage::atomic_write_private_json(&path, &next)?;
    }
    Ok((serde_json::to_value(next)?, changed))
}
