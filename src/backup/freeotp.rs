use anyhow::Result;
use gettextrs::gettext;
use gtk::prelude::*;
use serde::{Deserialize, Serialize};

use super::{Backupable, Restorable};
use crate::models::{Account, OTPUri, Provider, ProvidersModel};

#[allow(clippy::upper_case_acronyms)]
#[derive(Serialize, Deserialize)]
pub struct FreeOTP;

impl Backupable for FreeOTP {
    const ENCRYPTABLE: bool = false;
    const IDENTIFIER: &'static str = "authenticator";

    fn title() -> String {
        gettext("_Authenticator")
    }

    fn subtitle() -> String {
        gettext("Into a plain-text file, compatible with FreeOTP+")
    }

    fn backup(model: &ProvidersModel, _key: Option<&str>) -> Result<Vec<u8>> {
        let mut items: Vec<String> = Vec::new();

        for i in 0..model.n_items() {
            let provider = model.item(i).and_downcast::<Provider>().unwrap();
            let accounts = provider.accounts_model();

            for j in 0..accounts.n_items() {
                let account = accounts.item(j).and_downcast::<Account>().unwrap();

                items.push(account.otp_uri().into());
            }
        }

        let content = items.join("\n");
        Ok(content.as_bytes().to_vec())
    }
}

impl Restorable for FreeOTP {
    const ENCRYPTABLE: bool = false;
    const SCANNABLE: bool = false;
    const IDENTIFIER: &'static str = "authenticator";
    type Item = OTPUri;

    fn title() -> String {
        gettext("A_uthenticator")
    }

    fn subtitle() -> String {
        gettext("From a plain-text file, compatible with FreeOTP+")
    }

    fn restore_from_data(from: &[u8], _key: Option<&str>) -> Result<Vec<Self::Item>> {
        let uris = String::from_utf8(from.into())?;

        let items = uris
            .split('\n')
            .filter_map(|uri| uri.parse::<OTPUri>().ok())
            .collect::<Vec<OTPUri>>();

        Ok(items)
    }
}

#[cfg(test)]
mod tests {
    use super::{super::RestorableItem, *};
    use crate::models::{Algorithm, Method};

    #[test]
    fn parse() {
        let data = std::fs::read_to_string("./src/backup/tests/plain.txt").unwrap();
        let items = FreeOTP::restore_from_data(data.as_bytes(), None).unwrap();

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
        assert_eq!(items[3].period(), None);
        assert_eq!(items[3].method(), Method::HOTP);
        assert_eq!(items[3].algorithm(), Algorithm::SHA1);
        assert_eq!(items[3].digits(), Some(6));
        assert_eq!(items[3].counter(), Some(1));

        assert_eq!(items[4].account(), "Benjamin");
        assert_eq!(items[4].issuer(), "Air Canada");
        assert_eq!(items[4].secret(), "KUVJJOM753IHTNDSZVCNKL7GII");
        assert_eq!(items[4].period(), None);
        assert_eq!(items[4].method(), Method::HOTP);
        assert_eq!(items[4].algorithm(), Algorithm::SHA256);
        assert_eq!(items[4].digits(), Some(7));
        assert_eq!(items[4].counter(), Some(50));

        assert_eq!(items[5].account(), "Mason");
        assert_eq!(items[5].issuer(), "WWE");
        assert_eq!(items[5].secret(), "5VAML3X35THCEBVRLV24CGBKOY");
        assert_eq!(items[5].period(), None);
        assert_eq!(items[5].method(), Method::HOTP);
        assert_eq!(items[5].algorithm(), Algorithm::SHA512);
        assert_eq!(items[5].digits(), Some(8));
        assert_eq!(items[5].counter(), Some(10300));

        assert_eq!(items[6].account(), "Sophia");
        assert_eq!(items[6].issuer(), "Boeing");
        assert_eq!(items[6].secret(), "JRZCL47CMXVOQMNPZR2F7J4RGI");
        assert_eq!(items[6].period(), Some(30));
        assert_eq!(items[6].method(), Method::Steam);
        assert_eq!(items[6].algorithm(), Algorithm::SHA1);
        assert_eq!(items[6].digits(), Some(5));
        assert_eq!(items[6].counter(), None);
    }
}
