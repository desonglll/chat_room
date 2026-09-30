//! `CHAT_ROOM_BACKUP_*` overrides for the `[backup]` section.

use super::value::{set_parsed, set_path, set_string};
use crate::config::AppConfig;

pub(super) fn apply(config: &mut AppConfig, value: &mut impl FnMut(&str) -> Option<String>) {
    let backup = &mut config.backup;
    set_parsed(&mut backup.enabled, value("CHAT_ROOM_BACKUP_ENABLED"));
    set_parsed(
        &mut backup.interval_minutes,
        value("CHAT_ROOM_BACKUP_INTERVAL_MINUTES"),
    );
    set_parsed(
        &mut backup.retention_count,
        value("CHAT_ROOM_BACKUP_RETENTION_COUNT"),
    );
    set_string(
        &mut backup.target_backend,
        value("CHAT_ROOM_BACKUP_TARGET_BACKEND"),
    );
    set_path(&mut backup.directory, value("CHAT_ROOM_BACKUP_DIRECTORY"));
    set_parsed(
        &mut backup.include_files,
        value("CHAT_ROOM_BACKUP_INCLUDE_FILES"),
    );
}
