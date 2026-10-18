//! Optional patch geometry within the directory captured by the trusted Host.
//! Native cwd can select a nested base, but cannot widen that recorded root.
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};

use super::super::source::NativeDocument;

pub(super) struct ProjectFiles {
    root: Option<PathBuf>,
    lines: HashMap<PathBuf, Vec<String>>,
}

impl ProjectFiles {
    pub(super) fn new(recorded_directory: Option<&Path>) -> Self {
        Self {
            root: recorded_directory.and_then(|root| std::fs::canonicalize(root).ok()),
            lines: HashMap::new(),
        }
    }

    pub(super) fn lines(
        &mut self,
        path: &str,
        native_cwd: Option<&str>,
        stop: &AtomicBool,
    ) -> Option<Vec<String>> {
        if stop.load(Ordering::Acquire) || path.contains('\0') {
            return None;
        }
        let root = self.root.as_ref()?;
        let path = Path::new(path);
        #[cfg(windows)]
        if path.components().any(|part| {
            matches!(part, std::path::Component::Normal(name) if name.to_string_lossy().contains(':'))
        }) {
            return None;
        }
        let candidate = if path.is_absolute() {
            path.to_path_buf()
        } else {
            let base = match native_cwd.filter(|cwd| !cwd.is_empty()) {
                Some(cwd) => std::fs::canonicalize(root.join(cwd)).ok()?,
                None => root.clone(),
            };
            if !base.starts_with(root) {
                return None;
            }
            base.join(path)
        };
        // Resolve Windows normal/extended prefixes and aliases before checking
        // containment. A symlink outside the recorded root cannot authorize it.
        let candidate = std::fs::canonicalize(candidate).ok()?;
        if !candidate.starts_with(root) {
            return None;
        }
        if let Some(lines) = self.lines.get(&candidate) {
            return Some(lines.clone());
        }
        let mut document = NativeDocument::open_project(root, &candidate, stop).ok()?;
        let text = super::records::text_document(&mut document, stop).ok()?;
        let lines: Vec<_> = text.lines().map(str::to_owned).collect();
        self.lines.insert(candidate, lines.clone());
        Some(lines)
    }
}
