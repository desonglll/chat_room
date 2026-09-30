//! The member roster, paged by keyset so a 200 000-member supergroup is never read at once.
//!
//! Order: newest joiner first, ties broken by user id — `(joined_at, user_id) DESC`, served by
//! the index `chat_members_room_joined_idx (room_id, status, joined_at, user_id)`. The cursor
//! is the last row's key, so a page costs the same at the millionth row as at the first; an
//! `OFFSET` would scan every skipped row.

use chrono::{DateTime, TimeZone, Utc};
use sqlx::FromRow;
use uuid::Uuid;

use super::admin_models::{ChatMemberEntry, ChatMemberPage};
use super::restrictions::MemberRestriction;
use crate::state::{with_pool, AppState};

pub const DEFAULT_PAGE_SIZE: i64 = 50;
pub const MAX_PAGE_SIZE: i64 = 200;
/// Administrators and restricted members are listed whole, up to this many.
pub const MAX_SHORT_LIST: i64 = 200;

/// Which part of the roster to list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemberFilter {
    All,
    Admins,
    Restricted,
}

impl std::str::FromStr for MemberFilter {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "all" => Ok(MemberFilter::All),
            "admins" => Ok(MemberFilter::Admins),
            "restricted" => Ok(MemberFilter::Restricted),
            _ => Err(()),
        }
    }
}

/// The position after which the next page starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemberCursor {
    pub joined_at: DateTime<Utc>,
    pub user_id: Uuid,
}

impl MemberCursor {
    /// `<unix nanoseconds>.<user id, simple>` — URL-safe and exact to the nanosecond.
    pub fn encode(&self) -> String {
        let nanos = self.joined_at.timestamp_nanos_opt().unwrap_or(i64::MAX);
        format!("{nanos}.{}", self.user_id.simple())
    }

    pub fn decode(value: &str) -> Option<Self> {
        let (nanos, user_id) = value.split_once('.')?;
        Some(Self {
            joined_at: Utc.timestamp_nanos(nanos.parse().ok()?),
            user_id: Uuid::parse_str(user_id).ok()?,
        })
    }
}

/// What the viewer may see beyond the public fields.
#[derive(Debug, Clone, Copy)]
pub struct RosterVisibility {
    pub viewer_id: Uuid,
    pub sees_admin_rights: bool,
    pub sees_restrictions: bool,
}

#[derive(Debug, Clone, FromRow)]
pub struct MemberRow {
    pub user_id: Uuid,
    pub username: String,
    pub display_name: String,
    pub avatar_emoji: String,
    pub nickname: String,
    pub role: String,
    pub status: String,
    pub joined_at: Option<DateTime<Utc>>,
    pub custom_title: String,
}

const MEMBER_COLUMNS: &str = "SELECT members.user_id, users.username, users.display_name, \
     users.avatar_emoji, members.nickname, roles.name AS role, members.status, \
     members.joined_at, members.custom_title \
     FROM chat_members AS members \
     JOIN users ON users.id = members.user_id \
     JOIN chat_roles AS roles ON roles.id = members.role_id ";

impl AppState {
    /// One keyset page of active members, newest first. `limit` is clamped by the caller.
    pub async fn member_page_rows(
        &self,
        room_id: Uuid,
        after: Option<MemberCursor>,
        limit: i64,
    ) -> Result<Vec<MemberRow>, sqlx::Error> {
        let order = " ORDER BY members.joined_at DESC, members.user_id DESC LIMIT ";
        with_pool!(self, |pool| {
            match after {
                None => {
                    sqlx::query_as(&format!(
                        "{MEMBER_COLUMNS} WHERE members.room_id = $1 \
                         AND members.status = 'active'{order}$2"
                    ))
                    .bind(room_id)
                    .bind(limit)
                    .fetch_all(pool)
                    .await
                }
                Some(cursor) => {
                    sqlx::query_as(&format!(
                        "{MEMBER_COLUMNS} WHERE members.room_id = $1 \
                         AND members.status = 'active' \
                         AND (members.joined_at, members.user_id) < ($2, $3){order}$4"
                    ))
                    .bind(room_id)
                    .bind(cursor.joined_at)
                    .bind(cursor.user_id)
                    .bind(limit)
                    .fetch_all(pool)
                    .await
                }
            }
        })
    }

    /// The owner and every administrator, driven from the two role ids.
    pub async fn admin_rows(&self, room_id: Uuid) -> Result<Vec<MemberRow>, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query_as(&format!(
                "{MEMBER_COLUMNS} WHERE members.role_id IN \
                   (SELECT id FROM chat_roles WHERE room_id = $1 AND name IN ('owner', 'admin')) \
                 AND members.status = 'active' \
                 ORDER BY CASE roles.name WHEN 'owner' THEN 0 ELSE 1 END, \
                   members.joined_at, members.user_id LIMIT $2"
            ))
            .bind(room_id)
            .bind(MAX_SHORT_LIST)
            .fetch_all(pool)
            .await
        })
    }

    /// Active members with a restriction in force, driven from the restriction table.
    pub async fn restricted_rows(
        &self,
        room_id: Uuid,
        now: DateTime<Utc>,
    ) -> Result<Vec<MemberRow>, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query_as(&format!(
                "{MEMBER_COLUMNS} WHERE members.room_id = $1 AND members.status = 'active' \
                 AND members.user_id IN (SELECT user_id FROM chat_member_restrictions \
                   WHERE room_id = $2 AND (until IS NULL OR until > $3)) \
                 ORDER BY members.joined_at DESC, members.user_id DESC LIMIT $4"
            ))
            .bind(room_id)
            .bind(room_id)
            .bind(now)
            .bind(MAX_SHORT_LIST)
            .fetch_all(pool)
            .await
        })
    }

    /// One active member's row.
    pub async fn member_row(
        &self,
        room_id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<MemberRow>, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query_as(&format!(
                "{MEMBER_COLUMNS} WHERE members.room_id = $1 AND members.user_id = $2 \
                 AND members.status = 'active'"
            ))
            .bind(room_id)
            .bind(user_id)
            .fetch_optional(pool)
            .await
        })
    }

    /// A page of the roster for one viewer.
    pub async fn chat_member_page(
        &self,
        room_id: Uuid,
        filter: MemberFilter,
        after: Option<MemberCursor>,
        limit: i64,
        visibility: RosterVisibility,
    ) -> Result<ChatMemberPage, sqlx::Error> {
        let now = Utc::now();
        let (rows, next_cursor) = match filter {
            MemberFilter::All => {
                // One extra row tells whether another page exists without a COUNT.
                let mut rows = self.member_page_rows(room_id, after, limit + 1).await?;
                let more = rows.len() as i64 > limit;
                rows.truncate(limit as usize);
                let next = more
                    .then(|| rows.last())
                    .flatten()
                    .and_then(|row| {
                        row.joined_at.map(|joined_at| MemberCursor {
                            joined_at,
                            user_id: row.user_id,
                        })
                    })
                    .map(|cursor| cursor.encode());
                (rows, next)
            }
            MemberFilter::Admins => (self.admin_rows(room_id).await?, None),
            MemberFilter::Restricted => (self.restricted_rows(room_id, now).await?, None),
        };
        let items = self
            .decorate_member_rows(room_id, rows, visibility, now)
            .await?;
        Ok(ChatMemberPage { items, next_cursor })
    }

    /// Attach the fields that depend on who is looking: an administrator's rights and a
    /// member's restrictions. One query for the chat's restrictions, one per administrator.
    pub async fn decorate_member_rows(
        &self,
        room_id: Uuid,
        rows: Vec<MemberRow>,
        visibility: RosterVisibility,
        now: DateTime<Utc>,
    ) -> Result<Vec<ChatMemberEntry>, sqlx::Error> {
        let restrictions: Vec<(Uuid, MemberRestriction)> = if visibility.sees_restrictions
            || rows.iter().any(|row| row.user_id == visibility.viewer_id)
        {
            self.chat_restrictions_in_force(room_id, now).await?
        } else {
            Vec::new()
        };
        let mut entries = Vec::with_capacity(rows.len());
        for row in rows {
            let is_viewer = row.user_id == visibility.viewer_id;
            let admin_rights = if row.role == "admin" && (visibility.sees_admin_rights || is_viewer)
            {
                Some(self.chat_admin_rights(room_id, row.user_id).await?)
            } else {
                None
            };
            let restrictions = (visibility.sees_restrictions || is_viewer).then(|| {
                restrictions
                    .iter()
                    .filter(|(user_id, _)| *user_id == row.user_id)
                    .map(|(_, restriction)| restriction.clone())
                    .collect()
            });
            entries.push(ChatMemberEntry {
                user_id: row.user_id,
                username: row.username,
                display_name: row.display_name,
                avatar_emoji: row.avatar_emoji,
                nickname: row.nickname,
                role: row.role,
                status: row.status,
                joined_at: row.joined_at,
                custom_title: row.custom_title,
                admin_rights,
                restrictions,
            });
        }
        Ok(entries)
    }

    async fn chat_restrictions_in_force(
        &self,
        room_id: Uuid,
        now: DateTime<Utc>,
    ) -> Result<Vec<(Uuid, MemberRestriction)>, sqlx::Error> {
        let rows: Vec<(Uuid, String, Option<DateTime<Utc>>)> = with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT user_id, denied_permission_key, until FROM chat_member_restrictions \
                 WHERE room_id = $1 AND (until IS NULL OR until > $2) \
                 ORDER BY user_id, denied_permission_key",
            )
            .bind(room_id)
            .bind(now)
            .fetch_all(pool)
            .await
        })?;
        Ok(rows
            .into_iter()
            .map(|(user_id, permission_key, until)| {
                (
                    user_id,
                    MemberRestriction {
                        permission_key,
                        until,
                    },
                )
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cursor_round_trips_exactly() {
        let cursor = MemberCursor {
            joined_at: Utc.timestamp_nanos(1_790_000_000_123_456_789),
            user_id: Uuid::new_v4(),
        };
        assert_eq!(MemberCursor::decode(&cursor.encode()), Some(cursor));
    }

    #[test]
    fn a_malformed_cursor_is_rejected() {
        for value in ["", "123", "abc.def", "1.not-a-uuid"] {
            assert_eq!(MemberCursor::decode(value), None, "{value}");
        }
    }

    #[test]
    fn filters_parse_from_their_wire_names() {
        assert_eq!("admins".parse(), Ok(MemberFilter::Admins));
        assert!("everyone".parse::<MemberFilter>().is_err());
    }
}
