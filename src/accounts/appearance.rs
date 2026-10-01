//! TG-507 chat wallpapers, per account: one for every chat (`global`) and optional overrides
//! per chat. Presets, colours and gradients are pure data the client renders through the
//! wallpaper tokens; an uploaded image is stored like an avatar and served only to its owner.
//! Theme, accent and night schedule are device settings (client `settingsStore`), not here.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::state::{with_pool, AppState};

pub const GLOBAL_SCOPE: &str = "global";
pub const MAX_DIM: i32 = 80;
/// The built-in presets (`packages/ui/src/tokens/themes/wallpapers.css`).
pub const PRESETS: [&str; 14] = [
    "none",
    "default",
    "sunset",
    "meadow",
    "orchid",
    "lagoon",
    "haze",
    "forest-night",
    "midnight-blue",
    "deep-teal",
    "pine",
    "plum-night",
    "ember",
    "mulberry",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum WallpaperKind {
    Preset,
    Color,
    Gradient,
    Image,
}

impl WallpaperKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Preset => "preset",
            Self::Color => "color",
            Self::Gradient => "gradient",
            Self::Image => "image",
        }
    }

    fn parse(value: &str) -> Self {
        match value {
            "color" => Self::Color,
            "gradient" => Self::Gradient,
            "image" => Self::Image,
            _ => Self::Preset,
        }
    }
}

/// One wallpaper as the client applies it.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Wallpaper {
    /// `global` or a chat id.
    pub scope: String,
    pub kind: WallpaperKind,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub preset: String,
    /// `#rrggbb` colours: one for `color`, 2–4 for `gradient`.
    pub colors: Vec<String>,
    /// Owner-only image URL for `image` wallpapers (fetch with the session token).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_url: Option<String>,
    pub blur: bool,
    /// Darkening over the image, 0–80 %.
    pub dim: i32,
    pub updated_at: DateTime<Utc>,
}

/// A non-image wallpaper to store.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct WallpaperWrite {
    pub kind: WallpaperKind,
    #[serde(default)]
    pub preset: String,
    #[serde(default)]
    pub colors: Vec<String>,
    #[serde(default)]
    pub blur: bool,
    #[serde(default)]
    pub dim: i32,
}

fn is_hex_colour(value: &str) -> bool {
    value.len() == 7 && value.starts_with('#') && value[1..].chars().all(|c| c.is_ascii_hexdigit())
}

impl WallpaperWrite {
    /// Whether this write is well formed (an image is uploaded, never written as JSON).
    pub fn is_valid(&self) -> bool {
        let colours_ok = self.colors.iter().all(|colour| is_hex_colour(colour));
        (0..=MAX_DIM).contains(&self.dim)
            && colours_ok
            && match self.kind {
                WallpaperKind::Preset => {
                    PRESETS.contains(&self.preset.as_str()) && self.colors.is_empty()
                }
                WallpaperKind::Color => self.colors.len() == 1,
                WallpaperKind::Gradient => (2..=4).contains(&self.colors.len()),
                WallpaperKind::Image => false,
            }
    }
}

/// An uploaded image's stored file.
pub struct WallpaperImage {
    pub storage_key: String,
    pub mime_type: String,
    pub size_bytes: i64,
}

#[derive(FromRow)]
struct Row {
    scope: String,
    kind: String,
    preset: String,
    colors: String,
    image_key: Option<String>,
    blur: bool,
    dim: i32,
    updated_at: DateTime<Utc>,
}

impl Row {
    fn into_wallpaper(self) -> Wallpaper {
        let kind = WallpaperKind::parse(&self.kind);
        let version = self.updated_at.timestamp_millis();
        Wallpaper {
            image_url: (kind == WallpaperKind::Image && self.image_key.is_some())
                .then(|| format!("/api/users/me/wallpapers/{}/image?v={version}", self.scope)),
            scope: self.scope,
            kind,
            preset: self.preset,
            colors: serde_json::from_str(&self.colors).unwrap_or_default(),
            blur: self.blur,
            dim: self.dim,
            updated_at: self.updated_at,
        }
    }
}

impl AppState {
    pub async fn wallpapers(&self, user_id: Uuid) -> Result<Vec<Wallpaper>, sqlx::Error> {
        let rows: Vec<Row> = with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT scope, kind, preset, colors, image_key, blur, dim, updated_at \
                 FROM chat_wallpapers WHERE user_id = $1 ORDER BY scope",
            )
            .bind(user_id)
            .fetch_all(pool)
            .await
        })?;
        Ok(rows.into_iter().map(Row::into_wallpaper).collect())
    }

    /// Store a wallpaper for `scope`; returns the image key it replaced (for the caller to
    /// delete), if any.
    pub async fn put_wallpaper(
        &self,
        user_id: Uuid,
        scope: &str,
        write: &WallpaperWrite,
        image: Option<&WallpaperImage>,
    ) -> Result<(Wallpaper, Option<String>), sqlx::Error> {
        let previous = self
            .wallpaper_image(user_id, scope)
            .await?
            .map(|old| old.storage_key);
        let now = Utc::now();
        let colors = serde_json::to_string(&write.colors).unwrap_or_else(|_| "[]".into());
        let kind = if image.is_some() {
            WallpaperKind::Image
        } else {
            write.kind
        };
        with_pool!(self, |pool| {
            sqlx::query(
                "INSERT INTO chat_wallpapers \
                 (user_id, scope, kind, preset, colors, image_key, image_mime, image_size, blur, dim, updated_at) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11) \
                 ON CONFLICT (user_id, scope) DO UPDATE SET kind = excluded.kind, \
                   preset = excluded.preset, colors = excluded.colors, image_key = excluded.image_key, \
                   image_mime = excluded.image_mime, image_size = excluded.image_size, \
                   blur = excluded.blur, dim = excluded.dim, updated_at = excluded.updated_at",
            )
            .bind(user_id)
            .bind(scope)
            .bind(kind.as_str())
            .bind(&write.preset)
            .bind(&colors)
            .bind(image.map(|image| image.storage_key.clone()))
            .bind(image.map(|image| image.mime_type.clone()))
            .bind(image.map(|image| image.size_bytes))
            .bind(write.blur)
            .bind(write.dim)
            .bind(now)
            .execute(pool)
            .await
            .map(|_| ())
        })?;
        let replaced = previous.filter(|old| Some(old) != image.map(|image| &image.storage_key));
        let stored = self
            .wallpapers(user_id)
            .await?
            .into_iter()
            .find(|wallpaper| wallpaper.scope == scope)
            .ok_or(sqlx::Error::RowNotFound)?;
        Ok((stored, replaced))
    }

    /// Remove `scope`'s wallpaper; returns the image key to delete, if it had one.
    pub async fn delete_wallpaper(
        &self,
        user_id: Uuid,
        scope: &str,
    ) -> Result<(bool, Option<String>), sqlx::Error> {
        let image = self
            .wallpaper_image(user_id, scope)
            .await?
            .map(|image| image.storage_key);
        let removed = with_pool!(self, |pool| {
            sqlx::query("DELETE FROM chat_wallpapers WHERE user_id = $1 AND scope = $2")
                .bind(user_id)
                .bind(scope)
                .execute(pool)
                .await
                .map(|result| result.rows_affected())
        })?;
        Ok((removed > 0, image))
    }

    pub async fn wallpaper_image(
        &self,
        user_id: Uuid,
        scope: &str,
    ) -> Result<Option<WallpaperImage>, sqlx::Error> {
        let row: Option<(Option<String>, Option<String>, Option<i64>)> =
            with_pool!(self, |pool| {
                sqlx::query_as(
                    "SELECT image_key, image_mime, image_size FROM chat_wallpapers \
                 WHERE user_id = $1 AND scope = $2",
                )
                .bind(user_id)
                .bind(scope)
                .fetch_optional(pool)
                .await
            })?;
        Ok(row.and_then(|(key, mime, size)| {
            Some(WallpaperImage {
                storage_key: key?,
                mime_type: mime?,
                size_bytes: size?,
            })
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(kind: WallpaperKind, preset: &str, colors: &[&str], dim: i32) -> WallpaperWrite {
        WallpaperWrite {
            kind,
            preset: preset.into(),
            colors: colors.iter().map(|colour| colour.to_string()).collect(),
            blur: false,
            dim,
        }
    }

    #[test]
    fn writes_are_checked_per_kind() {
        assert!(write(WallpaperKind::Preset, "sunset", &[], 0).is_valid());
        assert!(!write(WallpaperKind::Preset, "nope", &[], 0).is_valid());
        assert!(write(WallpaperKind::Color, "", &["#aabbcc"], 0).is_valid());
        assert!(!write(WallpaperKind::Color, "", &["red"], 0).is_valid());
        assert!(write(WallpaperKind::Gradient, "", &["#000000", "#ffffff"], 40).is_valid());
        assert!(!write(WallpaperKind::Gradient, "", &["#000000"], 0).is_valid());
        assert!(!write(WallpaperKind::Gradient, "", &["#000000", "#ffffff"], 81).is_valid());
        assert!(!write(WallpaperKind::Image, "", &[], 0).is_valid());
    }
}
