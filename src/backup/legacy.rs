use anyhow::Result;
use gettextrs::gettext;
use serde::Deserialize;
use zeroize::{Zeroize, ZeroizeOnDrop};

use super::{Restorable, RestorableItem};
use crate::models::{Algorithm, Method};

// Same as andOTP except uses the first tag for the issuer
#[derive(Deserialize, Zeroize, ZeroizeOnDrop)]
pub struct LegacyAuthenticator {
    pub secret: String,
    #[zeroize(skip)]
    pub label: String,
    #[zeroize(skip)]
    pub digits: u32,
    #[serde(rename = "type")]
    #[zeroize(skip)]
    pub method: Method,
    #[zeroize(skip)]
    pub algorithm: Algorithm,
    #[zeroize(skip)]
    pub thumbnail: String,
    #[zeroize(skip)]
    pub last_used: i64,
    #[zeroize(skip)]
    pub tags: Vec<String>,
    #[zeroize(skip)]
    pub period: u32,
}

impl Restorable for LegacyAuthenticator {
    const ENCRYPTABLE: bool = false;
    const SCANNABLE: bool = false;
    const IDENTIFIER: &'static str = "authenticator_legacy";
    type Item = Self;

    fn title() -> String {
        // Translators: this is for restoring a backup from the old Authenticator
        // release
        gettext("Au_thenticator (Legacy)")
    }

    fn subtitle() -> String {
        gettext("From a plain-text JSON file")
    }

    fn restore_from_data(from: &[u8], _key: Option<&str>) -> Result<Vec<Self::Item>> {
        serde_json::de::from_slice(from).map_err(From::from)
    }
}

impl RestorableItem for LegacyAuthenticator {
    fn account(&self) -> String {
        self.label.clone()
    }

    fn issuer(&self) -> String {
        self.tags
            .first()
            .cloned()
            .unwrap_or_else(|| "Default".to_string())
    }

    fn secret(&self) -> String {
        self.secret.clone()
    }

    fn period(&self) -> Option<u32> {
        Some(self.period)
    }

    fn method(&self) -> Method {
        self.method
    }

    fn algorithm(&self) -> Algorithm {
        self.algorithm
    }

    fn digits(&self) -> Option<u32> {
        Some(self.digits)
    }

    fn counter(&self) -> Option<u32> {
        None
    }
}
