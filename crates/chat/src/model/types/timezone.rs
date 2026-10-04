use std::{fmt, str::FromStr};

use chrono_tz::Tz;
use serde::{de::Error as _, Deserialize, Deserializer, Serialize, Serializer};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SlackIanaTimezone(Tz);

impl SlackIanaTimezone {
    pub fn as_chrono_tz(&self) -> Tz {
        self.0
    }
}

impl From<Tz> for SlackIanaTimezone {
    fn from(timezone: Tz) -> Self {
        Self(timezone)
    }
}

impl fmt::Display for SlackIanaTimezone {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl FromStr for SlackIanaTimezone {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        value
            .parse::<Tz>()
            .map(Self)
            .map_err(|error| error.to_string())
    }
}

impl Serialize for SlackIanaTimezone {
    fn serialize<SerializerType>(
        &self,
        serializer: SerializerType,
    ) -> Result<SerializerType::Ok, SerializerType::Error>
    where
        SerializerType: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for SlackIanaTimezone {
    fn deserialize<DeserializerType>(
        deserializer: DeserializerType,
    ) -> Result<Self, DeserializerType::Error>
    where
        DeserializerType: Deserializer<'de>,
    {
        String::deserialize(deserializer)?
            .parse()
            .map_err(DeserializerType::Error::custom)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackDmPeerLocalTimeContext {
    pub conversation_label: String,
    pub timezone: SlackIanaTimezone,
}
