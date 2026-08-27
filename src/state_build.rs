//! Application state construction and production dependency wiring.

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use anyhow::{Context, Result};
use tokio::sync::RwLock;

use crate::admin_metrics::RuntimeMetrics;
use crate::ai::AiAssistant;
use crate::attachment_content::ContentHashLocks;
use crate::attachment_storage::{self, AttachmentStore};
use crate::attachments::upload_hashes::UploadHashTracker;
use crate::cache::RedisCache;
use crate::config::AppConfig;
use crate::knowledge::MessageIndex;
use crate::models::Room;
use crate::security::AuthRateLimits;
use crate::social::rate_limits::SocialRateLimits;
use crate::state::{AppState, RoomChannel, SELECT_ROOMS};
use crate::storage;
use crate::work_queue::WorkQueue;

impl AppState {
    /// Open a database file, creating it and applying migrations automatically.
    pub async fn open(database_path: &Path) -> Result<Self> {
        Self::open_with_config(database_path, &AppConfig::default()).await
    }

    /// Open a database with validated runtime settings.
    pub async fn open_with_config(database_path: &Path, config: &AppConfig) -> Result<Self> {
        let attachment_store = open_attachment_store(config).await?;
        let pool = storage::open_database(database_path, &attachment_store).await?;
        Self::from_pool(
            storage::DatabasePool::Sqlite(pool),
            config.max_upload_bytes()?,
            attachment_store,
            ai_assistant_for(config),
            config.clone(),
        )
        .await
    }

    pub async fn open_postgres(url: &str, config: &AppConfig) -> Result<Self> {
        let attachment_store = open_attachment_store(config).await?;
        let pool = storage::open_postgres_database(url, config.database.max_connections).await?;
        Self::from_pool(
            pool,
            config.max_upload_bytes()?,
            attachment_store,
            ai_assistant_for(config),
            config.clone(),
        )
        .await
    }

    /// Open the configured database or the default chat_rooms.db file.
    pub async fn load(storage_path: Option<String>) -> Result<Self> {
        let path = storage_path.as_deref().unwrap_or("chat_rooms.db");
        Self::open(Path::new(path)).await
    }

    /// Create an isolated in-memory database for tests.
    pub async fn new() -> Result<Self> {
        Self::new_with_config(&AppConfig::default()).await
    }

    /// Create an isolated in-memory database with explicit runtime settings.
    pub async fn new_with_config(config: &AppConfig) -> Result<Self> {
        let attachment_store = AttachmentStore::open(
            attachment_storage::test_directory(),
            gc_age(config),
            &config.attachments.oss,
        )
        .await?;
        let pool = storage::open_memory_database(&attachment_store).await?;
        Self::from_pool(
            storage::DatabasePool::Sqlite(pool),
            config.max_upload_bytes()?,
            attachment_store,
            ai_assistant_for(config),
            config.clone(),
        )
        .await
    }

    async fn from_pool(
        pool: storage::DatabasePool,
        max_upload_bytes: usize,
        attachment_store: AttachmentStore,
        ai_assistant: Option<AiAssistant>,
        config: AppConfig,
    ) -> Result<Self> {
        let loaded: Vec<Room> = match &pool {
            storage::DatabasePool::Sqlite(database) => {
                sqlx::query_as(SELECT_ROOMS).fetch_all(database).await
            }
            storage::DatabasePool::Postgres(database) => {
                sqlx::query_as(SELECT_ROOMS).fetch_all(database).await
            }
        }
        .context("load rooms from database")?;
        let redis_cache = if config.redis.enabled {
            match RedisCache::connect(&config.redis).await {
                Ok(cache) => {
                    tracing::info!("Redis session and message cache enabled");
                    Some(cache)
                }
                Err(error) => {
                    tracing::warn!("Redis unavailable; using database reads: {error:#}");
                    None
                }
            }
        } else {
            None
        };
        let auth_rate_limits = AuthRateLimits::new(&config.auth, redis_cache.clone());
        let message_index = match MessageIndex::connect(&config.vector_store).await {
            Ok(index) => index,
            Err(error) => {
                tracing::warn!(
                    "vector message index unavailable; semantic retrieval disabled: {error:#}"
                );
                None
            }
        };
        let mut rooms = HashMap::with_capacity(loaded.len());
        let mut channels = HashMap::with_capacity(loaded.len());
        for room in loaded {
            channels.insert(room.id, RoomChannel::new());
            rooms.insert(room.id, room);
        }

        let work_queue = WorkQueue::new(&config.work_queue);
        let state = Self {
            pool,
            rooms: RwLock::new(rooms),
            channels: RwLock::new(channels),
            members: RwLock::new(HashMap::new()),
            max_upload_bytes,
            attachment_store,
            content_hash_locks: ContentHashLocks::default(),
            upload_hashes: UploadHashTracker::default(),
            runtime_metrics: RuntimeMetrics::default(),
            action_cooldowns: RwLock::new(HashMap::new()),
            social_rate_limits: SocialRateLimits::default(),
            auth_rate_limits,
            ai_assistant,
            config,
            redis_cache,
            work_queue,
            ai_run_dispatcher_started: AtomicBool::new(false),
            message_index,
            message_index_worker_started: AtomicBool::new(false),
            backup_runtime: crate::state_backup::BackupRuntime::default(),
        };
        let imported = state
            .import_legacy_system_admins(&state.config.admin.usernames)
            .await
            .context("import legacy system administrators")?;
        if imported > 0 {
            tracing::warn!(
                imported,
                "imported deprecated admin.usernames entries as persistent administrators"
            );
        }
        state.backfill_attachment_content_hashes().await?;
        Ok(state)
    }
}

fn ai_assistant_for(config: &AppConfig) -> Option<AiAssistant> {
    config
        .ai
        .resolved_api_key()
        .map(|api_key| AiAssistant::new(&config.ai, api_key))
}

fn gc_age(config: &AppConfig) -> Duration {
    Duration::from_secs(
        config
            .uploads
            .abandoned_upload_gc_hours
            .saturating_mul(3600),
    )
}

async fn open_attachment_store(config: &AppConfig) -> Result<AttachmentStore> {
    AttachmentStore::open(
        &config.attachments.directory,
        gc_age(config),
        &config.attachments.oss,
    )
    .await
}
