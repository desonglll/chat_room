//! Runtime configuration and infrastructure accessors for application state.

use std::time::Duration;

use sqlx::{PgPool, SqlitePool};

use crate::{
    admin_metrics::RuntimeMetrics, ai::AiAssistant, ai::AiRuntimeStatus,
    attachment_content::ContentHashLocks, attachment_storage::AttachmentStore,
    attachments::upload_hashes::UploadHashTracker, state::AppState, storage,
};

impl AppState {
    pub fn pool(&self) -> &SqlitePool {
        match &self.pool {
            storage::DatabasePool::Sqlite(pool) => pool,
            storage::DatabasePool::Postgres(_) => {
                panic!("SQLite pool requested for PostgreSQL state")
            }
        }
    }

    pub fn postgres_pool(&self) -> Option<&PgPool> {
        match &self.pool {
            storage::DatabasePool::Postgres(pool) => Some(pool),
            storage::DatabasePool::Sqlite(_) => None,
        }
    }

    pub(crate) fn database_pool(&self) -> &storage::DatabasePool {
        &self.pool
    }

    pub fn max_upload_bytes(&self) -> usize {
        self.max_upload_bytes
    }

    pub fn ai_enabled(&self) -> bool {
        self.ai_assistant.is_some()
    }

    pub fn ai_status(&self) -> AiRuntimeStatus {
        if self.ai_assistant.is_some() {
            AiRuntimeStatus::Ready
        } else if self.config.ai.enabled {
            AiRuntimeStatus::MissingCredentials
        } else {
            AiRuntimeStatus::Disabled
        }
    }

    pub(crate) fn ai_assistant(&self) -> Option<&AiAssistant> {
        self.ai_assistant.as_ref()
    }

    pub(crate) fn ai_max_context_messages(&self) -> usize {
        self.config.ai.max_context_messages
    }

    pub(crate) fn ai_analysis_context_messages(&self) -> usize {
        self.config.ai.analysis_context_messages.min(500)
    }

    pub(crate) fn ai_suggest_cooldown(&self) -> Duration {
        Duration::from_secs(self.config.ai.suggest_cooldown_secs)
    }

    pub(crate) fn realtime_config(&self) -> &crate::config::RealtimeConfig {
        &self.config.realtime
    }

    pub(crate) fn poke_cooldown(&self) -> Duration {
        Duration::from_secs(self.config.realtime.poke_cooldown_secs)
    }

    pub(crate) fn session_lifetime_days(&self) -> i64 {
        self.config.auth.session_lifetime_days
    }

    pub(crate) fn session_cache(&self) -> Option<&crate::cache::SessionCache> {
        self.session_cache.as_ref()
    }

    pub fn chunk_body_limit_bytes(&self) -> usize {
        self.config.chunk_size_bytes().unwrap_or(8 * 1024 * 1024)
    }

    pub fn attachment_store(&self) -> &AttachmentStore {
        &self.attachment_store
    }

    pub(crate) fn content_hash_locks(&self) -> &ContentHashLocks {
        &self.content_hash_locks
    }

    pub(crate) fn upload_hashes(&self) -> &UploadHashTracker {
        &self.upload_hashes
    }

    pub(crate) fn runtime_metrics(&self) -> &RuntimeMetrics {
        &self.runtime_metrics
    }

    pub(crate) fn database_backend(&self) -> &'static str {
        match &self.pool {
            storage::DatabasePool::Sqlite(_) => "sqlite",
            storage::DatabasePool::Postgres(_) => "postgres",
        }
    }

    pub(crate) fn is_system_admin(&self, username: &str) -> bool {
        self.config
            .admin
            .usernames
            .iter()
            .any(|allowed| allowed.eq_ignore_ascii_case(username))
    }

    pub(crate) fn orphan_retention_hours(&self) -> i64 {
        self.config.admin.orphan_retention_hours
    }

    pub(crate) fn deleted_room_retention_days(&self) -> i64 {
        self.config.admin.deleted_room_retention_days
    }
}
