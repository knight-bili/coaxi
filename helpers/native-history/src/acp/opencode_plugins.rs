pub fn xdg_cache_home()->Option<std::path::PathBuf>{Some(crate::scoped_fs::root().join(".cache"))}
pub fn xdg_config_home()->Option<std::path::PathBuf>{Some(crate::scoped_fs::root().join(".config"))}
