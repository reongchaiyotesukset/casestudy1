use std::{str::FromStr, string::ToString};

use gettextrs::gettext;
use gtk::glib;
use ring::hmac;
use serde::{de::Deserializer, ser::Serializer, Deserialize, Serialize};

#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, Default, Eq, PartialEq, Clone, Copy, glib::Enum)]
#[repr(u32)]
#[enum_type(name = "OTPMethod")]
pub enum Method {
    #[enum_value(name = "TOTP")]
    #[default]
    TOTP = 0,
    #[enum_value(name = "HOTP")]
    HOTP = 1,
    Steam = 2,
}

impl Serialize for Method {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Method {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Self::from_str(&String::deserialize(deserializer)?).unwrap())
    }
}

impl From<u32> for Method {
    fn from(u: u32) -> Self {
        match u {
            1 => Self::HOTP,
            2 => Self::Steam,
            _ => Self::default(),
        }
    }
}

impl Method {
    pub fn is_time_based(self) -> bool {
        matches!(self, Self::TOTP | Self::Steam)
    }

    pub fn is_event_based(self) -> bool {
        matches!(self, Self::HOTP)
    }

    pub fn to_locale_string(self) -> String {
        match self {
            Self::HOTP => gettext("Counter-based"),
            Self::TOTP => gettext("Time-based"),
            // Translators: Steam refers to the gaming application by Valve.
            Self::Steam => gettext("Steam"),
        }
    }
}

impl FromStr for Method {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_ref() {
            "totp" | "otp" => Ok(Self::TOTP),
            "hotp" => Ok(Self::HOTP),
            "steam" => Ok(Self::Steam),
            _ => anyhow::bail!("Unsupported Method"),
        }
    }
}

impl ToString for Method {
    fn to_string(&self) -> String {
        match *self {
            Self::TOTP => "totp",
            Self::HOTP => "hotp",
            Self::Steam => "steam",
        }
        .to_string()
    }
}

#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, Default, Eq, PartialEq, Clone, Copy, glib::Enum)]
#[repr(u32)]
#[enum_type(name = "OTPAlgorithm")]
pub enum Algorithm {
    #[enum_value(name = "SHA1")]
    #[default]
    SHA1 = 0,
    #[enum_value(name = "SHA256")]
    SHA256 = 1,
    #[enum_value(name = "SHA512")]
    SHA512 = 2,
}

impl Serialize for Algorithm {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Algorithm {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Self::from_str(&String::deserialize(deserializer)?).unwrap())
    }
}

impl Algorithm {
    pub fn to_locale_string(self) -> String {
        match self {
            Self::SHA1 => gettext("SHA-1"),
            Self::SHA256 => gettext("SHA-256"),
            Self::SHA512 => gettext("SHA-512"),
        }
    }
}

impl FromStr for Algorithm {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_ref() {
            "sha1" => Ok(Self::SHA1),
            "sha256" => Ok(Self::SHA256),
            "sha512" => Ok(Self::SHA512),
            _ => anyhow::bail!("Unsupported HMAC-algorithm"),
        }
    }
}

impl ToString for Algorithm {
    fn to_string(&self) -> String {
        match *self {
            Self::SHA1 => "sha1",
            Self::SHA256 => "sha256",
            Self::SHA512 => "sha512",
        }
        .to_string()
    }
}

impl From<Algorithm> for hmac::Algorithm {
    fn from(h: Algorithm) -> Self {
        match h {
            Algorithm::SHA1 => hmac::HMAC_SHA1_FOR_LEGACY_USE_ONLY,
            Algorithm::SHA256 => hmac::HMAC_SHA256,
            Algorithm::SHA512 => hmac::HMAC_SHA512,
        }
    }
}

impl From<u32> for Algorithm {
    fn from(u: u32) -> Self {
        match u {
            1 => Self::SHA256,
            2 => Self::SHA512,
            _ => Self::default(),
        }
    }
}
