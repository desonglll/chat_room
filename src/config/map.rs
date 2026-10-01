//! TG-407 map tiles (D-010): Leaflet in the browser draws tiles from this URL template. The
//! default is OpenStreetMap's public tile server; a deployment that must not send viewers'
//! map views to a third party points it at its own tile server.

use anyhow::{bail, Result};
use serde::Deserialize;

pub const DEFAULT_TILE_URL: &str = "https://tile.openstreetmap.org/{z}/{x}/{y}.png";
pub const DEFAULT_ATTRIBUTION: &str = "© OpenStreetMap contributors";

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct MapConfig {
    /// `{z}`, `{x}` and `{y}` are substituted per tile (`{s}` subdomains are allowed too).
    pub tile_url: String,
    /// Shown on every map, as the tile licence requires.
    pub attribution: String,
}

impl Default for MapConfig {
    fn default() -> Self {
        Self {
            tile_url: DEFAULT_TILE_URL.into(),
            attribution: DEFAULT_ATTRIBUTION.into(),
        }
    }
}

impl MapConfig {
    pub(super) fn validate(&self) -> Result<()> {
        let url = self.tile_url.trim();
        if !(url.starts_with("https://") || url.starts_with("http://") || url.starts_with('/')) {
            bail!("map.tile_url must be an http(s) URL or a same-origin path");
        }
        if !["{z}", "{x}", "{y}"].iter().all(|part| url.contains(part)) {
            bail!("map.tile_url must contain {{z}}, {{x}} and {{y}}");
        }
        if self.attribution.trim().is_empty() {
            bail!("map.attribution must not be empty");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_is_osm_and_a_template_must_carry_the_tile_coordinates() {
        assert!(MapConfig::default().validate().is_ok());
        let bad = MapConfig {
            tile_url: "https://tiles.example/{z}/{x}.png".into(),
            ..MapConfig::default()
        };
        assert!(bad.validate().is_err());
        let local = MapConfig {
            tile_url: "/tiles/{z}/{x}/{y}.png".into(),
            ..MapConfig::default()
        };
        assert!(local.validate().is_ok());
        let scheme = MapConfig {
            tile_url: "javascript:{z}{x}{y}".into(),
            ..MapConfig::default()
        };
        assert!(scheme.validate().is_err());
    }
}
