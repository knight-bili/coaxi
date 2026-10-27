//! Coaxi computer helper and policy primitives; Core owns Thread/Run grants.
pub mod computer;
mod driver_cache;
pub mod service;
pub mod transport;
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GrantLevel {
    #[default]
    None,
    Read,
    Control,
}
impl GrantLevel {
    pub fn allows(self, required: Self) -> bool {
        fn rank(v: GrantLevel) -> u8 {
            match v {
                GrantLevel::None => 0,
                GrantLevel::Read => 1,
                GrantLevel::Control => 2,
            }
        }
        rank(self) >= rank(required)
    }
}
#[derive(Clone, Default)]
pub struct ComputerSwitch(std::sync::Arc<std::sync::atomic::AtomicBool>);
impl ComputerSwitch {
    pub async fn is_enabled(&self) -> bool {
        self.0.load(std::sync::atomic::Ordering::Acquire)
    }
    pub fn set(&self, value: bool) {
        self.0.store(value, std::sync::atomic::Ordering::Release)
    }
}
pub fn current_platform() -> String {
    let os = match std::env::consts::OS {
        "macos" => "darwin",
        v => v,
    };
    format!("{os}-{}", std::env::consts::ARCH)
}

pub mod model_tools;
pub mod native_events;
pub mod native_service;

pub mod model_dispatch;

pub mod native_host;
pub mod native_registry;
