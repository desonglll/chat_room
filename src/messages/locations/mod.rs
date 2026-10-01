//! TG-407: static and live locations.
//!
//! A location is an ordinary message (`media_kind = 'location'`) plus one `message_locations`
//! row; it goes through the normal post gate and `message.send`. A live location stores only its
//! latest point and accepts updates from its sender until `live_until` — after that (or after
//! the sender stops it) the server refuses updates, and since no trail is kept there is nothing
//! left to follow. Handlers live in `handlers.rs`.

pub mod handlers;

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, QueryBuilder};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::models::StoredMessage;
use crate::state::{with_pool, AppState};

pub const MEDIA_KIND_LOCATION: &str = "location";

/// A location as messages carry it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct MessageLocation {
    pub latitude: f64,
    pub longitude: f64,
    /// Radius of uncertainty in metres, as the sender's device reported it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accuracy_m: Option<f64>,
    /// Direction of travel in degrees (live locations only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heading: Option<i32>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub title: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub address: String,
    /// Present on live locations: when sharing ends (or ended).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub live_until: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
}

impl MessageLocation {
    pub fn is_live_at(&self, now: DateTime<Utc>) -> bool {
        self.live_until.is_some_and(|until| until > now)
    }
}

/// One point a sender reports (on send or as a live update).
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct LocationPoint {
    pub latitude: f64,
    pub longitude: f64,
    #[serde(default)]
    pub accuracy_m: Option<f64>,
    #[serde(default)]
    pub heading: Option<i32>,
}

impl LocationPoint {
    pub fn is_valid(&self) -> bool {
        self.latitude.is_finite()
            && self.longitude.is_finite()
            && (-90.0..=90.0).contains(&self.latitude)
            && (-180.0..=180.0).contains(&self.longitude)
            && self.accuracy_m.is_none_or(|accuracy| {
                accuracy.is_finite() && (0.0..=100_000.0).contains(&accuracy)
            })
            && self
                .heading
                .is_none_or(|heading| (0..360).contains(&heading))
    }
}

/// A live location currently being shared in a chat (the merged map view).
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct LiveLocationEntry {
    pub message_id: Uuid,
    pub sender_id: Option<Uuid>,
    pub sender: String,
    pub location: MessageLocation,
}

/// The outcome of a live update.
pub enum LiveUpdate {
    Updated(MessageLocation),
    /// No such live location from this sender in this chat.
    NotFound,
    /// It exists but sharing has ended: no more points are accepted.
    Ended,
}

#[derive(FromRow)]
struct LocationRow {
    message_id: Uuid,
    latitude: f64,
    longitude: f64,
    accuracy_m: Option<f64>,
    heading: Option<i32>,
    title: String,
    address: String,
    live_until: Option<DateTime<Utc>>,
    updated_at: DateTime<Utc>,
}

impl LocationRow {
    fn into_location(self) -> (Uuid, MessageLocation) {
        (
            self.message_id,
            MessageLocation {
                latitude: self.latitude,
                longitude: self.longitude,
                accuracy_m: self.accuracy_m,
                heading: self.heading,
                title: self.title,
                address: self.address,
                live_until: self.live_until,
                updated_at: self.updated_at,
            },
        )
    }
}

const LOCATION_COLUMNS: &str = "message_locations.message_id, message_locations.latitude, \
     message_locations.longitude, message_locations.accuracy_m, message_locations.heading, \
     message_locations.title, message_locations.address, message_locations.live_until, \
     message_locations.updated_at";

impl AppState {
    /// Attach locations to loaded messages (every loader runs this, like TG-410 contacts).
    pub(crate) async fn attach_message_locations(
        &self,
        messages: &mut [StoredMessage],
    ) -> Result<(), sqlx::Error> {
        let ids: Vec<Uuid> = messages
            .iter()
            .filter(|message| message.attachment.is_none())
            .map(|message| message.id)
            .collect();
        if ids.is_empty() {
            return Ok(());
        }
        let rows: Vec<LocationRow> = with_pool!(self, |pool| {
            let mut query = QueryBuilder::new(format!(
                "SELECT {LOCATION_COLUMNS} FROM message_locations WHERE message_id IN ("
            ));
            {
                let mut values = query.separated(", ");
                for id in &ids {
                    values.push_bind(*id);
                }
            }
            query.push(")");
            query.build_query_as().fetch_all(pool).await
        })?;
        let mut found: HashMap<Uuid, MessageLocation> =
            rows.into_iter().map(LocationRow::into_location).collect();
        for message in messages.iter_mut() {
            if let Some(location) = found.remove(&message.id) {
                message.media_kind = Some(MEDIA_KIND_LOCATION.to_string());
                if message.recalled_at.is_none() {
                    message.location = Some(location);
                }
            }
        }
        Ok(())
    }

    /// Store a location message; `None` when the sender is not an active member.
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn insert_location_message(
        &self,
        room_id: Uuid,
        sender_id: Uuid,
        sender_name: &str,
        point: &LocationPoint,
        title: &str,
        address: &str,
        live_until: Option<DateTime<Utc>>,
        reply_to: Option<Uuid>,
        topic_id: Option<Uuid>,
    ) -> Result<Option<Uuid>, sqlx::Error> {
        let message_id = Uuid::new_v4();
        let created_at = Utc::now();
        let reply_to = self
            .reply_preview(room_id, reply_to)
            .await?
            .map(|reply| reply.message_id);
        with_pool!(self, |pool| {
            async {
                let mut tx = pool.begin().await?;
                let inserted = sqlx::query(
                    "INSERT INTO messages (id, room_id, sender_id, sender, content, reply_to_id, \
                     media_kind, created_at, topic_id) \
                     SELECT $1, $2, $3, $4, $5, $6, $7, $8, $9 \
                     WHERE EXISTS (SELECT 1 FROM chat_members WHERE chat_members.room_id = $2 \
                       AND chat_members.user_id = $3 AND chat_members.status = 'active')",
                )
                .bind(message_id)
                .bind(room_id)
                .bind(sender_id)
                .bind(sender_name)
                .bind(title)
                .bind(reply_to)
                .bind(MEDIA_KIND_LOCATION)
                .bind(created_at)
                .bind(topic_id)
                .execute(&mut *tx)
                .await?
                .rows_affected()
                    > 0;
                if !inserted {
                    return Ok::<_, sqlx::Error>(None);
                }
                sqlx::query(
                    "INSERT INTO message_locations (message_id, latitude, longitude, accuracy_m, \
                     heading, title, address, live_until, updated_at) \
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
                )
                .bind(message_id)
                .bind(point.latitude)
                .bind(point.longitude)
                .bind(point.accuracy_m)
                .bind(point.heading)
                .bind(title)
                .bind(address)
                .bind(live_until)
                .bind(created_at)
                .execute(&mut *tx)
                .await?;
                tx.commit().await?;
                Ok(Some(message_id))
            }
            .await
        })
    }

    /// Move a live location (its sender only, while it is still live).
    pub(crate) async fn update_live_location(
        &self,
        room_id: Uuid,
        sender_id: Uuid,
        message_id: Uuid,
        point: &LocationPoint,
        stop: bool,
    ) -> Result<LiveUpdate, sqlx::Error> {
        let now = Utc::now();
        let changed = with_pool!(self, |pool| {
            sqlx::query(
                "UPDATE message_locations SET latitude = $1, longitude = $2, accuracy_m = $3, \
                   heading = $4, updated_at = $5, \
                   live_until = CASE WHEN $6 THEN $5 ELSE live_until END \
                 WHERE message_id = $7 AND live_until IS NOT NULL AND live_until > $5 \
                   AND EXISTS (SELECT 1 FROM messages WHERE messages.id = $7 \
                     AND messages.room_id = $8 AND messages.sender_id = $9 \
                     AND messages.recalled_at IS NULL)",
            )
            .bind(point.latitude)
            .bind(point.longitude)
            .bind(point.accuracy_m)
            .bind(point.heading)
            .bind(now)
            .bind(stop)
            .bind(message_id)
            .bind(room_id)
            .bind(sender_id)
            .execute(pool)
            .await
            .map(|result| result.rows_affected())
        })?;
        let current = self.location_of(room_id, message_id).await?;
        Ok(match (changed, current) {
            (0, Some((owner, _))) if owner == Some(sender_id) => LiveUpdate::Ended,
            (0, _) => LiveUpdate::NotFound,
            (_, Some((_, location))) => LiveUpdate::Updated(location),
            (_, None) => LiveUpdate::NotFound,
        })
    }

    /// A live location message's sender and current state, if it is one in `room_id`.
    async fn location_of(
        &self,
        room_id: Uuid,
        message_id: Uuid,
    ) -> Result<Option<(Option<Uuid>, MessageLocation)>, sqlx::Error> {
        let row: Option<LiveRow> = with_pool!(self, |pool| {
            sqlx::query_as(&format!(
                "SELECT messages.sender_id, messages.sender, {LOCATION_COLUMNS} \
                 FROM message_locations JOIN messages ON messages.id = message_locations.message_id \
                 WHERE message_locations.message_id = $1 AND messages.room_id = $2 \
                   AND messages.recalled_at IS NULL AND message_locations.live_until IS NOT NULL"
            ))
            .bind(message_id)
            .bind(room_id)
            .fetch_optional(pool)
            .await
        })?;
        Ok(row.map(|row| (row.sender_id, row.location.into_location().1)))
    }

    /// Every location still being shared live in `room_id`, newest first.
    pub(crate) async fn live_locations(
        &self,
        room_id: Uuid,
    ) -> Result<Vec<LiveLocationEntry>, sqlx::Error> {
        let rows: Vec<LiveRow> = with_pool!(self, |pool| {
            sqlx::query_as(&format!(
                "SELECT messages.sender_id, messages.sender, {LOCATION_COLUMNS} \
                 FROM message_locations JOIN messages ON messages.id = message_locations.message_id \
                 WHERE messages.room_id = $1 AND messages.recalled_at IS NULL \
                   AND message_locations.live_until > $2 \
                 ORDER BY message_locations.updated_at DESC"
            ))
            .bind(room_id)
            .bind(Utc::now())
            .fetch_all(pool)
            .await
        })?;
        Ok(rows
            .into_iter()
            .map(|row| {
                let (message_id, location) = row.location.into_location();
                LiveLocationEntry {
                    message_id,
                    sender_id: row.sender_id,
                    sender: row.sender,
                    location,
                }
            })
            .collect())
    }
}

#[derive(FromRow)]
struct LiveRow {
    sender_id: Option<Uuid>,
    sender: String,
    #[sqlx(flatten)]
    location: LocationRow,
}
