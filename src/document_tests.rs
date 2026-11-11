use super::*;

#[test]
fn edit_roundtrip_preserves_bom_crlf_permissions_and_symlinks() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("日本語 notes.md");
    let link = dir.path().join("linked.md");
    fs::write(&path, "\u{feff}# Hello\r\n\r\nText ✨\r\n").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
    symlink(&path, &link).unwrap();
    let mut doc = Document::open(1, &link).unwrap();
    assert_eq!(doc.edit.text, "# Hello\n\nText ✨\n");
    doc.edit.set_caret(doc.edit.text.len());
    doc.edit.insert("Another line\n");
    doc.save(&link).unwrap();
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        "\u{feff}# Hello\r\n\r\nText ✨\r\nAnother line\r\n"
    );
    assert!(
        fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o640
    );
    assert!(!doc.dirty());
}

#[test]
fn external_changes_are_not_overwritten_and_save_as_preserves_both_versions() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("notes.md");
    let copy = dir.path().join("my edits.md");
    fs::write(&path, "original").unwrap();
    let mut doc = Document::open(1, &path).unwrap();
    doc.edit.set_selection(0, 8);
    doc.edit.insert("my edit");
    fs::write(&path, "changed by another editor").unwrap();
    assert!(doc.save(&path).unwrap_err().contains("changed on disk"));
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        "changed by another editor"
    );
    assert_eq!(doc.edit.text, "my edit");
    doc.save(&copy).unwrap();
    assert_eq!(fs::read_to_string(&copy).unwrap(), "my edit");
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        "changed by another editor"
    );
}

#[test]
fn external_changes_reload_clean_documents_but_never_replace_unsaved_edits() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("live.md");
    fs::write(&path, "# First").unwrap();
    let mut doc = Document::open(1, &path).unwrap();
    fs::write(&path, "# Changed externally").unwrap();
    assert!(doc.reload_if_changed().unwrap());
    assert_eq!(doc.edit.text, "# Changed externally");
    doc.edit.insert("my edit ");
    fs::write(&path, "# Another external version").unwrap();
    assert!(!doc.reload_if_changed().unwrap());
    assert_eq!(doc.edit.text, "my edit # Changed externally");
    assert!(doc.save(&path).is_err());
}

#[test]
fn malformed_or_binary_files_are_rejected_without_lossy_decoding() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bad.md");
    fs::write(&path, [0xff, 0xfe, 0x41]).unwrap();
    assert!(Document::open(1, &path).err().unwrap().contains("UTF-8"));
    fs::write(&path, b"# Header\0binary").unwrap();
    assert!(Document::open(1, &path).err().unwrap().contains("binary"));
}

#[test]
fn relative_links_decode_spaces_and_unicode_and_reject_non_file_schemes() {
    let base = Path::new("/tmp/my docs/readme.md");
    assert_eq!(
        local_link(Some(base), "../images/a%20b.png"),
        Some(PathBuf::from("/tmp/images/a b.png"))
    );
    assert_eq!(
        local_link(Some(base), "%E6%97%A5.md#heading"),
        Some(PathBuf::from("/tmp/my docs/日.md"))
    );
    assert_eq!(local_link(Some(base), "javascript:alert(1)"), None);
    assert_eq!(
        local_link(Some(base), "https://example.com/image.png"),
        None
    );
}

#[test]
fn recovery_retains_unsaved_new_and_existing_files_and_the_conflict_baseline() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("saved.md");
    let recovery = dir.path().join("session/drafts.json");
    fs::write(&path, "original").unwrap();
    let mut saved = Document::open(1, &path).unwrap();
    saved.edit.insert("edited ");
    let mut new = Document::blank(2);
    new.edit.insert("new draft 📝");
    let clean = Document::open(3, &path).unwrap();
    crate::session::save_at(&recovery, &[saved, new, clean]).unwrap();
    fs::write(&path, "external edit").unwrap();
    let mut restored = crate::session::restore_at(&recovery).unwrap();
    assert_eq!(restored.len(), 2);
    assert_eq!(restored[0].edit.text, "edited original");
    assert_eq!(restored[1].edit.text, "new draft 📝");
    assert!(restored[0].save(&path).is_err());
    assert_eq!(fs::read_to_string(path).unwrap(), "external edit");
    // Explicitly discarding all drafts removes them from the next recovery.
    crate::session::save_at(&recovery, &[]).unwrap();
    assert!(crate::session::restore_at(&recovery).unwrap().is_empty());
}
