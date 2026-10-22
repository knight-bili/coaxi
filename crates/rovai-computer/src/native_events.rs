//! 桌面自身的事件回调，不向公开 Web 广播窗口标题、共享范围或截图。
use std::sync::Arc;
use serde::Serialize;
use serde_json::{json,Value};
use crate::computer::{agent::{ComputerActivityPayload,ComputerGrantPayload},backend::BackendStatus,targets::{SharedApp,SharedScreen,SharedWindow}};

#[derive(Clone)]
pub struct ComputerEvents(Arc<dyn Fn(&str,Value)+Send+Sync>);
impl ComputerEvents {
    pub fn new(callback:impl Fn(&str,Value)+Send+Sync+'static)->Self {Self(Arc::new(callback))}
    fn send(&self,kind:&str,value:impl Serialize){if let Ok(value)=serde_json::to_value(value){(self.0)(kind,value)}}
    pub fn state(&self,shared:&[SharedWindow],apps:&[SharedApp],screen:Option<&SharedScreen>){self.send("computer://state",json!({"shared":shared,"apps":apps,"screen":screen}))}
    pub fn backend_status(&self,value:&BackendStatus){self.send("computer://backend-status",value)}
    pub fn grant(&self,value:&ComputerGrantPayload){self.send("computer://agent-grant",value)}
    pub fn activity(&self,value:&ComputerActivityPayload){self.send("computer://agent-activity",value)}
}
