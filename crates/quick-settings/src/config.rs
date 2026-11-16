use serde::Deserialize;

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
