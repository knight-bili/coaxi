use super::*;
use anyhow::ensure;
use serde::Deserialize;
use std::fs;
impl Core {
    pub(super) async fn channel_adapter_host_status(&self, value: Value) -> Result<Value> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct State {
            adapter_id: String,
            state: String,
        }
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct Snapshot {
            states: Vec<State>,
            pending_inbox: usize,
            pending_callbacks: usize,
            rejected_inbox: usize,
        }
        let snapshot: Snapshot = serde_json::from_value(value)?;
        ensure!(
            snapshot.states.len() <= 1024,
            "channel_adapter.status_limit"
        );
        let adapters = self.read_channel_adapters()?;
        let mut states = Vec::new();
        for state in snapshot.states {
            ensure!(
                ["connected", "connecting", "disconnected", "error"]
                    .contains(&state.state.as_str()),
                "channel_adapter.invalid_status"
            );
            if adapters
                .iter()
                .any(|adapter| adapter.id == state.adapter_id)
            {
                states.push(json!({"adapterId":state.adapter_id,"state":state.state}));
            }
        }
        let value = json!({"updatedAt":chrono::Utc::now().timestamp(),"states":states,"pendingInbox":snapshot.pending_inbox,"pendingCallbacks":snapshot.pending_callbacks,"rejectedInbox":snapshot.rejected_inbox});
        crate::platform::private_storage::atomic_write_private_json(
            &self.data_dir.join("channel-adapter-host.json"),
            &value,
        )?;
        Ok(json!({"recorded":true}))
    }
    pub(super) fn channel_adapter_public_status(&self) -> Value {
        let path = self.data_dir.join("channel-adapter-host.json");
        let Ok(bytes) = fs::read(path) else {
            return json!({"states":[],"fresh":false});
        };
        if bytes.len() > 256 * 1024 {
            return json!({"states":[],"fresh":false});
        }
        let Ok(mut value) = serde_json::from_slice::<Value>(&bytes) else {
            return json!({"states":[],"fresh":false});
        };
        let age = chrono::Utc::now().timestamp() - value["updatedAt"].as_i64().unwrap_or(0);
        let fresh = (0..=15).contains(&age);
        value["fresh"] = json!(fresh);
        if !fresh {
            if let Some(states) = value["states"].as_array_mut() {
                for state in states {
                    state["state"] = json!("unknown");
                }
            }
        }
        value
    }
}
