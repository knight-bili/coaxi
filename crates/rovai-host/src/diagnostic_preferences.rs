//! 仅独立 Server 的 tracing 层读取设置；stderr 原始故障证据仍由 Capture 保存。
use std::{path::PathBuf,sync::Mutex,time::{Duration,Instant}};
use rovai_core::diagnostic_preferences::Settings;

pub(crate) struct Filter{root:PathBuf,cache:Mutex<(Instant,Settings)>}
impl Filter{
    pub(crate) fn new(root:PathBuf)->Self{let settings=rovai_core::diagnostic_preferences::load(&root).unwrap_or_default();Self{root,cache:Mutex::new((Instant::now(),settings))}}
    pub(crate) fn enabled(&self,metadata:&tracing::Metadata<'_>)->bool{
        let Ok(mut cache)=self.cache.lock()else{return *metadata.level()<=tracing::Level::INFO};
        if cache.0.elapsed()>=Duration::from_secs(1){if let Ok(settings)=rovai_core::diagnostic_preferences::load(&self.root){cache.1=settings;}cache.0=Instant::now();}
        cache.1.enabled(metadata.target(),metadata.level().as_str().to_ascii_lowercase().as_str())
    }
}
