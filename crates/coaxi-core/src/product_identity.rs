//! Installation/discovery identity only; no Runtime or model context vocabulary.
pub const PRODUCT_ID: &str = "coaxi";
pub const APPLICATION_DATA_NAME: &str = "Coaxi";

pub fn desktop_automation_root() -> Option<std::path::PathBuf> {
    #[cfg(target_os = "linux")]
    let base = dirs::config_dir();
    #[cfg(not(target_os = "linux"))]
    let base = dirs::data_dir();
    base.map(|root| root.join(APPLICATION_DATA_NAME).join("automation-v1"))
}
