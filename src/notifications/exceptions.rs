//! TG-508: notification defaults per chat type and per-chat exceptions, and the ONE rule that
//! decides whether a chat notifies, with what preview and sound (`decide`, a pure function
//! with a test matrix). Web Push delivery asks it (`AppState::chat_push_decision`).
//!
//! Precedence, highest first:
//! 1. a timed mute (`chat_members.muted_until` in the future) — silent; it lifts by itself;
//! 2. the chat's exception `enabled`, when set;
//! 3. the chat's legacy level: `none` silent, `mentions` only for mentions/replies;
//! 4. the default of the chat's type (private / group / channel).
//!
//! Preview and sound: the exception's value when set, else the type default's.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::chats::chat_type::ChatType;
use crate::state::{with_pool, AppState};

/// Sounds the client offers; `none` means a silent notification.
pub const SOUNDS: [&str; 6] = ["default", "note", "chime", "pop", "bell", "none"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum NotificationScope {
    Private,
    Group,
    Channel,
}

impl NotificationScope {
    pub fn of(chat_type: ChatType) -> Self {
        match chat_type {
            ChatType::Private => Self::Private,
            ChatType::Channel => Self::Channel,
            _ => Self::Group,
        }
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Private => "private",
            Self::Group => "group",
            Self::Channel => "channel",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct NotificationDefaults {
    pub scope: NotificationScope,
    pub enabled: bool,
    pub preview: bool,
    pub sound: String,
}

impl NotificationDefaults {
    fn fallback(scope: NotificationScope) -> Self {
        Self {
            scope,
            enabled: true,
            preview: true,
            sound: "default".into(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct NotificationException {
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default)]
    pub preview: Option<bool>,
    #[serde(default)]
    pub sound: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotifyDecision {
    pub deliver: bool,
    pub preview: bool,
    pub sound: String,
}

/// The one rule (see the module docs). `mention` = the event mentions or replies to the viewer.
pub fn decide(
    now: DateTime<Utc>,
    level: &str,
    muted_until: Option<DateTime<Utc>>,
    exception: Option<&NotificationException>,
    default: &NotificationDefaults,
    mention: bool,
) -> NotifyDecision {
    let preview = exception.and_then(|e| e.preview).unwrap_or(default.preview);
    let sound = exception
        .and_then(|e| e.sound.clone())
        .unwrap_or_else(|| default.sound.clone());
    let deliver = if muted_until.is_some_and(|until| until > now) {
        false
    } else if let Some(enabled) = exception.and_then(|e| e.enabled) {
        enabled
    } else {
        match level {
            "none" => false,
            "mentions" => mention,
            _ => default.enabled,
        }
    };
    NotifyDecision {
        deliver,
        preview,
        sound,
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct NotificationExceptionView {
    pub chat_id: Uuid,
    pub chat_title: String,
    #[serde(flatten)]
    pub exception: NotificationException,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct NotificationSettings {
    pub defaults: Vec<NotificationDefaults>,
    pub exceptions: Vec<NotificationExceptionView>,
}

/// One `notification_exceptions` row: `(room_id, enabled, preview, sound)`.
pub(crate) type ExceptionRow = (Uuid, Option<bool>, Option<bool>, Option<String>);

impl AppState {
    pub(crate) async fn notification_defaults(
        &self,
        user_id: Uuid,
        scope: NotificationScope,
    ) -> Result<NotificationDefaults, sqlx::Error> {
        let row: Option<(bool, bool, String)> = with_pool!(self, |pool| {
            sqlx::query_as("SELECT enabled, preview, sound FROM notification_defaults WHERE user_id = $1 AND scope = $2")
                .bind(user_id)
                .bind(scope.as_str())
                .fetch_optional(pool)
                .await
        })?;
        Ok(row
            .map(|(enabled, preview, sound)| NotificationDefaults {
                scope,
                enabled,
                preview,
                sound,
            })
            .unwrap_or_else(|| NotificationDefaults::fallback(scope)))
    }

    async fn notification_exception(
        &self,
        user_id: Uuid,
        room_id: Uuid,
    ) -> Result<Option<NotificationException>, sqlx::Error> {
        let row: Option<(Option<bool>, Option<bool>, Option<String>)> = with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT enabled, preview, sound FROM notification_exceptions WHERE user_id = $1 AND room_id = $2",
            )
            .bind(user_id)
            .bind(room_id)
            .fetch_optional(pool)
            .await
        })?;
        Ok(row.map(|(enabled, preview, sound)| NotificationException {
            enabled,
            preview,
            sound,
        }))
    }

    /// Whether (and how) a chat event notifies `recipient_id`; `None` when not an active member.
    pub(crate) async fn chat_push_decision(
        &self,
        recipient_id: Uuid,
        room_id: Uuid,
        mention: bool,
    ) -> Result<Option<NotifyDecision>, sqlx::Error> {
        let member: Option<(String, Option<DateTime<Utc>>)> = with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT notification_level, muted_until FROM chat_members \
                 WHERE user_id = $1 AND room_id = $2 AND status = 'active'",
            )
            .bind(recipient_id)
            .bind(room_id)
            .fetch_optional(pool)
            .await
        })?;
        let Some((level, muted_until)) = member else {
            return Ok(None);
        };
        let scope = NotificationScope::of(
            self.chat(room_id)
                .await
                .map(|chat| chat.chat_type)
                .unwrap_or_default(),
        );
        let default = self.notification_defaults(recipient_id, scope).await?;
        let exception = self.notification_exception(recipient_id, room_id).await?;
        Ok(Some(decide(
            Utc::now(),
            &level,
            muted_until,
            exception.as_ref(),
            &default,
            mention,
        )))
    }
}

#[cfg(test)]
mod tests {
    use chrono::Duration;

    use super::*;

    /// `(level, muted_until, exception, default enabled, mention, deliver)`.
    type DecisionCase<'a> = (
        &'a str,
        Option<DateTime<Utc>>,
        Option<&'a NotificationException>,
        bool,
        bool,
        bool,
    );

    fn default(enabled: bool) -> NotificationDefaults {
        NotificationDefaults {
            scope: NotificationScope::Group,
            enabled,
            preview: true,
            sound: "default".into(),
        }
    }

    #[test]
    fn exceptions_outrank_defaults_and_timed_mutes_outrank_everything() {
        let now = Utc::now();
        let on = NotificationException {
            enabled: Some(true),
            ..Default::default()
        };
        let off = NotificationException {
            enabled: Some(false),
            ..Default::default()
        };
        // (level, muted_until, exception, default enabled, mention) -> deliver
        let cases: Vec<DecisionCase> = vec![
            ("all", None, None, true, false, true),
            ("all", None, None, false, false, false),
            ("all", None, Some(&on), false, false, true),
            ("all", None, Some(&off), true, false, false),
            ("none", None, None, true, false, false),
            ("none", None, Some(&on), true, false, true),
            ("mentions", None, None, true, false, false),
            ("mentions", None, None, true, true, true),
            (
                "all",
                Some(now + Duration::hours(1)),
                Some(&on),
                true,
                true,
                false,
            ),
            // An expired mute is no mute: the chat notifies again by itself.
            (
                "all",
                Some(now - Duration::seconds(1)),
                None,
                true,
                false,
                true,
            ),
        ];
        for (level, muted, exception, default_on, mention, want) in cases {
            let got = decide(now, level, muted, exception, &default(default_on), mention).deliver;
            assert_eq!(got, want, "level={level} muted={muted:?} exception={exception:?} default={default_on} mention={mention}");
        }
    }

    #[test]
    fn preview_and_sound_inherit_unless_overridden() {
        let now = Utc::now();
        let base = NotificationDefaults {
            scope: NotificationScope::Private,
            enabled: true,
            preview: false,
            sound: "chime".into(),
        };
        let inherited = decide(now, "all", None, None, &base, false);
        assert_eq!(
            (inherited.preview, inherited.sound.as_str()),
            (false, "chime")
        );
        let custom = NotificationException {
            preview: Some(true),
            sound: Some("none".into()),
            ..Default::default()
        };
        let overridden = decide(now, "all", None, Some(&custom), &base, false);
        assert_eq!(
            (overridden.preview, overridden.sound.as_str()),
            (true, "none")
        );
    }
}
