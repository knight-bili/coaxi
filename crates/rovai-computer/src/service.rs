//! Host-independent surface around Codeg's original helper and target table.
//! It owns no Thread, Run, Mission or model loop; Core supplies that scope.
use crate::computer::{
    agent::{Blocklist, GrantChange, SelfIdentity},
    backend::{ComputerBackend, SnapshotOptions},
    local::LocalBackend,
    protocol::{ClipboardUse, OsPermission},
    targets::{ReadMark, TargetTable},
    types::{ActDelivery, ComputerActRequest},
};
use crate::{ComputerSwitch, GrantLevel};
use anyhow::{anyhow, ensure, Context, Result};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    pub operation: String,
    #[serde(default)]
    pub target_id: Option<String>,
    #[serde(default)]
    pub level: Option<GrantLevel>,
    #[serde(default)]
    pub permission: Option<OsPermission>,
    #[serde(default)]
    pub action: Option<ComputerActRequest>,
    #[serde(default)]
    pub enabled: Option<bool>,
}
pub struct Service {
    backend: LocalBackend,
    targets: TargetTable,
    switch: ComputerSwitch,
    enabled: AtomicBool,
    stop: AtomicU64,
    me: SelfIdentity,
    blocklist: Blocklist,
}
impl Default for Service {
    fn default() -> Self {
        let switch = ComputerSwitch::default();
        Self {
            backend: LocalBackend::new(|_| {}).with_switch(switch.clone()),
            targets: TargetTable::new(),
            switch,
            enabled: AtomicBool::new(false),
            stop: AtomicU64::new(0),
            me: SelfIdentity::current(),
            blocklist: Blocklist::new(&[]),
        }
    }
}
fn failed(error: impl std::fmt::Debug) -> anyhow::Error {
    anyhow!("computer.operation_failed: {error:?}")
}
impl Service {
    pub async fn stop(&self) -> Result<Value> {
        let stop = self.stop.fetch_add(1, Ordering::AcqRel) + 1;
        self.targets.revoke_all(GrantChange::Revoked);
        self.backend.note_stop(stop);
        self.backend.halt(stop).await.map_err(failed)?;
        Ok(json!({"stopped":true,"stop":stop}))
    }
    async fn refresh(&self) -> Result<Value> {
        let windows = self.backend.list_windows(None).await.map_err(failed)?;
        let (entries, _) = self.targets.observe(&windows, None);
        let now = chrono::Utc::now().timestamp_millis();
        let rows=entries.into_iter().filter(|entry|entry.worth_listing()).map(|entry|json!({"targetId":entry.target_id,"app":entry.app.name,"title":entry.title,"bounds":entry.bounds,"level":entry.grant.as_ref().map(|grant|grant.level).unwrap_or_default(),"shareable":crate::computer::agent::grantable(&entry.app,&self.me,&self.blocklist).is_ok()})).collect::<Vec<_>>();
        self.targets.sweep(
            now,
            Some(std::time::Duration::from_secs(1800)),
            &self.me,
            &self.blocklist,
        );
        Ok(json!({"windows":rows}))
    }
    pub async fn handle(&self, request: Request) -> Result<Value> {
        if request.operation == "status" {
            return Ok(
                json!({"enabled":self.enabled.load(Ordering::Acquire),"backend":self.backend.status().await,"shared":self.targets.shared(),"driverVersion":crate::computer::driver::DRIVER_VERSION,"installed":crate::computer::driver::cached_driver_path().is_some()}),
            );
        }
        if request.operation == "enable" {
            let enabled = request.enabled.context("computer.enabled_required")?;
            self.switch.set(enabled);
            self.enabled.store(enabled, Ordering::Release);
            if enabled {
                self.backend.open().await
            } else {
                self.stop().await?;
                self.backend.close_now().await
            }
            return Ok(json!({"enabled":enabled}));
        }
        if request.operation == "stop" {
            return self.stop().await;
        }
        if request.operation == "install" {
            let _ = crate::computer::driver::ensure_driver(|_| {}).await?;
            return Ok(json!({"installed":true}));
        }
        if request.operation == "remove" {
            ensure!(
                !self.enabled.load(Ordering::Acquire),
                "computer.disable_before_remove"
            );
            self.backend.close_now().await;
            crate::computer::driver::forget_cached_driver().await?;
            return Ok(json!({"installed":false}));
        }
        ensure!(self.enabled.load(Ordering::Acquire), "computer.disabled");
        if request.operation == "permissions" {
            return Ok(serde_json::to_value(
                self.backend.permissions().await.map_err(failed)?,
            )?);
        }
        if request.operation == "request_permission" {
            return Ok(serde_json::to_value(
                self.backend
                    .request_permission(request.permission.context("computer.permission_required")?)
                    .await
                    .map_err(failed)?,
            )?);
        }
        let stop = self.stop.load(Ordering::Acquire);
        let windows = self.refresh().await?;
        ensure!(
            self.enabled.load(Ordering::Acquire) && self.stop.load(Ordering::Acquire) == stop,
            "computer.stopped"
        );
        if request.operation == "windows" {
            return Ok(windows);
        }
        let id = request.target_id.context("computer.target_required")?;
        let now = chrono::Utc::now().timestamp_millis();
        if request.operation == "share" {
            let level = request.level.context("computer.level_required")?;
            if level == GrantLevel::None {
                self.backend
                    .note_stop(self.stop.fetch_add(1, Ordering::AcqRel) + 1);
                self.backend
                    .halt(self.stop.load(Ordering::Acquire))
                    .await
                    .map_err(failed)?;
            }
            self.targets
                .share(&id, level, now, &self.me, &self.blocklist)
                .map_err(failed)?;
            return Ok(json!({"shared":self.targets.shared()}));
        }
        let ttl = Some(std::time::Duration::from_secs(1800));
        if request.operation == "act" {
            let action = request.action.context("computer.action_required")?;
            ensure!(
                !action.needs_launch_switch(),
                "computer.window_movement_not_enabled"
            );
            let ticket = self
                .targets
                .begin_act(&id, now, ttl, &self.me, &self.blocklist, &action, false)
                .map_err(failed)?;
            let result = self
                .backend
                .act(
                    ticket.identity.pid,
                    ticket.identity.window_id,
                    ticket
                        .identity
                        .started_at
                        .context("computer.process_unidentified")?,
                    ticket.identity.content,
                    ticket.app.key().map(String::from),
                    ticket.action,
                    ActDelivery::default(),
                    ClipboardUse::default(),
                    stop,
                )
                .await
                .map_err(failed)?;
            self.refresh().await?;
            ensure!(
                self.enabled.load(Ordering::Acquire)
                    && self.stop.load(Ordering::Acquire) == stop
                    && self
                        .targets
                        .get(&id)
                        .is_some_and(|entry| entry.identity == ticket.identity
                            && entry.epoch == ticket.epoch
                            && entry
                                .grant
                                .is_some_and(|grant| grant.level == GrantLevel::Control)),
                "computer.grant_changed_after_action"
            );
            return Ok(serde_json::to_value(result)?);
        }
        let ticket = self
            .targets
            .begin_read(&id, now, ttl, &self.me, &self.blocklist)
            .map_err(failed)?;
        let pid = ticket.identity.pid;
        let window = ticket.identity.window_id;
        let (mut result, mark) = match request.operation.as_str() {
            "capture" => {
                let value = self
                    .backend
                    .capture(pid, window, Some(2048))
                    .await
                    .map_err(failed)?;
                let mark = ReadMark::Capture {
                    width: value.width,
                    height: value.height,
                    native_width: value.native_width,
                    native_height: value.native_height,
                    full_size: value.full_size,
                    window_bounds: value.window_bounds,
                };
                (serde_json::to_value(value)?, mark)
            }
            "snapshot" => {
                let value = self
                    .backend
                    .snapshot(
                        pid,
                        window,
                        SnapshotOptions {
                            max_depth: Some(32),
                            max_elements: Some(10000),
                            query: None,
                            app_menus: false,
                        },
                    )
                    .await
                    .map_err(failed)?;
                ensure!(
                    value.tree.len() <= 8 * 1024 * 1024,
                    "computer.snapshot_limit"
                );
                let mark = ReadMark::Snapshot {
                    snapshot_id: value.snapshot_id.clone(),
                    shown: value.refs.iter().map(|entry| entry.index).collect(),
                    cut: Default::default(),
                    secret: value
                        .refs
                        .iter()
                        .filter(|entry| entry.secret)
                        .map(|entry| entry.index)
                        .collect(),
                };
                (serde_json::to_value(value)?, mark)
            }
            _ => anyhow::bail!("computer.operation_not_admitted"),
        };
        self.refresh().await?;
        ensure!(
            self.enabled.load(Ordering::Acquire) && self.stop.load(Ordering::Acquire) == stop,
            "computer.stopped"
        );
        let generation = self
            .targets
            .finish_read(&ticket, &self.me, &self.blocklist, Some(mark))
            .map_err(failed)?;
        result["generation"] = json!(generation);
        result["targetId"] = json!(id);
        Ok(result)
    }
}
