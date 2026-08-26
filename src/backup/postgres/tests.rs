use super::*;

#[test]
fn data_only_backup_allows_oss_but_file_backup_does_not() {
    let mut config = AppConfig::default();
    config.attachments.oss.enabled = true;
    assert!(validate_backup_config(&config, false).is_ok());
    assert!(validate_backup_config(&config, true).is_err());
}

#[test]
fn replacing_attachments_preserves_the_previous_visible_files() {
    let root = std::env::temp_dir().join(format!("chat-restore-{}", Uuid::new_v4()));
    let target = root.join("attachments");
    let source = root.join("restored");
    fs::create_dir_all(target.join("old")).unwrap();
    fs::create_dir_all(source.join("new")).unwrap();
    fs::create_dir_all(target.join(".backup-work")).unwrap();
    fs::write(target.join("old/file.txt"), b"old").unwrap();
    fs::write(source.join("new/file.txt"), b"new").unwrap();

    let previous = replace_attachment_files(&target, &source).unwrap();

    assert_eq!(fs::read(target.join("new/file.txt")).unwrap(), b"new");
    assert_eq!(fs::read(previous.join("old/file.txt")).unwrap(), b"old");
    assert!(target.join(".backup-work").is_dir());
    let _ = fs::remove_dir_all(root);
}
