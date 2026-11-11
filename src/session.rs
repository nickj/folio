use crate::document::{Document, Newlines, atomic_write};
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

#[derive(Serialize, Deserialize)]
struct Draft {
    path: Option<PathBuf>,
    text: String,
    original: String,
    disk_bytes: Option<Vec<u8>>,
    newlines: Newlines,
    bom: bool,
}

pub fn directory() -> PathBuf {
    PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
        .join("Library/Application Support/Folio")
}

pub fn save(documents: &[Document]) -> Result<(), String> {
    save_at(&directory().join("drafts.json"), documents)
}

pub fn save_at(path: &std::path::Path, documents: &[Document]) -> Result<(), String> {
    let drafts: Vec<_> = documents
        .iter()
        .filter(|d| d.dirty())
        .map(|d| Draft {
            path: d.path.clone(),
            text: d.edit.text.clone(),
            original: d.original.clone(),
            disk_bytes: d.disk_bytes.clone(),
            newlines: d.newlines,
            bom: d.bom,
        })
        .collect();
    fs::create_dir_all(path.parent().ok_or("Missing draft directory")?)
        .map_err(|e| e.to_string())?;
    let bytes = serde_json::to_vec(&drafts).map_err(|e| e.to_string())?;
    atomic_write(path, &bytes).map_err(|e| format!("Couldn’t keep a recovery draft: {e}"))
}

pub fn restore() -> Result<Vec<Document>, String> {
    restore_at(&directory().join("drafts.json"))
}

pub fn restore_at(path: &std::path::Path) -> Result<Vec<Document>, String> {
    let bytes = match fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.to_string()),
    };
    let drafts: Vec<Draft> =
        serde_json::from_slice(&bytes).map_err(|e| format!("Couldn’t restore drafts: {e}"))?;
    Ok(drafts
        .into_iter()
        .enumerate()
        .map(|(i, d)| {
            let mut doc = Document::blank(i as u64 + 1);
            doc.path = d.path;
            doc.edit = rune::TextEdit::new(d.text);
            doc.edit.set_caret(0);
            doc.original = d.original;
            doc.disk_bytes = d.disk_bytes;
            doc.newlines = d.newlines;
            doc.bom = d.bom;
            doc
        })
        .collect())
}
