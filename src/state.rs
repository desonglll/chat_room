//! Shared chat state backed by SQLite with in-memory broadcast channels.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::{broadcast, RwLock};
use uuid::Uuid;

use crate::admin_metrics::RuntimeMetrics;
use crate::ai::AiAssistant;
use crate::attachment_content::ContentHashLocks;
use crate::attachment_storage::AttachmentStore;
use crate::attachments::upload_hashes::UploadHashTracker;
use crate::cache::RedisCache;
use crate::config::AppConfig;
use crate::knowledge::MessageIndex;
use crate::models::{Chat, ChatMember, ChatMessage, User};
use crate::security::AuthRateLimits;
use crate::social::rate_limits::SocialRateLimits;
use crate::storage;
use crate::work_queue::WorkQueue;

pub(crate) const SELECT_CHATS: &str = "SELECT id, chat_type, title, password_hash, \
     password_hash <> '' AS has_password, creator_user_id, join_policy, \
     avatar_emoji, description, username, access_hash, is_forum, linked_chat_id, \
     CAST(slow_mode_seconds AS BIGINT) AS slow_mode_seconds, \
     CAST(auto_delete_seconds AS BIGINT) AS auto_delete_seconds, \
     signatures_enabled, history_visible_to_new_members, \
     CAST(member_count AS BIGINT) AS member_count, \
     CAST(NULL AS TEXT) AS membership_status, CAST(NULL AS TEXT) AS membership_role, \
     CAST(0 AS BIGINT) AS unread_count, created_at FROM chats WHERE deleted_at IS NULL";

macro_rules! with_pool {
    ($state:expr, |$pool:ident| $body:block) => {
        match $state.database_pool() {
            $crate::storage::DatabasePool::Sqlite($pool) => $body,
            $crate::storage::DatabasePool::Postgres($pool) => $body,
        }
    };
}
pub(crate) use with_pool;

#[derive(Clone)]
pub(crate) enum ChatEvent {
    Message(Box<ChatMessage>),
    Disconnect { reason: String },
    DisconnectUser { user_id: Uuid, reason: String },
}

pub(crate) struct ChatChannel {
    tx: broadcast::Sender<ChatEvent>,
}

pub(crate) struct ConnectedMember {
    member: ChatMember,
    connections: usize,
}

impl ChatChannel {
    pub(crate) fn new() -> Self {
        let (tx, _) = broadcast::channel(256);
        Self { tx }
    }
}

/// Application state. SQLite is durable storage; the chat map is a read cache.
pub struct AppState {
    pub(crate) pool: storage::DatabasePool,
    pub(crate) chats: RwLock<HashMap<Uuid, Chat>>,
    pub(crate) channels: RwLock<HashMap<Uuid, ChatChannel>>,
    pub(crate) members: RwLock<HashMap<Uuid, HashMap<Uuid, ConnectedMember>>>,
    pub(crate) max_upload_bytes: usize,
    pub(crate) attachment_store: AttachmentStore,
    pub(crate) content_hash_locks: ContentHashLocks,
    pub(crate) upload_hashes: UploadHashTracker,
    pub(crate) runtime_metrics: RuntimeMetrics,
    /// Per-(chat, from, target) cooldown timestamps for rate-limited, ephemeral
    /// actions (poke, AI suggestions) that don't need database persistence.
    pub(crate) action_cooldowns: RwLock<HashMap<(Uuid, Uuid, Uuid), Instant>>,
    pub(crate) social_rate_limits: SocialRateLimits,
    pub(crate) auth_rate_limits: AuthRateLimits,
    pub(crate) ai_assistant: Option<AiAssistant>,
    pub(crate) config: AppConfig,
    pub(crate) redis_cache: Option<RedisCache>,
    pub(crate) work_queue: WorkQueue,
    pub(crate) ai_run_dispatcher_started: AtomicBool,
    pub(crate) ai_extraction_dispatcher_started: AtomicBool,
    pub(crate) message_index: Option<MessageIndex>,
    pub(crate) message_index_worker_started: AtomicBool,
    pub(crate) push_dispatcher_started: AtomicBool,
    pub(crate) backup_runtime: crate::state_backup::BackupRuntime,
}

impl AppState {
    /// Returns true (and starts a new cooldown window) if enough time has passed
    /// since the last time this exact (chat, from, target) action fired.
    pub(crate) async fn check_action_cooldown(
        &self,
        room_id: Uuid,
        from: Uuid,
        target: Uuid,
        window: Duration,
    ) -> bool {
        let key = (room_id, from, target);
        let now = Instant::now();
        let mut cooldowns = self.action_cooldowns.write().await;
        if let Some(last) = cooldowns.get(&key) {
            if now.duration_since(*last) < window {
                return false;
            }
        }
        cooldowns.insert(key, now);
        true
    }

    pub(crate) async fn cache_inserted_chat(&self, chat: Chat) {
        let id = chat.id;
        self.chats.write().await.insert(id, chat);
        self.channels.write().await.insert(id, ChatChannel::new());
    }

    pub(crate) async fn cache_updated_chat(&self, chat: Chat) {
        self.chats.write().await.insert(chat.id, chat);
    }

    pub(crate) async fn remove_cached_chat(&self, id: Uuid, reason: &str) {
        self.chats.write().await.remove(&id);
        self.disconnect_chat(id, reason).await;
    }

    /// Return chats in stable creation order, optionally filtered by exact name.
    pub async fn list_chats(&self, name: Option<&str>) -> Vec<Chat> {
        let chats = self.chats.read().await;
        let mut list: Vec<Chat> = chats
            .values()
            .filter(|chat| name.is_none_or(|wanted| chat.title == wanted))
            .cloned()
            .collect();
        list.sort_by(|left, right| {
            left.created_at
                .cmp(&right.created_at)
                .then_with(|| left.id.cmp(&right.id))
        });
        list
    }

    pub async fn chat(&self, id: Uuid) -> Option<Chat> {
        self.chats.read().await.get(&id).cloned()
    }

    pub(crate) async fn subscribe(&self, id: Uuid) -> Option<broadcast::Receiver<ChatEvent>> {
        self.channels
            .read()
            .await
            .get(&id)
            .map(|chat| chat.tx.subscribe())
    }

    pub async fn broadcast(&self, id: Uuid, message: ChatMessage) {
        if let Some(chat) = self.channels.read().await.get(&id) {
            let _ = chat.tx.send(ChatEvent::Message(Box::new(message)));
        }
    }

    /// Track unique accounts while allowing the same account to use multiple tabs.
    pub async fn member_connected(&self, room_id: Uuid, user: &User) -> (Vec<ChatMember>, bool) {
        let mut chats = self.members.write().await;
        let chat = chats.entry(room_id).or_default();
        let first_connection = !chat.contains_key(&user.id);
        let connected = chat.entry(user.id).or_insert_with(|| ConnectedMember {
            member: ChatMember {
                user_id: user.id,
                username: user.username.clone(),
                avatar_emoji: user.avatar_emoji.clone(),
            },
            connections: 0,
        });
        connected.connections += 1;
        (sorted_members(chat), first_connection)
    }

    /// Remove one socket and report whether the account fully left the chat.
    pub async fn member_disconnected(
        &self,
        room_id: Uuid,
        user_id: Uuid,
    ) -> (Vec<ChatMember>, bool) {
        let mut chats = self.members.write().await;
        let Some(chat) = chats.get_mut(&room_id) else {
            return (Vec::new(), false);
        };
        let fully_disconnected = match chat.get_mut(&user_id) {
            Some(connected) if connected.connections > 1 => {
                connected.connections -= 1;
                false
            }
            Some(_) => {
                chat.remove(&user_id);
                true
            }
            None => false,
        };
        let members = sorted_members(chat);
        if chat.is_empty() {
            chats.remove(&room_id);
        }
        (members, fully_disconnected)
    }

    pub async fn remove_connected_member(&self, room_id: Uuid, user_id: Uuid) -> Vec<ChatMember> {
        let mut chats = self.members.write().await;
        let Some(chat) = chats.get_mut(&room_id) else {
            return Vec::new();
        };
        chat.remove(&user_id);
        let members = sorted_members(chat);
        if chat.is_empty() {
            chats.remove(&room_id);
        }
        members
    }

    pub async fn connected_members(&self, room_id: Uuid) -> Vec<ChatMember> {
        self.members
            .read()
            .await
            .get(&room_id)
            .map(sorted_members)
            .unwrap_or_default()
    }

    pub async fn disconnect_chat_member(&self, id: Uuid, user_id: Uuid, reason: &str) {
        if let Some(chat) = self.channels.read().await.get(&id) {
            let _ = chat.tx.send(ChatEvent::DisconnectUser {
                user_id,
                reason: reason.to_string(),
            });
        }
    }

    pub async fn disconnect_all_chat_rooms(&self, reason: &str) {
        for chat in self.channels.read().await.values() {
            let _ = chat.tx.send(ChatEvent::Disconnect {
                reason: reason.to_string(),
            });
        }
    }

    /// Refresh a connected account in every chat and publish the new member snapshots.
    pub async fn publish_member_profile(&self, user: &User) {
        let snapshots = {
            let mut chats = self.members.write().await;
            let mut snapshots = Vec::new();
            for (room_id, members) in chats.iter_mut() {
                let Some(connected) = members.get_mut(&user.id) else {
                    continue;
                };
                connected.member.username = user.username.clone();
                connected.member.avatar_emoji = user.avatar_emoji.clone();
                snapshots.push((*room_id, sorted_members(members)));
            }
            snapshots
        };

        for (room_id, members) in snapshots {
            let participants = match self.chat_participants(room_id).await {
                Ok(participants) => participants,
                Err(error) => {
                    tracing::warn!(
                        "load chat participants for profile update failed: {}",
                        error
                    );
                    Vec::new()
                }
            };
            self.broadcast(
                room_id,
                ChatMessage::Presence {
                    members,
                    participants,
                },
            )
            .await;
        }
    }

    /// Close current chat sessions and install a fresh channel for future joins.
    pub async fn restart_chat_connections(&self, id: Uuid, reason: &str) {
        let previous = self.channels.write().await.insert(id, ChatChannel::new());
        if let Some(previous) = previous {
            let _ = previous.tx.send(ChatEvent::Disconnect {
                reason: reason.to_string(),
            });
        }
    }

    async fn disconnect_chat(&self, id: Uuid, reason: &str) {
        if let Some(chat) = self.channels.write().await.remove(&id) {
            let _ = chat.tx.send(ChatEvent::Disconnect {
                reason: reason.to_string(),
            });
        }
    }

    pub(crate) async fn online_counts(&self) -> (u64, u64) {
        let chats = self.members.read().await;
        let mut users = HashSet::new();
        let mut connections = 0u64;
        for members in chats.values() {
            for (user_id, connected) in members {
                users.insert(*user_id);
                connections += connected.connections as u64;
            }
        }
        (users.len() as u64, connections)
    }
}

/// Convenience alias used by axum handlers.
pub type SharedState = Arc<AppState>;

fn sorted_members(members: &HashMap<Uuid, ConnectedMember>) -> Vec<ChatMember> {
    let mut result: Vec<_> = members
        .values()
        .map(|connected| connected.member.clone())
        .collect();
    result.sort_by(|left, right| {
        left.username
            .to_lowercase()
            .cmp(&right.username.to_lowercase())
            .then_with(|| left.user_id.cmp(&right.user_id))
    });
    result
}
