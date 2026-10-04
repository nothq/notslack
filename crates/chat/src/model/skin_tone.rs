use std::str::FromStr;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SlackSkinTone {
    #[default]
    Default,
    Light,
    MediumLight,
    Medium,
    MediumDark,
    Dark,
}

impl SlackSkinTone {
    pub const ALL: [Self; 6] = [
        Self::Default,
        Self::Light,
        Self::MediumLight,
        Self::Medium,
        Self::MediumDark,
        Self::Dark,
    ];

    pub fn preference_value(self) -> &'static str {
        match self {
            Self::Default => "1",
            Self::Light => "2",
            Self::MediumLight => "3",
            Self::Medium => "4",
            Self::MediumDark => "5",
            Self::Dark => "6",
        }
    }

    pub fn modifier(self) -> Option<u8> {
        match self {
            Self::Default => None,
            Self::Light => Some(2),
            Self::MediumLight => Some(3),
            Self::Medium => Some(4),
            Self::MediumDark => Some(5),
            Self::Dark => Some(6),
        }
    }
}

impl FromStr for SlackSkinTone {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "1" => Ok(Self::Default),
            "2" => Ok(Self::Light),
            "3" => Ok(Self::MediumLight),
            "4" => Ok(Self::Medium),
            "5" => Ok(Self::MediumDark),
            "6" => Ok(Self::Dark),
            _ => Err(format!(
                "Slack preferred_skin_tone returned unsupported value {value:?}"
            )),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackPreferredSkinTone {
    pub team_id: String,
    pub selection: Option<SlackSkinTone>,
}
