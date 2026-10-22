//! Run 专用桥和只追加语义的操作回执；不拥有 Thread/Conversation/Run 数据。
use super::*;
use chrono::{DateTime, Utc};
use std::{collections::{BTreeMap, HashMap}, sync::{Mutex, MutexGuard}};
use subtle::ConstantTimeEq;

#[derive(Clone)]
struct Bridge { binding: Value, features: Features, secret: String, revoked: bool }
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Ledger { schema_version: u32, binding: Value, call: Value, receipt: Value, grants: Vec<Value>, owner_reconciled: bool }
#[derive(Default)]
struct State {
    recovered: bool,
    bridges: HashMap<String, Bridge>,
    peers: HashMap<String, Value>,
    grants: BTreeMap<String, Value>,
    ledger: BTreeMap<String, Ledger>,
    approvals: HashMap<String, String>,
    targets:HashMap<String,(u64,Value)>,
}
pub struct Broker { pub policies: PolicyStore, root: PathBuf, core_instance: String, state: Mutex<State> }
fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str> { value[key].as_str().context("interaction.invalid_identity") }
fn unexpired(value: &Value) -> bool { value["expiresAt"].as_str().and_then(|time| DateTime::parse_from_rfc3339(time).ok()).is_some_and(|time| time > Utc::now()) }
pub fn digest(value: &Value) -> Result<String> { Ok(format!("sha256:{}", crate::command::canonical_json_digest(value)?)) }
fn same_scope(left: &Value, right: &Value) -> bool { left["scope"] == right["scope"] && left["agentId"] == right["agentId"] }

impl Broker {
    pub fn new(data_dir: &Path) -> Self { Self { policies: PolicyStore::new(data_dir), root: data_dir.join("interaction-tools"), core_instance: uuid::Uuid::new_v4().to_string(), state: Default::default() } }
    pub fn core_instance(&self) -> &str { &self.core_instance }
    fn locked(&self) -> Result<MutexGuard<'_, State>> {
        let mut state = self.state.lock().map_err(|_| anyhow::anyhow!("interaction.state_lock"))?;
        if !state.recovered {
            let directory = self.root.join("ledger");
            if directory.exists() {
                let paths = std::fs::read_dir(&directory)?.collect::<std::io::Result<Vec<_>>>()?;
                ensure!(paths.len() <= 100_000, "interaction.ledger_capacity");
                for entry in paths {
                    if entry.path().extension().and_then(|extension| extension.to_str()) != Some("json") { continue; }
                    ensure!(entry.file_type()?.is_file() && entry.metadata()?.len() <= 2 * MAX_REQUEST_BYTES as u64, "interaction.invalid_ledger");
                    let mut ledger: Ledger = serde_json::from_slice(&std::fs::read(entry.path())?)?;
                    ensure!(ledger.schema_version == 1, "interaction.ledger_version");
                    validate_shape("binding", &ledger.binding)?;
                    validate_shape("bridgeCall", &ledger.call)?;
                    validate_shape("receipt", &ledger.receipt)?;
                    let id = text(&ledger.receipt, "bridgeCallId")?.to_owned();
                    ensure!(entry.path().file_stem().and_then(|name| name.to_str()) == Some(id.as_str()), "interaction.ledger_identity");
                    if ledger.receipt["state"] == "started" { ledger.receipt["state"] = json!("unknown"); ledger.receipt["finishedAt"] = json!(Utc::now().to_rfc3339()); self.persist(&ledger)?; }
                    if ledger.receipt["state"] == "prepared" { ledger.receipt["state"] = json!("cancelled"); ledger.receipt["finishedAt"] = json!(Utc::now().to_rfc3339()); self.persist(&ledger)?; }
                    state.ledger.insert(id, ledger);
                }
            }
            state.recovered = true;
        }
        Ok(state)
    }
    fn persist(&self, ledger: &Ledger) -> Result<()> {
        validate_shape("receipt", &ledger.receipt)?;
        let directory = self.root.join("ledger");
        crate::platform::private_storage::prepare_private_directory(&directory)?;
        let id = text(&ledger.receipt, "bridgeCallId")?;
        uuid::Uuid::parse_str(id)?;
        crate::platform::private_storage::atomic_write_private_json(&directory.join(format!("{id}.json")), &serde_json::to_value(ledger)?)
    }
    pub fn register(&self, binding: Value, features: Features) -> Result<String> {
        validate_shape("binding", &binding)?;
        features.validate()?;
        ensure!(features.any() && binding["coreInstanceId"] == self.core_instance && unexpired(&binding), "binding_changed");
        let id = text(&binding, "bridgeInstanceId")?.to_owned();
        let secret = format!("{}{}", uuid::Uuid::new_v4().simple(), uuid::Uuid::new_v4().simple());
        let mut state = self.locked()?;
        ensure!(!state.bridges.contains_key(&id) && state.bridges.len() < 4096, "interaction.bridge_capacity");
        // 同一 Run/epoch 的旧桥先撤销；旧连接永远不能转绑到新范围。
        let old: Vec<String> = state.bridges.iter().filter(|(_, bridge)| bridge.binding["agentRunId"] == binding["agentRunId"]).map(|(id, _)| id.clone()).collect();
        for old in old { self.revoke_locked(&mut state, &old)?; }
        state.bridges.insert(id, Bridge { binding, features, secret: secret.clone(), revoked: false });
        Ok(secret)
    }
    fn current_bridge(&self, state: &State, id: &str) -> Result<Bridge> {
        let bridge = state.bridges.get(id).context("binding_changed")?;
        ensure!(!bridge.revoked && unexpired(&bridge.binding), "run_not_active");
        ensure!(bridge.binding["coreInstanceId"] == self.core_instance, "binding_changed");
        let policy = self.policies.read()?;
        ensure!(bridge.binding["permissionRevision"] == policy.revision && policy.features(text(&bridge.binding, "agentId")?) == bridge.features, "permission_changed");
        Ok(bridge.clone())
    }
    pub fn authenticated_binding(&self, id: &str, secret: &str) -> Result<Value> {
        let state = self.locked()?;
        let bridge = self.current_bridge(&state, id)?;
        ensure!(bridge.secret.as_bytes().ct_eq(secret.as_bytes()).into(), "binding_changed");
        Ok(bridge.binding)
    }
    pub fn catalog(&self, id: &str, secret: &str) -> Result<Vec<Value>> {
        self.authenticated_binding(id, secret)?;
        let state = self.locked()?;
        Ok(self.current_bridge(&state, id)?.features.catalog())
    }
    pub fn attach_peer(&self, peer: Value) -> Result<Value> {
        validate_shape("desktopPeer", &peer)?;
        ensure!(peer["coreInstanceId"] == self.core_instance && unexpired(&peer), "desktop_unavailable");
        let id = text(&peer, "desktopPeerId")?.to_owned();
        let mut state = self.locked()?;
        if let Some(previous) = state.peers.get(&id) { ensure!(previous["ownerWindowId"] == peer["ownerWindowId"] && previous["connectionGeneration"] == peer["connectionGeneration"] && previous["capabilities"] == peer["capabilities"], "desktop_unavailable"); }
        else { ensure!(state.peers.len() < 16, "interaction.peer_capacity"); }
        state.peers.insert(id, peer.clone());
        Ok(peer)
    }
    pub fn peers(&self) -> Result<Vec<Value>> { Ok(self.locked()?.peers.values().filter(|peer| unexpired(peer)).cloned().collect()) }
    pub fn register_targets(&self,value:Value)->Result<Value>{
        #[derive(Deserialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]struct Catalog{desktop_peer_id:String,connection_generation:u64,sequence:u64,targets:Vec<Target>}
        #[derive(Deserialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]struct Target{target_id:String,target_kind:String,app_id:Option<String>,shareable:bool}
        let input:Catalog=serde_json::from_value(value.clone())?;
        ensure!(input.sequence>0&&input.sequence<=MAX_SAFE_INTEGER&&input.targets.len()<=4096,"invalid_arguments");
        let mut ids=BTreeSet::new();
        for target in &input.targets{ensure!(ids.insert(&target.target_id)&&target.target_id.len()<=256&&matches!(target.target_kind.as_str(),"computer_window"|"computer_app"|"computer_desktop"),"invalid_arguments");if target.target_kind=="computer_window"{ensure!(target.app_id.as_ref().is_none_or(|id|id.len()<=256),"invalid_arguments")}}
        let mut state=self.locked()?;self.peer(&state,&input.desktop_peer_id,&json!(input.connection_generation))?;
        if let Some((sequence,previous))=state.targets.get(&input.desktop_peer_id){if *sequence==input.sequence&&previous==&value{return Ok(json!({"registered":true}))}ensure!(input.sequence>*sequence,"generation_changed")}
        state.grants.retain(|_,grant|grant["desktopPeerId"]!=input.desktop_peer_id||grant["targetKind"]=="browser_tab"||input.targets.iter().any(|target|grant["targetId"]==target.target_id&&grant["targetKind"]==target.target_kind&&target.shareable));
        state.targets.insert(input.desktop_peer_id,(input.sequence,value));Ok(json!({"registered":true}))
    }
    fn peer<'a>(&self, state: &'a State, id: &str, generation: &Value) -> Result<&'a Value> {
        let peer = state.peers.get(id).context("desktop_unavailable")?;
        ensure!(unexpired(peer) && &peer["connectionGeneration"] == generation, "desktop_unavailable");
        Ok(peer)
    }
    pub fn share(&self, grant: Value) -> Result<Value> {
        validate_shape("resourceGrant", &grant)?;
        ensure!(unexpired(&grant), "grant_required");
        let expires = DateTime::parse_from_rfc3339(text(&grant, "expiresAt")?)?;
        ensure!(expires <= Utc::now() + chrono::Duration::minutes(120), "grant_required");
        if grant["targetKind"] == "browser_tab" {
            let origin = url::Url::parse(text(&grant, "origin")?)?;
            ensure!(matches!(origin.scheme(), "http" | "https") && origin.username().is_empty() && origin.password().is_none() && origin.origin().ascii_serialization() == text(&grant, "origin")?, "origin_changed");
        } else { ensure!(grant["origin"].is_null(), "origin_changed"); }
        let mut state = self.locked()?;
        let peer_id = text(&grant, "desktopPeerId")?;
        ensure!(state.peers.get(peer_id).is_some_and(unexpired), "desktop_unavailable");
        if grant["targetKind"]!="browser_tab"{ensure!(state.targets.get(peer_id).and_then(|(_,catalog)|catalog["targets"].as_array()).is_some_and(|targets|targets.iter().any(|target|target["targetId"]==grant["targetId"]&&target["targetKind"]==grant["targetKind"]&&target["shareable"]==true)),"target_changed")}
        let id = text(&grant, "grantId")?.to_owned();
        if let Some(previous) = state.grants.get(&id) { if previous == &grant {return Ok(previous.clone());} ensure!(grant["grantGeneration"].as_u64() > previous["grantGeneration"].as_u64() && same_scope(previous, &grant) && previous["desktopPeerId"] == grant["desktopPeerId"] && previous["targetId"] == grant["targetId"], "generation_changed"); }
        state.grants.insert(id, grant.clone());
        Ok(grant)
    }
    pub fn revoke_grant(&self, id: &str) -> Result<()> { self.locked()?.grants.remove(id); Ok(()) }
    pub fn cancel_call(&self, bridge_id: &str, id: &str) -> Result<Value> {
        let mut state = self.locked()?;
        let ledger = state.ledger.get_mut(id).context("original_request_pending")?;
        ensure!(ledger.binding["bridgeInstanceId"] == bridge_id, "binding_changed");
        if matches!(ledger.receipt["state"].as_str(), Some("prepared" | "started")) {
            ledger.receipt["state"] = json!(if ledger.receipt["state"] == "started" { "unknown" } else { "cancelled" });
            ledger.receipt["finishedAt"] = json!(Utc::now().to_rfc3339()); self.persist(ledger)?;
        }
        Ok(ledger.receipt.clone())
    }
    pub fn disconnect(&self, bridge_id: &str) -> Result<()> { self.revoke_locked(&mut self.locked()?, bridge_id) }
    pub fn grants(&self, binding: &Value) -> Result<Vec<Value>> { Ok(self.locked()?.grants.values().filter(|grant| unexpired(grant) && same_scope(grant, binding) && grant["desktopPeerId"] == binding["desktopPeerId"]).cloned().collect()) }
    fn selected_grants(&self, state: &State, binding: &Value, name: &str, arguments: &Value) -> Result<Vec<Value>> {
        if matches!(name,"computer_launch_app"|"computer_clipboard_read"|"computer_clipboard_write"){return Ok(Vec::new())}
        if name == "browser_open_tab" { return Ok(Vec::new()); }
        if matches!(name, "browser_list_tabs" | "computer_list_apps" | "computer_list_windows") {
            return Ok(state.grants.values().filter(|grant| unexpired(grant) && same_scope(grant,binding) && grant["desktopPeerId"] == binding["desktopPeerId"]).cloned().collect());
        }
        let target = arguments["tabId"].as_str().or(arguments["targetId"].as_str());
        let app_id=state.targets.get(binding["desktopPeerId"].as_str().unwrap_or("")).and_then(|(_,catalog)|catalog["targets"].as_array()).and_then(|targets|targets.iter().find(|row|row["targetId"].as_str()==target)).and_then(|row|row["appId"].as_str());
        let needed = if mutates(name) { "control" } else { "read" };
        let selected: Vec<Value> = state.grants.values().filter(|grant| {
            if !unexpired(grant) || !same_scope(grant, binding) || grant["desktopPeerId"] != binding["desktopPeerId"] { return false; }
            if needed == "control" && grant["level"] != "control" { return false; }
            match name {
                "computer_launch_app" => grant["targetKind"] == "computer_app" && (arguments["key"] == grant["targetId"] || arguments["name"] == grant["targetId"]),
                "computer_clipboard_read" | "computer_clipboard_write" => grant["targetKind"] == "computer_desktop",
                "computer_invoke_menu" => grant["targetKind"] == "computer_app" && app_id.is_some() && app_id == grant["targetId"].as_str(),
                _ if name.starts_with("browser_") => target == grant["targetId"].as_str() && grant["targetKind"] == "browser_tab",
                _ => (grant["targetKind"]=="computer_window"&&target==grant["targetId"].as_str())||(grant["targetKind"]=="computer_app"&&app_id.is_some()&&app_id==grant["targetId"].as_str())||(grant["targetKind"]=="computer_desktop"&&grant["targetId"]==rovai_computer::computer::targets::SCREEN_TARGET_ID),
            }
        }).cloned().collect();
        ensure!(!selected.is_empty(), "grant_required");
        Ok(selected)
    }
    fn grants_current(&self, state: &State, ledger: &Ledger) -> Result<()> {
        for expected in &ledger.grants { ensure!(state.grants.get(text(expected, "grantId")?).is_some_and(|current| current == expected && unexpired(current)), "grant_required"); }
        Ok(())
    }
    pub fn enqueue(&self, call: Value, secret: &str) -> Result<Value> {
        validate_shape("bridgeCall", &call)?;
        ensure!(call["nativeToolCallId"].is_null(), "interaction.native_call_id_requires_provider_proof");
        let bridge_id = text(&call, "bridgeInstanceId")?;
        self.authenticated_binding(bridge_id, secret)?;
        let mut state = self.locked()?;
        let bridge = self.current_bridge(&state, bridge_id)?;
        let name = text(&call, "toolName")?;
        validate_arguments(bridge.features, name, &call["arguments"])?;
        ensure!(digest(&call["arguments"])? == text(&call, "argumentsDigest")?, "invalid_arguments");
        let id = text(&call, "bridgeCallId")?.to_owned();
        if let Some(original) = state.ledger.get(&id) { ensure!(original.call == call && original.binding == bridge.binding, "invalid_arguments"); return Ok(original.receipt.clone()); }
        if let Some(original) = state.ledger.values().find(|ledger| ledger.call["bridgeInstanceId"] == call["bridgeInstanceId"] && ledger.call["rpcId"] == call["rpcId"]) { ensure!(original.call == call, "invalid_arguments"); return Ok(original.receipt.clone()); }
        ensure!(state.ledger.values().filter(|ledger| ledger.call["bridgeInstanceId"] == call["bridgeInstanceId"] && matches!(ledger.receipt["state"].as_str(), Some("prepared" | "started"))).count() < 32, "original_request_pending");
        if mutates(name) { ensure!(!state.ledger.values().any(|ledger| !ledger.owner_reconciled && ledger.receipt["state"] == "unknown" && same_scope(&ledger.binding, &bridge.binding)), "unknown_outcome"); }
        let grants = self.selected_grants(&state, &bridge.binding, name, &call["arguments"])?;
        let receipt = json!({"schemaVersion":1,"bridgeCallId":id,"bridgeInstanceId":bridge_id,"bindingDigest":digest(&bridge.binding)?,"toolName":name,"argumentsDigest":call["argumentsDigest"],"nativeToolCallId":null,"state":"prepared","resultDigest":null,"resultLocator":null,"admittedAt":Utc::now().to_rfc3339(),"finishedAt":null,"delivered":false});
        let ledger = Ledger { schema_version:1, binding:bridge.binding, call, receipt:receipt.clone(), grants, owner_reconciled:false };
        self.persist(&ledger)?;
        state.ledger.insert(id, ledger);
        Ok(receipt)
    }
    pub fn approve(&self, id: &str, arguments_digest: &str) -> Result<()> {
        let mut state = self.locked()?;
        let ledger = state.ledger.get(id).context("original_request_pending")?;
        ensure!(needs_local_approval(text(&ledger.call,"toolName")?) && ledger.receipt["state"] == "prepared" && ledger.call["argumentsDigest"] == arguments_digest, "approval_rejected");
        self.current_bridge(&state, text(&ledger.binding, "bridgeInstanceId")?)?;
        self.grants_current(&state, ledger)?;
        state.approvals.insert(id.to_owned(), arguments_digest.to_owned());
        Ok(())
    }
    pub fn check_desktop_call(&self, request: &Value, authority: impl Fn(&Value) -> Result<()>) -> Result<Value> {
        let state = self.locked()?;
        self.peer(&state,text(request,"desktopPeerId")?,&request["connectionGeneration"])?;
        let ledger = state.ledger.get(text(request,"bridgeCallId")?).context("original_request_pending")?;
        ensure!(ledger.receipt["state"] == "started" && ledger.binding["desktopPeerId"] == request["desktopPeerId"] && ledger.receipt["bindingDigest"] == request["bindingDigest"] && ledger.call["argumentsDigest"] == request["argumentsDigest"],"binding_changed");
        authority(&ledger.binding)?;
        self.current_bridge(&state,text(&ledger.binding,"bridgeInstanceId")?)?;
        self.grants_current(&state,ledger)?;
        Ok(json!({"binding":ledger.binding,"call":ledger.call,"grants":ledger.grants,"receipt":ledger.receipt}))
    }
    pub fn poll(&self, request: &Value, authority: impl Fn(&Value) -> Result<()>) -> Result<Value> {
        validate_shape("desktopPoll", request)?;
        let mut state = self.locked()?;
        self.peer(&state, text(request, "desktopPeerId")?, &request["connectionGeneration"])?;
        let ids: Vec<String> = state.ledger.iter().filter(|(_, ledger)| ledger.receipt["state"] == "prepared" && ledger.binding["desktopPeerId"] == request["desktopPeerId"]).map(|(id, _)| id.clone()).collect();
        let mut calls = Vec::new();
        for id in ids {
            if calls.len() >= request["limit"].as_u64().unwrap_or(1) as usize { break; }
            let mut ledger = state.ledger[&id].clone();
            let valid = authority(&ledger.binding).and_then(|_| self.current_bridge(&state, text(&ledger.binding, "bridgeInstanceId")?).map(|_| ())).and_then(|_| self.grants_current(&state, &ledger));
            if valid.is_err() { ledger.receipt["state"] = json!("cancelled"); ledger.receipt["finishedAt"] = json!(Utc::now().to_rfc3339()); self.persist(&ledger)?; state.ledger.insert(id, ledger); continue; }
            if needs_local_approval(text(&ledger.call,"toolName")?) && state.approvals.get(&id).map(String::as_str) != ledger.call["argumentsDigest"].as_str() { continue; }
            // 这一持久化先于把动作交给桌面；中途丢失只能记 unknown。
            ledger.receipt["state"] = json!("started");
            self.persist(&ledger)?;
            state.approvals.remove(&id);
            calls.push(json!({"binding":ledger.binding,"call":ledger.call,"grants":ledger.grants,"receipt":ledger.receipt}));
            state.ledger.insert(id, ledger);
        }
        Ok(json!({"calls":calls}))
    }
    pub fn finish(&self, request: &Value, authority: impl Fn(&Value) -> Result<()>) -> Result<Value> {
        validate_shape("desktopFinish", request)?;
        let mut state = self.locked()?;
        self.peer(&state, text(request, "desktopPeerId")?, &request["connectionGeneration"])?;
        let id = text(request, "bridgeCallId")?;
        let mut ledger = state.ledger.get(id).context("original_request_pending")?.clone();
        ensure!(ledger.binding["desktopPeerId"] == request["desktopPeerId"] && ledger.receipt["bindingDigest"] == request["bindingDigest"] && ledger.call["argumentsDigest"] == request["argumentsDigest"], "binding_changed");
        if ledger.receipt["state"] != "started" { return Ok(ledger.receipt); }
        let valid = authority(&ledger.binding).and_then(|_| self.current_bridge(&state, text(&ledger.binding, "bridgeInstanceId")?).map(|_| ())).and_then(|_| self.grants_current(&state, &ledger)).is_ok();
        let normalized = if request["outcome"] == "completed" {super::output::normalize(text(&ledger.call,"toolName")?,request["result"].clone())?} else {request["result"].clone()};
        let result = &normalized;
        ensure!(serde_json::to_vec(result)?.len() <= MAX_RESULT_BYTES, "result_too_large");
        if !result.is_null() {
            let directory = self.root.join("results");
            crate::platform::private_storage::prepare_private_directory(&directory)?;
            crate::platform::private_storage::atomic_write_private_json(&directory.join(format!("{id}.json")), result)?;
            ledger.receipt["resultDigest"] = json!(digest(result)?);
            ledger.receipt["resultLocator"] = json!(format!("interaction-tools/results/{id}.json"));
        }
        ledger.receipt["state"] = request["outcome"].clone();
        ledger.receipt["finishedAt"] = json!(Utc::now().to_rfc3339());
        ledger.receipt["delivered"] = json!(false);
        if !valid && request["outcome"] == "completed" { ledger.receipt["state"] = json!("unknown"); }
        self.persist(&ledger)?;
        state.ledger.insert(id.to_owned(), ledger.clone());
        Ok(ledger.receipt)
    }
    pub fn result(&self, bridge_id: &str, secret: &str, id: &str, authority: impl Fn(&Value) -> Result<()>) -> Result<Value> {
        let binding = self.authenticated_binding(bridge_id, secret)?;
        authority(&binding)?;
        let mut state = self.locked()?;
        let mut ledger = state.ledger.get(id).context("original_request_pending")?.clone();
        ensure!(ledger.binding == binding, "binding_changed");
        self.grants_current(&state, &ledger)?;
        match ledger.receipt["state"].as_str() {
            Some("completed") => {
                ensure!(!ledger.receipt["resultLocator"].is_null(), "transport_failed");
                let bytes = std::fs::read(self.root.join("results").join(format!("{id}.json")))?;
                ensure!(bytes.len() <= MAX_RESULT_BYTES, "result_too_large");
                let value: Value = serde_json::from_slice(&bytes)?;
                ensure!(digest(&value)? == text(&ledger.receipt, "resultDigest")?, "transport_failed");
                ledger.receipt["delivered"] = json!(true); self.persist(&ledger)?; state.ledger.insert(id.to_owned(), ledger);
                Ok(json!({"state":"completed","result":value}))
            }
            Some("prepared" | "started") => Ok(json!({"state":ledger.receipt["state"],"result":null})),
            Some("unknown") => Ok(json!({"state":"unknown","result":failure("unknown", "unknown_outcome")})),
            _ => Ok(json!({"state":ledger.receipt["state"],"result":failure("cancelled", "cancelled")})),
        }
    }
    fn revoke_locked(&self, state: &mut State, id: &str) -> Result<()> {
        if let Some(bridge) = state.bridges.get_mut(id) { bridge.revoked = true; }
        for ledger in state.ledger.values_mut().filter(|ledger| ledger.call["bridgeInstanceId"] == id && matches!(ledger.receipt["state"].as_str(), Some("prepared" | "started"))) {
            ledger.receipt["state"] = json!(if ledger.receipt["state"] == "started" { "unknown" } else { "cancelled" }); ledger.receipt["finishedAt"] = json!(Utc::now().to_rfc3339()); self.persist(ledger)?;
        }
        Ok(())
    }
    pub fn revoke_run(&self, run: &str) -> Result<()> { let mut state = self.locked()?; let ids: Vec<String> = state.bridges.iter().filter(|(_, bridge)| bridge.binding["agentRunId"] == run).map(|(id, _)| id.clone()).collect(); for id in ids { self.revoke_locked(&mut state, &id)?; } Ok(()) }
    pub fn revoke_peer(&self, peer: &str) -> Result<()> { let mut state = self.locked()?; state.peers.remove(peer); state.grants.retain(|_, grant| grant["desktopPeerId"] != peer); let ids: Vec<String> = state.bridges.iter().filter(|(_, bridge)| bridge.binding["desktopPeerId"] == peer).map(|(id, _)| id.clone()).collect(); for id in ids { self.revoke_locked(&mut state, &id)?; } Ok(()) }
    pub fn stop_computer(&self)->Result<Value>{let mut state=self.locked()?;state.grants.retain(|_,grant|grant["targetKind"]=="browser_tab");let ids:Vec<String>=state.ledger.iter().filter(|(_,ledger)|ledger.call["toolName"].as_str().is_some_and(|name|name.starts_with("computer_"))&&matches!(ledger.receipt["state"].as_str(),Some("prepared"|"started"))).map(|(id,_)|id.clone()).collect();for id in ids{state.approvals.remove(&id);let ledger=state.ledger.get_mut(&id).context("original_request_pending")?;ledger.receipt["state"]=json!(if ledger.receipt["state"]=="started"{"unknown"}else{"cancelled"});ledger.receipt["finishedAt"]=json!(Utc::now().to_rfc3339());self.persist(ledger)?;}Ok(json!({"revoked":true}))}
    pub fn pending(&self) -> Result<Vec<Value>> { Ok(self.locked()?.ledger.values().filter(|ledger| matches!(ledger.receipt["state"].as_str(), Some("prepared" | "started" | "unknown")) && !ledger.owner_reconciled).map(|ledger| json!({"binding":ledger.binding,"call":ledger.call,"receipt":ledger.receipt})).collect()) }
    pub fn reconcile_unknown(&self, id: &str, expected_digest: &str) -> Result<Value> { let mut state = self.locked()?; let ledger = state.ledger.get_mut(id).context("original_request_pending")?; ensure!(ledger.receipt["state"] == "unknown" && ledger.call["argumentsDigest"] == expected_digest, "original_request_pending"); ledger.owner_reconciled = true; self.persist(ledger)?; Ok(ledger.receipt.clone()) }
}
