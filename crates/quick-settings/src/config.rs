//! File-backed configuration used while constructing the quick-settings tiles.

use serde::Deserialize;
use std::{error::Error, fs};

/// Capacity of the current two-row, four-column grid.
pub(crate) const TILE_COUNT: usize = 8;

/// Read and validate `./quick-settings.yaml` relative to the working directory.
///
/// File errors, invalid YAML, unknown tile names and lists exceeding the grid's
/// capacity are returned to the caller. The bottom bar handles logging and the
/// blank fallback; it also pads shorter lists when spawning the tiles.
pub(crate) fn load_config() -> Result<QuickSettingsConfig, Box<dyn Error>> {
    let contents = fs::read_to_string("./quick-settings.yaml")?;
    let config: QuickSettingsConfig = yaml_serde::from_str(&contents)?;

    // Reject overflow rather than silently dropping tiles until pages exist.
    if config.tiles.len() > TILE_COUNT {
        return Err(format!(
            "quick settings supports at most {TILE_COUNT} tiles, got {}",
            config.tiles.len()
        )
        .into());
    }
    Ok(config)
}

/// Temporary configuration, consumed and dropped during tile construction.
#[derive(Debug, Deserialize)]
pub(crate) struct QuickSettingsConfig {
    /// Tile slots in left-to-right, top-to-bottom order, including explicit blanks.
    pub(crate) tiles: Vec<TileName>,
}

/// Supported tile names in `quick-settings.yaml`.
///
/// Serde maps snake_case names such as `keep_awake` to these variants and rejects
/// unrecognized names. Rendering each variant is the bottom bar's responsibility.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TileName {
    Wifi,
    Bluetooth,
    KeepAwake,
    Terminal,
    Record,
    Settings,
    Rotate,
    Battery,
    /// Reserve a grid slot without an icon, label or interaction.
    Blank,
}

#[cfg(test)]
mod tests;
