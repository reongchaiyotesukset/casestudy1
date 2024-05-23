use anyhow::Result;
use gettextrs::gettext;
use serde::Deserialize;
use zeroize::{Zeroize, ZeroizeOnDrop};

use super::{Restorable, RestorableItem};
use crate::models::{Algorithm, Method, OTPUri, OTP};

#[derive(Deserialize)]
pub struct Bitwarden {
    items: Vec<BitwardenItem>,
}

#[derive(Deserialize)]
pub struct BitwardenItem {
    #[serde(rename = "name")]
    issuer: Option<String>,
    login: Option<BitwardenDetails>,
    #[serde(skip)]
    algorithm: Algorithm,
    #[serde(skip)]
    method: Method,
    #[serde(skip)]
    digits: Option<u32>,
    #[serde(skip)]
    period: Option<u32>,
    #[serde(skip)]
    counter: Option<u32>,
}

#[derive(Deserialize, ZeroizeOnDrop, Zeroize)]
struct BitwardenDetails {
    #[zeroize(skip)]
    username: Option<String>,
    totp: Option<String>,
}

impl RestorableItem for BitwardenItem {
    fn account(&self) -> String {
        if let Some(account) = self
            .login
            .as_ref()
            .and_then(|login| login.username.as_ref())
        {
            account.clone()
        } else {
            gettext("Unknown account")
        }
    }

    fn issuer(&self) -> String {
        if let Some(issuer) = self.issuer.clone() {
            issuer
        } else {
            gettext("Unknown issuer")
        }
    }

    fn secret(&self) -> String {
        self.login
            .as_ref()
            .unwrap()
            .totp
            .as_ref()
            .unwrap()
            .to_owned()
    }

    fn period(&self) -> Option<u32> {
        self.period
    }

    fn method(&self) -> Method {
        self.method
    }

    fn algorithm(&self) -> Algorithm {
        self.algorithm
    }

    fn digits(&self) -> Option<u32> {
        self.digits
    }

    fn counter(&self) -> Option<u32> {
        self.counter
    }
}

impl BitwardenItem {
    fn overwrite_with(&mut self, uri: OTPUri) {
        self.issuer = Some(uri.issuer());

        if let Some(ref mut login) = self.login {
            login.totp = Some(uri.secret());
            login.username = Some(uri.account());
        } else {
            self.login = Some(BitwardenDetails {
                username: Some(uri.account()),
                totp: Some(uri.secret()),
            });
        }

        self.algorithm = uri.algorithm();
        self.method = uri.method();
        self.digits = uri.digits();
        self.period = uri.period();
        self.counter = uri.counter();
    }
}

impl Restorable for Bitwarden {
    const ENCRYPTABLE: bool = false;
    const SCANNABLE: bool = false;
    const IDENTIFIER: &'static str = "bitwarden";
    type Item = BitwardenItem;

    fn title() -> String {
        // Translators: This is for restoring a backup from Bitwarden.
        gettext("_Bitwarden")
    }

    fn subtitle() -> String {
        gettext("From a plain-text JSON file")
    }

    fn restore_from_data(from: &[u8], _key: Option<&str>) -> Result<Vec<Self::Item>> {
        let bitwarden_root: Bitwarden = serde_json::de::from_slice(from)?;

        let mut items = Vec::new();

        for mut item in bitwarden_root.items {
            if let Some(ref mut login) = item.login {
                if let Some(ref totp) = login.totp {
                    if totp.starts_with("steam://") {
                        login.totp = Some(totp.trim_start_matches("steam://").to_owned());
                        item.algorithm = Algorithm::SHA1;
                        item.method = Method::Steam;
                        item.period = Some(OTP::STEAM_DEFAULT_PERIOD);
                        item.digits = Some(OTP::STEAM_DEFAULT_DIGITS);
                        items.push(item);
                    } else if let Ok(uri) = totp.parse::<OTPUri>() {
                        item.overwrite_with(uri);
                        items.push(item);
                    }
                }
            }
        }

        Ok(items)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parse() {
        let data = std::fs::read_to_string("./src/backup/tests/bitwarden.json").unwrap();
        let items = Bitwarden::restore_from_data(data.as_bytes(), None).unwrap();

        assert_eq!(items[0].account(), "Mason");
        assert_eq!(items[0].issuer(), "Deno");
        assert_eq!(items[0].secret(), "4SJHB4GSD43FZBAI7C2HLRJGPQ");
        assert_eq!(items[0].period(), Some(30));
        assert_eq!(items[0].method(), Method::TOTP);
        assert_eq!(items[0].algorithm(), Algorithm::SHA1);
        assert_eq!(items[0].digits(), Some(6));
        assert_eq!(items[0].counter(), None);

        assert_eq!(items[1].account(), "James");
        assert_eq!(items[1].issuer(), "SPDX");
        assert_eq!(items[1].secret(), "5OM4WOOGPLQEF6UGN3CPEOOLWU");
        assert_eq!(items[1].period(), Some(20));
        assert_eq!(items[1].method(), Method::TOTP);
        assert_eq!(items[1].algorithm(), Algorithm::SHA256);
        assert_eq!(items[1].digits(), Some(7));
        assert_eq!(items[1].counter(), None);

        assert_eq!(items[2].account(), "Elijah");
        assert_eq!(items[2].issuer(), "Airbnb");
        assert_eq!(items[2].secret(), "7ELGJSGXNCCTV3O6LKJWYFV2RA");
        assert_eq!(items[2].period(), Some(50));
        assert_eq!(items[2].method(), Method::TOTP);
        assert_eq!(items[2].algorithm(), Algorithm::SHA512);
        assert_eq!(items[2].digits(), Some(8));
        assert_eq!(items[2].counter(), None);

        assert_eq!(items[3].account(), "Unknown account");
        assert_eq!(items[3].issuer(), "Test Steam");
        assert_eq!(items[3].secret(), "JRZCL47CMXVOQMNPZR2F7J4RGI");
        assert_eq!(items[3].period(), Some(30));
        assert_eq!(items[3].method(), Method::Steam);
        assert_eq!(items[3].algorithm(), Algorithm::SHA1);
        assert_eq!(items[3].digits(), Some(5));
        assert_eq!(items[3].counter(), None);
    }
}
