//! Layout choices kept between launches, such as the sidebar's width, in
//! `ui-preferences.json` beside notslack's other data. Nothing here is
//! secret; a missing or unreadable file just means the defaults.

use std::path::PathBuf;

use serde_json::{json, Value};

const UI_PREFERENCES_FILE: &str = "ui-preferences.json";
const SIDEBAR_RATIO_KEY: &str = "sidebar_ratio_basis_points";

fn preferences_path() -> Option<PathBuf> {
    app_model::app_data_dir()
        .ok()
        .map(|dir| dir.join(UI_PREFERENCES_FILE))
}

fn read_preferences() -> Value {
    preferences_path()
        .and_then(|path| std::fs::read_to_string(path).ok())
        .and_then(|text| serde_json::from_str(&text).ok())
        .filter(Value::is_object)
        .unwrap_or_else(|| json!({}))
}

/// The sidebar width the person last dragged it to, as basis points of the
/// window width.
pub fn sidebar_ratio_basis_points() -> Option<u16> {
    read_preferences()
        .get(SIDEBAR_RATIO_KEY)?
        .as_u64()
        .and_then(|value| u16::try_from(value).ok())
        .filter(|value| (500..=9_500).contains(value))
}

pub fn save_sidebar_ratio_basis_points(ratio_basis_points: u16) {
    let Some(path) = preferences_path() else {
        return;
    };
    let mut preferences = read_preferences();
    preferences[SIDEBAR_RATIO_KEY] = json!(ratio_basis_points);
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Err(error) = std::fs::write(&path, preferences.to_string()) {
        eprintln!("notslack could not save the sidebar width: {error}");
    }
}
