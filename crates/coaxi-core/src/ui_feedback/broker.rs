use super::*;
use chrono::{DateTime, Utc};
use std::{collections::HashMap, sync::Mutex};

#[derive(Clone)]
struct Lease {
    binding: Binding,
    token: String,
    revoked: bool,
    discovered: bool,
    successful_call: bool,
}
#[derive(Default)]
pub struct Registry {
    leases: Mutex<HashMap<String, Lease>>,
}
impl Registry {
    pub fn register(&self, binding: Binding) -> Result<String> {
        validate_shape("binding", &serde_json::to_value(&binding)?)?;
        let mut leases = self
            .leases
            .lock()
            .map_err(|_| anyhow::anyhow!("feedback.binding_lock"))?;
        leases.retain(|_, lease| {
            DateTime::parse_from_rfc3339(&lease.binding.expires_at)
                .is_ok_and(|time| time.with_timezone(&Utc) > Utc::now())
        });
        ensure!(
            leases.len() < 4096 && !leases.contains_key(&binding.bridge_instance_id),
            "feedback.binding_capacity"
        );
        let token = format!(
            "{}{}",
            uuid::Uuid::new_v4().simple(),
            uuid::Uuid::new_v4().simple()
        );
        leases.insert(
            binding.bridge_instance_id.clone(),
            Lease {
                binding,
                token: token.clone(),
                revoked: false,
                discovered: false,
                successful_call: false,
            },
        );
        Ok(token)
    }
    pub fn authenticate(
        &self,
        bridge: &str,
        token: &str,
        binding_digest: &str,
        allow_receipt: bool,
    ) -> Result<Binding> {
        let leases = self
            .leases
            .lock()
            .map_err(|_| anyhow::anyhow!("feedback.binding_lock"))?;
        let lease = leases.get(bridge).context("binding_changed")?;
        ensure!(
            (!lease.revoked || allow_receipt)
                && token.len() == 64
                && lease.token.len() == token.len(),
            "binding_changed"
        );
        let diff = lease
            .token
            .as_bytes()
            .iter()
            .zip(token.as_bytes())
            .fold(0u8, |diff, (a, b)| diff | (a ^ b));
        ensure!(
            diff == 0 && digest(&serde_json::to_value(&lease.binding)?)? == binding_digest,
            "binding_changed"
        );
        ensure!(
            DateTime::parse_from_rfc3339(&lease.binding.expires_at)?.with_timezone(&Utc)
                > Utc::now(),
            "binding_changed"
        );
        Ok(lease.binding.clone())
    }
    pub fn observe(&self, bridge: &str, discovered: bool, successful: bool) {
        if let Ok(mut leases) = self.leases.lock() {
            if let Some(lease) = leases.get_mut(bridge).filter(|lease| !lease.revoked) {
                lease.discovered |= discovered;
                lease.successful_call |= successful;
            }
        }
    }
    pub fn observation(&self, run: &str, epoch: u64) -> Option<(bool, bool)> {
        let leases = self.leases.lock().ok()?;
        let lease = leases.values().find(|lease| !lease.revoked
            && lease.binding.agent_run_id == run && lease.binding.execution_epoch == epoch
            && DateTime::parse_from_rfc3339(&lease.binding.expires_at)
                .is_ok_and(|time| time.with_timezone(&Utc) > Utc::now()))?;
        Some((lease.discovered, lease.successful_call))
    }
    pub fn revoke(&self, bridge: &str) -> Result<()> {
        let mut leases = self
            .leases
            .lock()
            .map_err(|_| anyhow::anyhow!("feedback.binding_lock"))?;
        if let Some(lease) = leases.get_mut(bridge) {
            lease.revoked = true
        }
        Ok(())
    }
    pub fn remove(&self, bridge: &str) -> Result<()> {
        self.leases
            .lock()
            .map_err(|_| anyhow::anyhow!("feedback.binding_lock"))?
            .remove(bridge);
        Ok(())
    }
}
