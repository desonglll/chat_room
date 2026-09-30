//! Last-seen privacy on the WebSocket read paths: the `auth_ok` snapshot and every
//! broadcast frame that reveals who is online.
//!
//! A viewer sees an owner's exact presence (online, `offline{last_seen}`, membership of
//! the connected-members list, the `user_status` frames of connect/disconnect) only when
//! both hold (Telegram's reciprocity):
//!
//! * the owner's `last_seen` rule admits the viewer, and
//! * the viewer's own `last_seen` rule admits the owner — hiding yours hides theirs.
//!
//! Everyone else gets [`obscured_status`] in the snapshot and never receives the owner's
//! live `user_status` frames or connected-member entries: those frames are emitted at the
//! instant of connecting and disconnecting, so their mere arrival would leak the exact
//! time the obscured tier is designed to hide.

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use uuid::Uuid;

use super::last_seen::{exact_status, obscured_status, LastSeenRecord};
use super::{decide, ExceptionEffect, PrivacyFacts, PrivacyKey, PrivacyTier};
use crate::models::{ChatMember, ChatMessage, UserStatusEntry};
use crate::state::{with_pool, AppState};

/// How long a live connection trusts its loaded rules. A rule change reaches already-open
/// sockets within this window; new connections see it immediately.
const PRESENCE_RULES_TTL: Duration = Duration::from_secs(15);

const LAST_SEEN: &str = "last_seen";

/// Everything about one viewer and one chat that the last-seen decision reads.
#[derive(Debug, Default)]
struct ViewerContext {
    viewer: Uuid,
    viewer_tier: PrivacyTier,
    /// The viewer's own exceptions, keyed by the account they name.
    viewer_exceptions: HashMap<Uuid, ExceptionEffect>,
    /// Exceptions other accounts made about the viewer, keyed by the owner.
    exceptions_about_viewer: HashMap<Uuid, ExceptionEffect>,
    contacts: HashSet<Uuid>,
    blocked_viewer: HashSet<Uuid>,
    blocked_by_viewer: HashSet<Uuid>,
    /// `last_seen` tiers of the chat's members; absent means everybody.
    owner_tiers: HashMap<Uuid, PrivacyTier>,
}

impl ViewerContext {
    fn exact_visible(&self, owner: Uuid) -> bool {
        if owner == self.viewer {
            return true;
        }
        let contacts = self.contacts.contains(&owner);
        let owner_admits_viewer = decide(PrivacyFacts {
            is_self: false,
            owner_blocked_viewer: self.blocked_viewer.contains(&owner),
            tier: self.owner_tiers.get(&owner).copied().unwrap_or_default(),
            exception: self.exceptions_about_viewer.get(&owner).copied(),
            are_contacts: contacts,
        });
        let viewer_admits_owner = decide(PrivacyFacts {
            is_self: false,
            owner_blocked_viewer: self.blocked_by_viewer.contains(&owner),
            tier: self.viewer_tier,
            exception: self.viewer_exceptions.get(&owner).copied(),
            are_contacts: contacts,
        });
        owner_admits_viewer && viewer_admits_owner
    }
}

fn effect_map(rows: Vec<(Uuid, String)>) -> HashMap<Uuid, ExceptionEffect> {
    rows.into_iter()
        .filter_map(|(id, effect)| ExceptionEffect::parse(&effect).map(|effect| (id, effect)))
        .collect()
}

impl AppState {
    async fn viewer_context(
        &self,
        viewer: Uuid,
        room_id: Uuid,
    ) -> Result<ViewerContext, sqlx::Error> {
        let viewer_tier = self.privacy_tier(viewer, PrivacyKey::LastSeen).await?;
        let (own, about, contacts, blocked_viewer, blocked_by_viewer, tiers) =
            with_pool!(self, |pool| {
                let own: Vec<(Uuid, String)> = sqlx::query_as(
                    "SELECT target_user_id, effect FROM user_privacy_exceptions \
                     WHERE user_id = $1 AND privacy_key = $2",
                )
                .bind(viewer)
                .bind(LAST_SEEN)
                .fetch_all(pool)
                .await?;
                let about: Vec<(Uuid, String)> = sqlx::query_as(
                    "SELECT user_id, effect FROM user_privacy_exceptions \
                     WHERE target_user_id = $1 AND privacy_key = $2",
                )
                .bind(viewer)
                .bind(LAST_SEEN)
                .fetch_all(pool)
                .await?;
                let contacts: Vec<Uuid> = sqlx::query_scalar(
                    "SELECT CASE WHEN user_low_id = $1 THEN user_high_id ELSE user_low_id END \
                     FROM friendships WHERE status = 'accepted' \
                       AND (user_low_id = $1 OR user_high_id = $1)",
                )
                .bind(viewer)
                .fetch_all(pool)
                .await?;
                let blocked_viewer: Vec<Uuid> =
                    sqlx::query_scalar("SELECT blocker_id FROM user_blocks WHERE blocked_id = $1")
                        .bind(viewer)
                        .fetch_all(pool)
                        .await?;
                let blocked_by_viewer: Vec<Uuid> =
                    sqlx::query_scalar("SELECT blocked_id FROM user_blocks WHERE blocker_id = $1")
                        .bind(viewer)
                        .fetch_all(pool)
                        .await?;
                let tiers: Vec<(Uuid, String)> = sqlx::query_as(
                    "SELECT rules.user_id, rules.tier FROM user_privacy_rules AS rules \
                     JOIN chat_members AS members ON members.user_id = rules.user_id \
                       AND members.room_id = $1 \
                     WHERE rules.privacy_key = $2",
                )
                .bind(room_id)
                .bind(LAST_SEEN)
                .fetch_all(pool)
                .await?;
                Ok::<_, sqlx::Error>((
                    own,
                    about,
                    contacts,
                    blocked_viewer,
                    blocked_by_viewer,
                    tiers,
                ))
            })?;
        Ok(ViewerContext {
            viewer,
            viewer_tier,
            viewer_exceptions: effect_map(own),
            exceptions_about_viewer: effect_map(about),
            contacts: contacts.into_iter().collect(),
            blocked_viewer: blocked_viewer.into_iter().collect(),
            blocked_by_viewer: blocked_by_viewer.into_iter().collect(),
            owner_tiers: tiers
                .into_iter()
                .map(|(id, tier)| (id, PrivacyTier::parse(&tier)))
                .collect(),
        })
    }

    /// The per-viewer `auth_ok.statuses` snapshot and connected-members list.
    ///
    /// Fails closed: when the rules cannot be read, every other account is obscured.
    pub(crate) async fn presence_snapshot(
        &self,
        viewer: Uuid,
        room_id: Uuid,
        connected: &[ChatMember],
        participants: &[ChatMember],
        now: DateTime<Utc>,
    ) -> (Vec<UserStatusEntry>, Vec<ChatMember>) {
        let context = match self.viewer_context(viewer, room_id).await {
            Ok(context) => context,
            Err(error) => {
                tracing::warn!("load last-seen privacy failed: {error}");
                ViewerContext {
                    viewer,
                    viewer_tier: PrivacyTier::Nobody,
                    ..ViewerContext::default()
                }
            }
        };
        let records: HashMap<Uuid, LastSeenRecord> = self
            .chat_last_seen_records(room_id)
            .await
            .unwrap_or_else(|error| {
                tracing::warn!("load chat last-seen failed: {error}");
                Vec::new()
            })
            .into_iter()
            .collect();
        let online: HashSet<Uuid> = connected.iter().map(|member| member.user_id).collect();
        let statuses = participants
            .iter()
            .map(|participant| {
                let owner = participant.user_id;
                let record = records.get(&owner);
                UserStatusEntry {
                    user_id: owner,
                    status: if context.exact_visible(owner) {
                        exact_status(record, online.contains(&owner))
                    } else {
                        obscured_status(record, now)
                    },
                }
            })
            .collect();
        let members = connected
            .iter()
            .filter(|member| context.exact_visible(member.user_id))
            .cloned()
            .collect();
        (statuses, members)
    }
}

/// Per-connection filter for outbound broadcast frames (`src/realtime/outbound.rs`).
pub(crate) struct PresenceFilter {
    viewer: Uuid,
    room_id: Uuid,
    context: Option<(ViewerContext, Instant)>,
}

impl PresenceFilter {
    pub(crate) fn new(viewer: Uuid, room_id: Uuid) -> Self {
        Self {
            viewer,
            room_id,
            context: None,
        }
    }

    async fn context(&mut self, state: &AppState) -> &ViewerContext {
        let stale = self
            .context
            .as_ref()
            .is_none_or(|(_, loaded)| loaded.elapsed() >= PRESENCE_RULES_TTL);
        if stale {
            let context = match state.viewer_context(self.viewer, self.room_id).await {
                Ok(context) => context,
                Err(error) => {
                    tracing::warn!("reload last-seen privacy failed: {error}");
                    ViewerContext {
                        viewer: self.viewer,
                        viewer_tier: PrivacyTier::Nobody,
                        ..ViewerContext::default()
                    }
                }
            };
            self.context = Some((context, Instant::now()));
        }
        &self.context.as_ref().expect("context loaded above").0
    }

    /// The frame as this viewer may see it, or `None` when it must not be delivered.
    pub(crate) async fn apply(
        &mut self,
        state: &AppState,
        message: ChatMessage,
    ) -> Option<ChatMessage> {
        match message {
            ChatMessage::UserStatusChanged { user_id, status } => {
                if user_id == self.viewer || self.context(state).await.exact_visible(user_id) {
                    Some(ChatMessage::UserStatusChanged { user_id, status })
                } else {
                    None
                }
            }
            ChatMessage::Presence {
                members,
                participants,
            } => {
                let context = self.context(state).await;
                Some(ChatMessage::Presence {
                    members: retain_visible(context, members),
                    participants,
                })
            }
            ChatMessage::System {
                content,
                members: Some(members),
                participants,
            } => {
                let context = self.context(state).await;
                Some(ChatMessage::System {
                    content,
                    members: Some(retain_visible(context, members)),
                    participants,
                })
            }
            other => Some(other),
        }
    }
}

fn retain_visible(context: &ViewerContext, mut members: Vec<ChatMember>) -> Vec<ChatMember> {
    members.retain(|member| context.exact_visible(member.user_id));
    members
}
