use serde::Deserialize;
use std::{error::Error, fs};

/// Read `./quick-settings.yaml` relative to the working directory.
pub(crate) fn load_config() -> Result<QuickSettingsConfig, Box<dyn Error>> {
    let contents = fs::read_to_string("./quick-settings.yaml")?;
    Ok(yaml_serde::from_str(&contents)?)
}

/// Quick-settings tiles in left-to-right, top-to-bottom order.
#[derive(Debug, Deserialize)]
pub(crate) struct QuickSettingsConfig {
    pub(crate) tiles: Vec<TileName>,
}

/// Supported tile names in `quick-settings.yaml`.
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
    Blank,
}
