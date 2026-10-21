use serde::{Deserialize,Serialize};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogModel {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub reasoning: bool,
    #[serde(default)]
    pub tool_call: bool,
    #[serde(default)]
    pub context: Option<u64>,
    #[serde(default)]
    pub cost_in: Option<f64>,
    #[serde(default)]
    pub cost_out: Option<f64>,
    #[serde(default)]
    pub cost_cache_read: Option<f64>,
    #[serde(default)]
    pub cost_cache_write: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogProvider {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub npm: Option<String>,
    #[serde(default)]
    pub env: Vec<String>,
    #[serde(default)]
    pub doc: Option<String>,
    /// `"api"` or `"oauth"`.
    pub auth_kind: String,
    #[serde(default)]
    pub models: Vec<CatalogModel>,
}


pub fn bundled_catalog()->Vec<CatalogProvider>{serde_json::from_str(include_str!("../../resources/opencode/models-dev.json")).unwrap_or_default()}
