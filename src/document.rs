use rune::TextEdit;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

pub const MAX_FILE_BYTES: u64 = 8 * 1024 * 1024;

#[derive(Clone, Copy, Default, Serialize, Deserialize)]
pub enum Newlines {
    #[default]
    Lf,
    CrLf,
}

pub struct Document {
    pub id: u64,
    pub path: Option<PathBuf>,
    pub edit: TextEdit,
    pub original: String,
    pub disk_bytes: Option<Vec<u8>>,
    pub newlines: Newlines,
    pub bom: bool,
    pub split_sync: crate::split_sync::SplitSync,
    pub outline: crate::outline::Outline,
    observed: Option<(std::time::SystemTime, u64)>,
}

impl Document {
    pub fn blank(id: u64) -> Self {
        Self {
            id,
            path: None,
            edit: TextEdit::new(""),
            original: String::new(),
            disk_bytes: None,
            newlines: Newlines::Lf,
            bom: false,
            observed: None,
            split_sync: Default::default(),
            outline: Default::default(),
        }
    }
    pub fn open(id: u64, path: &Path) -> Result<Self, String> {
        let path = path
            .canonicalize()
            .map_err(|e| format!("Couldn’t open {}: {e}", path.display()))?;
        let meta = fs::metadata(&path).map_err(|e| e.to_string())?;
        if !meta.is_file() {
            return Err("Choose a Markdown file, not a folder.".into());
        }
        if meta.len() > MAX_FILE_BYTES {
            return Err("This file is larger than 8 MB.".into());
        }
        let bytes = fs::read(&path).map_err(|e| e.to_string())?;
        let raw =
            std::str::from_utf8(&bytes).map_err(|_| "This file isn’t UTF-8 text.".to_string())?;
        if raw.contains('\0') {
            return Err("This file contains binary data.".into());
        }
        let bom = raw.starts_with('\u{feff}');
        let newlines = if raw.contains("\r\n") {
            Newlines::CrLf
        } else {
            Newlines::Lf
        };
        let original = raw.trim_start_matches('\u{feff}').replace("\r\n", "\n");
        let mut edit = TextEdit::new(original.clone());
        edit.set_caret(0);
        Ok(Self {
            id,
            path: Some(path),
            edit,
            original,
            disk_bytes: Some(bytes),
            newlines,
            bom,
            observed: meta.modified().ok().map(|time| (time, meta.len())),
            split_sync: Default::default(),
            outline: Default::default(),
        })
    }
    pub fn title(&self) -> String {
        self.path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Untitled.md".into())
    }
    pub fn dirty(&self) -> bool {
        self.edit.text != self.original
    }
    /// Reload only clean documents. A changed disk file must never replace unsaved edits.
    pub fn reload_if_changed(&mut self) -> Result<bool, String> {
        if self.dirty() {
            return Ok(false);
        }
        let Some(path) = self.path.clone() else {
            return Ok(false);
        };
        let meta = match fs::metadata(&path) {
            Ok(m) => m,
            Err(_) => return Ok(false),
        };
        let observed = meta.modified().ok().map(|t| (t, meta.len()));
        if observed == self.observed {
            return Ok(false);
        }
        let mut next = Self::open(self.id, &path)?;
        next.edit
            .set_caret(self.edit.focus.min(next.edit.text.len()));
        *self = next;
        Ok(true)
    }
    pub fn encoded(&self) -> Vec<u8> {
        let mut value = if self.bom {
            "\u{feff}".to_string()
        } else {
            String::new()
        };
        match self.newlines {
            Newlines::Lf => value.push_str(&self.edit.text),
            Newlines::CrLf => value.push_str(&self.edit.text.replace('\n', "\r\n")),
        }
        value.into_bytes()
    }
    pub fn save(&mut self, chosen: &Path) -> Result<(), String> {
        // Resolve symlinks before the atomic replacement; never replace the link itself.
        let target = if chosen.exists() {
            chosen.canonicalize().map_err(|e| e.to_string())?
        } else {
            chosen.to_path_buf()
        };
        if self.path.as_ref() == Some(&target) && self.disk_bytes.is_some() {
            let current = fs::read(&target)
                .map_err(|e| format!("Couldn’t read the file before saving: {e}"))?;
            if Some(&current) != self.disk_bytes.as_ref() {
                return Err("The file changed on disk. Use Save As to keep your edits in a separate file, or Revert to reload it.".into());
            }
        }
        let bytes = self.encoded();
        atomic_write(&target, &bytes)
            .map_err(|e| format!("Couldn’t save {}: {e}", target.display()))?;
        // Newly named files also need a canonical path, just like opened files. In
        // particular /var and /private/var must not become two recent entries or tabs.
        let target = target
            .canonicalize()
            .map_err(|e| format!("Saved the file, but couldn’t resolve its path: {e}"))?;
        self.observed = fs::metadata(&target)
            .ok()
            .and_then(|m| m.modified().ok().map(|t| (t, m.len())));
        self.path = Some(target);
        self.original = self.edit.text.clone();
        self.disk_bytes = Some(bytes);
        Ok(())
    }
}

pub fn atomic_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    if let Ok(meta) = fs::metadata(path) {
        temp.as_file().set_permissions(meta.permissions())?;
    }
    temp.write_all(bytes)?;
    temp.as_file().sync_all()?;
    temp.persist(path).map_err(|e| e.error)?;
    Ok(())
}

pub fn local_link(base: Option<&Path>, link: &str) -> Option<PathBuf> {
    let base = base?.parent()?;
    let base = url::Url::from_directory_path(base).ok()?;
    base.join(link).ok()?.to_file_path().ok()
}

#[cfg(test)]
#[path = "document_tests.rs"]
mod tests;
