//! Aegis Import/Export Module
//!
//! See <https://github.com/beemdevelopment/Aegis/blob/master/docs/vault.md> for a description of the
//! aegis vault format.
//!
//! This module does not convert all information from aegis (note, icon, group
//! are lost). When exporting to the aegis json format the icon, url, help url,
//! and tags are lost.
//!
//! Exported files by this module cannot be decrypted by the python script
//! provided in the aegis repository (<https://github.com/beemdevelopment/Aegis/blob/master/docs/decrypt.py>). However,
//! aegis android app is able to read the files! See line 173 for a discussion.

use aes_gcm::{aead::Aead, KeyInit};
use anyhow::{Context, Result};
use gettextrs::gettext;
use gtk::prelude::*;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, ZeroizeOnDrop};

use super::{Backupable, Restorable, RestorableItem};
use crate::models::{Account, Algorithm, Method, Provider, ProvidersModel};

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Aegis {
    Encrypted(AegisEncrypted),
    Plaintext(AegisPlainText),
}

/// Plaintext version of the JSON format.
#[derive(Debug, Serialize, Deserialize)]
pub struct AegisPlainText {
    version: u32,
    header: Header,
    db: Database,
}

impl Default for AegisPlainText {
    fn default() -> Self {
        Self {
            version: 1,
            header: Header {
                params: None,
                slots: Default::default(),
            },
            db: Default::default(),
        }
    }
}

/// Encrypted version of the JSON format. `db` is simply a base64 encoded string
/// with an encrypted AegisDatabase.
#[derive(Debug, Serialize, Deserialize)]
pub struct AegisEncrypted {
    version: u32,
    header: Header,
    db: String,
}

impl Default for Aegis {
    fn default() -> Self {
        Self::Plaintext(AegisPlainText::default())
    }
}

impl Aegis {
    pub fn add_item(&mut self, item: Item) {
        if let Self::Plaintext(plain_text) = self {
            plain_text.db.entries.push(item);
        } else {
            // This is an implementation error. Thus, panic is here okay.
            panic!("Trying to add an OTP item to an encrypted aegis database")
        }
    }

    pub fn encrypt(&mut self, password: &str) -> Result<()> {
        // Create a new master key
        let mut rng = rand::thread_rng();
        let mut master_key = [0u8; 32];
        rng.fill_bytes(&mut master_key);

        // Create a new header (including defaults for a password slot)
        let mut header = Header {
            params: Some(HeaderParam::default()),
            slots: Some(vec![HeaderSlot::default()]),
        };

        // We only support password encrypted database so far so we don't have to do any
        // checks for the slot type
        let password_slot = &mut header.slots.as_mut().unwrap().get_mut(0).unwrap();
        // Derive key from given password
        let mut derived_key: [u8; 32] = [0u8; 32];
        let params = scrypt::Params::new(
            // TODO log2 for u64 is not stable yet. Change this in the future.
            (password_slot.n() as f64).log2() as u8,
            password_slot.r(),
            password_slot.p(),
            scrypt::Params::RECOMMENDED_LEN,
        )
        // All parameters are default values. Thus, this should always work and unwrap is okay.
        .expect("Scrypt params creation");
        scrypt::scrypt(
            password.as_bytes(),
            password_slot.salt(),
            &params,
            &mut derived_key,
        )
        .map_err(|_| anyhow::anyhow!("Scrypt key derivation"))?;

        // Encrypt new master key with derived key
        let cipher = aes_gcm::Aes256Gcm::new_from_slice(&derived_key)?;
        let mut ciphertext: Vec<u8> = cipher
            .encrypt(
                aes_gcm::Nonce::from_slice(&password_slot.key_params.nonce),
                master_key.as_ref(),
            )
            .map_err(|_| anyhow::anyhow!("Encrypter master key"))?;

        // Add encrypted master key and tag to our password slot. If this assignment
        // fails, we have a mistake in our logic, thus unwrap is okay.
        password_slot.key_params.tag = ciphertext.split_off(32).try_into().unwrap();
        password_slot.key = ciphertext.try_into().unwrap();

        // Finally, we get the JSON string for the database and encrypt it.
        if let Self::Plaintext(plain_text) = self {
            let db_json: Vec<u8> = serde_json::ser::to_string_pretty(&plain_text.db)?
                .as_bytes()
                .to_vec();
            let cipher = aes_gcm::Aes256Gcm::new_from_slice(&master_key)?;
            let mut ciphertext: Vec<u8> = cipher
                .encrypt(
                    aes_gcm::Nonce::from_slice(&header.params.as_ref().unwrap().nonce),
                    db_json.as_ref(),
                )
                .map_err(|_| anyhow::anyhow!("Encrypting aegis database"))?;
            header.params.as_mut().unwrap().tag = ciphertext
                .split_off(ciphertext.len() - 16)
                .try_into()
                .unwrap();
            let db_encrypted = ciphertext;

            *self = Self::Encrypted(AegisEncrypted {
                version: plain_text.version,
                header,
                db: data_encoding::BASE64.encode(&db_encrypted),
            });
        } else {
            // This is an implementation error. Thus, panic is okay.
            panic!("Encrypt can only be called on a plaintext object.")
        }

        Ok(())
    }
}

/// Header of the Encrypted Aegis JSON File
///
/// Contains all necessary information for encrypting / decrypting the vault (db
/// field).
#[derive(Debug, Serialize, Deserialize)]
pub struct Header {
    #[serde(default)]
    pub slots: Option<Vec<HeaderSlot>>,
    #[serde(default)]
    pub params: Option<HeaderParam>,
}

/// Header Slots
///
/// Containts information to decrypt the master key.
#[derive(Debug, Serialize, Deserialize)]
pub struct HeaderSlot {
    // We are not interested in biometric slots at the moment. Thus, we omit these information.
    // However, in the future, authenticator app might be able to lock / unlock the database using
    // fingerprint sensors (see <https://gitlab.gnome.org/World/Authenticator/-/issues/106> for more
    // information). Thus, it might be possible to read also these biometric slots and unlock them
    // with a fingerprint reader used by authenticar. However, it would be ncessary that aegis
    // android app (thus the android system) and authenticator use the same mechanisms to derive
    // keys from biometric input. This has to be checked beforehand.
    //
    // TODO rename should be changed to `rename = 2`. However this does not work yet with serde,
    // see: <https://github.com/serde-rs/serde/issues/745>. This allows decrypting the exported file
    // with the python script provided in the aegis repository. The python script expects an
    // integer but we provide a string. Thus, change the string in header / slots / password
    // slot / `type = "1"` to `type = 1` to use the python script.
    #[serde(rename = "type")]
    pub type_: u32,
    pub uuid: String,
    #[serde(with = "hex::serde")]
    pub key: [u8; 32],
    // First tuple entry is the nonce, the second is the tag.
    pub key_params: HeaderParam,
    n: Option<u32>,
    r: Option<u32>,
    p: Option<u32>,
    #[serde(default, with = "hex::serde")]
    salt: [u8; 32],
}

impl HeaderSlot {
    pub fn n(&self) -> u32 {
        self.n.unwrap_or_else(|| 2_u32.pow(15))
    }

    pub fn r(&self) -> u32 {
        self.r.unwrap_or(8)
    }

    pub fn p(&self) -> u32 {
        self.p.unwrap_or(1)
    }

    pub fn salt(&self) -> &[u8; 32] {
        &self.salt
    }
}

impl Default for HeaderSlot {
    fn default() -> Self {
        let mut rng = rand::thread_rng();
        let mut salt = [0u8; 32];
        rng.fill_bytes(&mut salt);

        Self {
            type_: 1,
            uuid: uuid::Uuid::new_v4().to_string(),
            key: [0u8; 32],
            key_params: HeaderParam::default(),
            n: Some(2_u32.pow(15)),
            r: Some(8),
            p: Some(1),
            salt,
        }
    }
}

/// Parameters to Database Encryption
#[derive(Debug, Serialize, Deserialize)]
pub struct HeaderParam {
    #[serde(with = "hex::serde")]
    pub nonce: [u8; 12],
    #[serde(with = "hex::serde")]
    pub tag: [u8; 16],
}

impl Default for HeaderParam {
    fn default() -> Self {
        let mut rng = rand::thread_rng();
        let mut nonce = [0u8; 12];
        rng.fill_bytes(&mut nonce);

        Self {
            nonce,
            tag: [0u8; 16],
        }
    }
}

/// Contains All OTP Entries
#[derive(Debug, Serialize, Deserialize)]
pub struct Database {
    pub version: u32,
    pub entries: Vec<Item>,
}

impl Default for Database {
    fn default() -> Self {
        Self {
            version: 2,
            entries: std::vec::Vec::new(),
        }
    }
}

/// An OTP Entry
#[derive(Debug, Serialize, Deserialize)]
pub struct Item {
    #[serde(rename = "type")]
    pub method: Method,
    // UUID is omitted
    #[serde(rename = "name")]
    pub label: String,
    pub issuer: Option<String>,
    // TODO tags are not imported/exported right now.
    #[serde(rename = "group")]
    pub tags: Option<String>,
    // Note is omitted
    // Icon:
    // TODO: Aegis encodes icons as JPEG's encoded in Base64 with padding. Does authenticator
    // support this?
    // TODO tags are not imported/exported right now.
    #[serde(rename = "icon")]
    pub thumbnail: Option<String>,
    pub info: Detail,
}

impl Item {
    pub fn new(account: &Account) -> Self {
        let provider = account.provider();

        let mut detail = Detail {
            secret: account.otp().secret(),
            algorithm: provider.algorithm(),
            digits: provider.digits(),
            period: None,
            counter: None,
        };

        if provider.method().is_event_based() {
            detail.counter = Some(account.counter());
        } else {
            detail.period = Some(provider.period());
        }

        Self {
            method: provider.method(),
            label: account.name(),
            issuer: Some(provider.name()),
            tags: None,
            thumbnail: None,
            info: detail,
        }
    }

    pub fn fix_empty_issuer(&mut self) -> Result<()> {
        if self.issuer.is_none() {
            let mut vals: Vec<&str> = self.label.split('@').collect();
            if vals.len() > 1 {
                self.issuer = vals.pop().map(ToOwned::to_owned);
                self.label = vals.join("@");
            } else {
                anyhow::bail!("Entry {} has an empty issuer", self.label);
            }
        }
        Ok(())
    }
}

/// OTP Entry Details
#[derive(Debug, Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
pub struct Detail {
    pub secret: String,
    #[serde(rename = "algo")]
    #[zeroize(skip)]
    pub algorithm: Algorithm,
    #[zeroize(skip)]
    pub digits: u32,
    #[zeroize(skip)]
    pub period: Option<u32>,
    #[zeroize(skip)]
    pub counter: Option<u32>,
}

impl RestorableItem for Item {
    fn account(&self) -> String {
        self.label.clone()
    }

    fn issuer(&self) -> String {
        self.issuer
            .as_ref()
            .map(ToOwned::to_owned)
            .unwrap_or_default()
    }

    fn secret(&self) -> String {
        self.info.secret.clone()
    }

    fn period(&self) -> Option<u32> {
        self.info.period
    }

    fn method(&self) -> Method {
        self.method
    }

    fn algorithm(&self) -> Algorithm {
        self.info.algorithm
    }

    fn digits(&self) -> Option<u32> {
        Some(self.info.digits)
    }

    fn counter(&self) -> Option<u32> {
        self.info.counter
    }
}

impl Backupable for Aegis {
    const ENCRYPTABLE: bool = true;
    const IDENTIFIER: &'static str = "aegis";

    fn title() -> String {
        // Translators: This is for making a backup for the aegis Android app.
        gettext("Aegis")
    }

    fn subtitle() -> String {
        gettext("Into a JSON file containing plain-text or encrypted fields")
    }

    fn backup(model: &ProvidersModel, key: Option<&str>) -> Result<Vec<u8>> {
        // Create structure
        let mut aegis_root = Aegis::default();

        for i in 0..model.n_items() {
            let provider = model.item(i).and_downcast::<Provider>().unwrap();
            let accounts = provider.accounts_model();

            for j in 0..accounts.n_items() {
                let account = accounts.item(j).and_downcast::<Account>().unwrap();
                let otp_item = Item::new(&account);
                aegis_root.add_item(otp_item);
            }
        }

        if let Some(password) = key {
            aegis_root.encrypt(password)?;
        }

        let content = serde_json::ser::to_string_pretty(&aegis_root)?;

        Ok(content.as_bytes().to_vec())
    }
}

impl Restorable for Aegis {
    const ENCRYPTABLE: bool = true;
    const SCANNABLE: bool = false;
    const IDENTIFIER: &'static str = "aegis";
    type Item = Item;

    fn title() -> String {
        // Translators: This is for restoring a backup from the aegis Android app.
        gettext("Aegis")
    }

    fn subtitle() -> String {
        gettext("From a JSON file containing plain-text or encrypted fields")
    }

    fn restore_from_data(from: &[u8], key: Option<&str>) -> Result<Vec<Self::Item>> {
        // TODO check whether file / database is encrypted by aegis
        let aegis_root: Aegis = serde_json::de::from_slice(from)?;
        let mut items = Vec::new();

        // Check whether file is encrypted or in plaintext
        match aegis_root {
            Aegis::Plaintext(plain_text) => {
                tracing::info!(
                    "Found unencrypted aegis vault with version {} and database version {}.",
                    plain_text.version,
                    plain_text.db.version
                );

                // Check for correct aegis vault version and correct database version.
                if plain_text.version != 1 {
                    anyhow::bail!(
                        "Aegis vault version expected to be 1. Found {} instead.",
                        plain_text.version
                    );
                // There is no version 0. So this should be okay ...
                } else if plain_text.db.version > 2 {
                    anyhow::bail!(
                        "Aegis database version expected to be 1 or 2. Found {} instead.",
                        plain_text.db.version
                    );
                } else {
                    for mut item in plain_text.db.entries {
                        item.fix_empty_issuer()?;
                        items.push(item);
                    }
                    Ok(items)
                }
            }
            Aegis::Encrypted(encrypted) => {
                tracing::info!(
                    "Found encrypted aegis vault with version {}.",
                    encrypted.version
                );

                // Check for correct aegis vault version and whether a password was supplied.
                if encrypted.version != 1 {
                    anyhow::bail!(
                        "Aegis vault version expected to be 1. Found {} instead.",
                        encrypted.version
                    );
                } else if key.is_none() {
                    anyhow::bail!("Found encrypted aegis database but no password given.");
                }

                // Ciphertext is stored in base64, we have to decode it.
                let mut ciphertext = data_encoding::BASE64
                    .decode(encrypted.db.as_bytes())
                    .context("Cannot decode (base64) encoded database")?;

                // Add the encryption tag
                ciphertext.append(&mut encrypted.header.params.as_ref().unwrap().tag.into());

                // Find slots with type password and derive the corresponding key. This key is
                // used to decrypt the master key which in turn can be used to
                // decrypt the database.
                let master_keys: Vec<Vec<u8>> = encrypted
                    .header
                    .slots
                    .as_ref()
                    .unwrap()
                    .iter()
                    .filter(|slot| slot.type_ == 1) // We don't handle biometric slots for now
                    .map(|slot| -> Result<Vec<u8>> {
                        tracing::info!("Found possible master key with UUID {}.", slot.uuid);

                        // Create parameters for scrypt function and derive decryption key for
                        // master key
                        //
                        // Somehow, scrypt errors do not implement StdErr and cannot be converted to
                        // anyhow::Error. Should be possible but don't know why it doesn't work.
                        let params = scrypt::Params::new(
                            // TODO log2 for u64 is not stable yet. Change this in the future.
                            (slot.n() as f64).log2() as u8, // Defaults to 15 by aegis
                            slot.r(),                       // Defaults to 8 by aegis
                            slot.p(),                       // Defaults to 1 by aegis
                            scrypt::Params::RECOMMENDED_LEN,
                        )
                        .map_err(|_| anyhow::anyhow!("Invalid scrypt parameters"))?;
                        let mut temp_key: [u8; 32] = [0u8; 32];
                        scrypt::scrypt(
                            key.unwrap().as_bytes(),
                            slot.salt(),
                            &params,
                            &mut temp_key,
                        )
                        .map_err(|_| anyhow::anyhow!("Scrypt key derivation failed"))?;

                        // Now, try to decrypt the master key.
                        let cipher = aes_gcm::Aes256Gcm::new_from_slice(&temp_key)?;
                        let mut ciphertext: Vec<u8> = slot.key.to_vec();
                        ciphertext.append(&mut slot.key_params.tag.to_vec());

                        // Here we get the master key. The decrypt function does not return an error
                        // implementing std error. Thus, we have to convert it.
                        cipher
                            .decrypt(
                                aes_gcm::Nonce::from_slice(&slot.key_params.nonce),
                                ciphertext.as_ref(),
                            )
                            .map_err(|_| anyhow::anyhow!("Cannot decrypt master key"))
                    })
                    // Here, we don't want to fail the whole function because one key slot failed to
                    // get the correct master key. Maybe there is another slot we were able to
                    // decrypt.
                    .filter_map(|x| match x {
                        Ok(x) => Some(x),
                        Err(e) => {
                            tracing::error!("Decrypting master key failed: {:?}", e);
                            None
                        }
                    })
                    .collect();

                // Choose the first valid master key. I don't think there are aegis
                // installations with two valid password slots.
                tracing::info!(
                    "Found {} valid password slots / master keys.",
                    master_keys.len()
                );
                let master_key = match master_keys.first() {
                    Some(x) => {
                        tracing::info!("Using only the first valid key slot / master key.");
                        x
                    }
                    None => anyhow::bail!(
                        "Did not find at least one slot with a valid key. Wrong password?"
                    ),
                };

                // Try to decrypt the database with this master key.
                let cipher = aes_gcm::Aes256Gcm::new_from_slice(master_key)?;
                let plaintext = cipher
                    .decrypt(
                        aes_gcm::Nonce::from_slice(
                            &encrypted.header.params.as_ref().unwrap().nonce,
                        ),
                        ciphertext.as_ref(),
                    )
                    // Decrypt does not return an error implementing std error, thus we convert it.
                    .map_err(|_| anyhow::anyhow!("Cannot decrypt database"))?;

                // Now, we have the decrypted string. Trying to load it with JSON.
                let db: Database = serde_json::de::from_slice(&plaintext)
                    .context("Deserialize decrypted database failed")?;

                // Check version of the database
                tracing::info!("Found aegis database with version {}.", db.version);
                if encrypted.version > 2 {
                    anyhow::bail!(
                        "Aegis database version expected to be 1 or 2. Found {} instead.",
                        db.version
                    );
                }

                // Return items
                for mut item in db.entries {
                    item.fix_empty_issuer()?;
                    items.push(item);
                }
                Ok(items)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn issuer_from_name() {
        let data =
            std::fs::read_to_string("./src/backup/tests/aegis_issuer_from_name.json").unwrap();
        let items = Aegis::restore_from_data(data.as_bytes(), None).unwrap();

        assert_eq!(items[0].issuer(), "issuer");
        assert_eq!(items[0].account(), "missing-issuer");
        assert_eq!(items[1].issuer(), "issuer");
        assert_eq!(items[1].account(), "missing-issuer@domain.com");
    }

    #[test]
    fn parse_plain() {
        let data = std::fs::read_to_string("./src/backup/tests/aegis_plain.json").unwrap();
        let items = Aegis::restore_from_data(data.as_bytes(), None).unwrap();

        assert_eq!(items[0].account(), "Bob");
        assert_eq!(items[0].issuer(), "Google");
        assert_eq!(items[0].secret(), "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567");
        assert_eq!(items[0].period(), Some(30));
        assert_eq!(items[0].algorithm(), Algorithm::SHA1);
        assert_eq!(items[0].digits(), Some(6));
        assert_eq!(items[0].counter(), None);
        assert_eq!(items[0].method(), Method::TOTP);

        assert_eq!(items[1].account(), "Benjamin");
        assert_eq!(items[1].issuer(), "Air Canada");
        assert_eq!(items[1].secret(), "KUVJJOM753IHTNDSZVCNKL7GII");
        assert_eq!(items[1].period(), None);
        assert_eq!(items[1].algorithm(), Algorithm::SHA256);
        assert_eq!(items[1].digits(), Some(7));
        assert_eq!(items[1].counter(), Some(50));
        assert_eq!(items[1].method(), Method::HOTP);

        assert_eq!(items[2].account(), "Sophia");
        assert_eq!(items[2].issuer(), "Boeing");
        assert_eq!(items[2].secret(), "JRZCL47CMXVOQMNPZR2F7J4RGI");
        assert_eq!(items[2].period(), Some(30));
        assert_eq!(items[2].algorithm(), Algorithm::SHA1);
        assert_eq!(items[2].digits(), Some(5));
        assert_eq!(items[2].counter(), None);
        assert_eq!(items[2].method(), Method::Steam);
    }

    #[test]
    fn parse_encrypted() {
        // See <https://github.com/beemdevelopment/Aegis/blob/master/app/src/test/resources/com/beemdevelopment/aegis/importers/aegis_encrypted.json>
        // for this example file.
        let data = std::fs::read_to_string("./src/backup/tests/aegis_encrypted.json").unwrap();
        let items = Aegis::restore_from_data(data.as_bytes(), Some("test")).unwrap();

        assert_eq!(items[0].account(), "Mason");
        assert_eq!(items[0].issuer(), "Deno");
        assert_eq!(items[0].secret(), "4SJHB4GSD43FZBAI7C2HLRJGPQ");
        assert_eq!(items[0].period(), Some(30));
        assert_eq!(items[0].algorithm(), Algorithm::SHA1);
        assert_eq!(items[0].digits(), Some(6));
        assert_eq!(items[0].counter(), None);
        assert_eq!(items[0].method(), Method::TOTP);

        assert_eq!(items[3].account(), "James");
        assert_eq!(items[3].issuer(), "Issuu");
        assert_eq!(items[3].secret(), "YOOMIXWS5GN6RTBPUFFWKTW5M4");
        assert_eq!(items[3].period(), None);
        assert_eq!(items[3].algorithm(), Algorithm::SHA1);
        assert_eq!(items[3].digits(), Some(6));
        assert_eq!(items[3].counter(), Some(1));
        assert_eq!(items[3].method(), Method::HOTP);

        assert_eq!(items[6].account(), "Sophia");
        assert_eq!(items[6].issuer(), "Boeing");
        assert_eq!(items[6].secret(), "JRZCL47CMXVOQMNPZR2F7J4RGI");
        assert_eq!(items[6].period(), Some(30));
        assert_eq!(items[6].algorithm(), Algorithm::SHA1);
        assert_eq!(items[6].digits(), Some(5));
        assert_eq!(items[6].counter(), None);
        assert_eq!(items[6].method(), Method::Steam);
    }

    // TODO: add tests for importing
}
