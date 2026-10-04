use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppearanceMode {
    Light,
    #[default]
    Dark,
}


impl gpui::Global for AppearanceMode {}

impl AppearanceMode {
    /// The window appearance the host shell last applied.
    
    pub fn current(cx: &gpui::App) -> Self {
        *cx.global::<Self>()
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }

    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "light" => Ok(Self::Light),
            "dark" => Ok(Self::Dark),
            _ => Err(format!("unsupported appearance mode {value}")),
        }
    }
}
