use crate::document::atomic_write;
use std::{
    fs,
    path::{Path, PathBuf},
};

const LIMIT: usize = 100;

/// Most-recent first. Only successful opens/saves enter the history; untitled documents and
/// failed opens never do. Headless runs use memory-only history and cannot touch user data.
#[derive(Default)]
pub struct History {
    paths: Vec<PathBuf>,
    storage: Option<PathBuf>,
}
impl History {
    pub fn load(path: PathBuf) -> Result<Self, String> {
        let paths: Vec<PathBuf> = match fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map_err(|e| format!("Couldn’t read recent documents: {e}"))?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(e) => return Err(format!("Couldn’t read recent documents: {e}")),
        };
        let mut history = Self {
            paths: Vec::new(),
            storage: Some(path),
        };
        for path in paths {
            if path.is_absolute() && !history.paths.contains(&path) {
                history.paths.push(path);
            }
        }
        history.paths.truncate(LIMIT);
        Ok(history)
    }
    pub fn paths(&self) -> &[PathBuf] {
        &self.paths
    }
    pub fn record(&mut self, path: &Path) -> Result<(), String> {
        if self.paths.first().is_some_and(|p| p == path) {
            return Ok(());
        }
        self.paths.retain(|p| p != path);
        self.paths.insert(0, path.to_path_buf());
        self.paths.truncate(LIMIT);
        self.persist()
    }
    pub fn clear(&mut self) -> Result<(), String> {
        self.paths.clear();
        self.persist()
    }
    fn persist(&self) -> Result<(), String> {
        let Some(path) = &self.storage else {
            return Ok(());
        };
        fs::create_dir_all(path.parent().ok_or("Missing recent documents directory")?)
            .map_err(|e| e.to_string())?;
        let bytes = serde_json::to_vec(&self.paths).map_err(|e| e.to_string())?;
        atomic_write(path, &bytes).map_err(|e| format!("Couldn’t save recent documents: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn history_reorders_deduplicates_bounds_and_survives_a_restart_and_clear() {
        let dir = tempfile::tempdir().unwrap();
        let storage = dir.path().join("history/recents.json");
        let mut h = History::load(storage.clone()).unwrap();
        for i in 0..105 {
            h.record(&dir.path().join(format!("note {i} 日本語.md")))
                .unwrap();
        }
        let old = dir.path().join("note 20 日本語.md");
        h.record(&old).unwrap();
        let mut restarted = History::load(storage.clone()).unwrap();
        assert_eq!(restarted.paths().len(), 100);
        assert_eq!(restarted.paths()[0], old);
        assert_eq!(restarted.paths().iter().filter(|p| **p == old).count(), 1);
        assert!(
            !restarted
                .paths()
                .contains(&dir.path().join("note 0 日本語.md"))
        );
        restarted.clear().unwrap();
        assert!(History::load(storage).unwrap().paths().is_empty());
    }
}
