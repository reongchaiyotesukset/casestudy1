use anyhow::Result;
use gettextrs::gettext;
use serde::Deserialize;
use zeroize::{Zeroize, ZeroizeOnDrop};

use super::{Restorable, RestorableItem};
use crate::models::{Algorithm, Method};

#[derive(Deserialize)]
pub struct FreeOTPJSON {
    tokens: Vec<FreeOTPItem>,
}

#[derive(Deserialize, Zeroize, ZeroizeOnDrop)]
pub struct FreeOTPItem {
    #[zeroize(skip)]
    algo: Algorithm,
    // Note: For some reason FreeOTP adds -1 to the counter
    #[zeroize(skip)]
    counter: Option<u32>,
    #[zeroize(skip)]
    digits: Option<u32>,
    #[zeroize(skip)]
    label: String,
    #[serde(rename = "issuerExt")]
    #[zeroize(skip)]
    issuer: String,
    #[zeroize(skip)]
    period: Option<u32>,
    secret: Vec<i16>,
    #[serde(rename = "type")]
    #[zeroize(skip)]
    method: Method,
}

impl RestorableItem for FreeOTPItem {
    fn account(&self) -> String {
        self.label.clone()
    }

    fn issuer(&self) -> String {
        self.issuer.clone()
    }

    fn secret(&self) -> String {
        let secret = self
            .secret
            .iter()
            .map(|x| (x & 0xff) as u8)
            .collect::<Vec<_>>();
        data_encoding::BASE32_NOPAD.encode(&secret)
    }

    fn period(&self) -> Option<u32> {
        self.period
    }

    fn method(&self) -> Method {
        self.method
    }

    fn algorithm(&self) -> Algorithm {
        self.algo
    }

    fn digits(&self) -> Option<u32> {
        self.digits
    }

    fn counter(&self) -> Option<u32> {
        if self.method().is_event_based() {
            // for some reason, FreeOTP adds -1 to the counter
            self.counter.map(|c| c + 1)
        } else {
            None
        }
    }
}

impl Restorable for FreeOTPJSON {
    const ENCRYPTABLE: bool = false;
    const SCANNABLE: bool = false;
    const IDENTIFIER: &'static str = "freeotp_json";
    type Item = FreeOTPItem;

    fn title() -> String {
        gettext("FreeOTP+")
    }

    fn subtitle() -> String {
        gettext("From a plain-text JSON file, compatible with FreeOTP+")
    }

    fn restore_from_data(from: &[u8], _key: Option<&str>) -> Result<Vec<Self::Item>> {
        let root: FreeOTPJSON = serde_json::de::from_slice(from)?;
        Ok(root.tokens)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse() {
        let data = std::fs::read_to_string("./src/backup/tests/freeotp_json.json").unwrap();
        let items = FreeOTPJSON::restore_from_data(data.as_bytes(), None).unwrap();

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

        assert_eq!(items[3].account(), "James");
        assert_eq!(items[3].issuer(), "Issuu");
        assert_eq!(items[3].secret(), "YOOMIXWS5GN6RTBPUFFWKTW5M4");
        assert_eq!(items[3].period(), Some(30));
        assert_eq!(items[3].method(), Method::HOTP);
        assert_eq!(items[3].algorithm(), Algorithm::SHA1);
        assert_eq!(items[3].digits(), Some(6));
        assert_eq!(items[3].counter(), Some(1));

        assert_eq!(items[4].account(), "Benjamin");
        assert_eq!(items[4].issuer(), "Air Canada");
        assert_eq!(items[4].secret(), "KUVJJOM753IHTNDSZVCNKL7GII");
        assert_eq!(items[4].period(), Some(30));
        assert_eq!(items[4].method(), Method::HOTP);
        assert_eq!(items[4].algorithm(), Algorithm::SHA256);
        assert_eq!(items[4].digits(), Some(7));
        assert_eq!(items[4].counter(), Some(50));

        assert_eq!(items[5].account(), "Mason");
        assert_eq!(items[5].issuer(), "WWE");
        assert_eq!(items[5].secret(), "5VAML3X35THCEBVRLV24CGBKOY");
        assert_eq!(items[5].period(), Some(30));
        assert_eq!(items[5].method(), Method::HOTP);
        assert_eq!(items[5].algorithm(), Algorithm::SHA512);
        assert_eq!(items[5].digits(), Some(8));
        assert_eq!(items[5].counter(), Some(10300));
    }
}
