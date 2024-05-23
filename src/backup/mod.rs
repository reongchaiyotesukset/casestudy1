use anyhow::Result;

use crate::{
    models::{keyring, Account, Algorithm, Method, ProvidersModel},
    utils::spawn_tokio_blocking,
};

pub enum Operation {
    Backup,
    Restore,
}

pub trait Restorable: Sized {
    /// Indicates that the GUI might need to prompt for a password.
    const ENCRYPTABLE: bool = false;

    /// Indicates that the GUI needs to show a QR code scanner.
    const SCANNABLE: bool = false;

    // Used to define the `restore.$identifier` action
    const IDENTIFIER: &'static str;

    type Item: RestorableItem;

    fn title() -> String;
    fn subtitle() -> String;

    /// Restore many items from a slice of data, optionally using a key to
    /// unencrypt it.
    ///
    /// If `key` is `None`, then the implementation should assume that the slice
    /// is unencrypted, and error if it only supports encrypted slices.
    fn restore_from_data(from: &[u8], key: Option<&str>) -> Result<Vec<Self::Item>>;
}

pub trait RestorableItem {
    fn account(&self) -> String;
    fn issuer(&self) -> String;
    fn secret(&self) -> String;
    fn period(&self) -> Option<u32>;
    fn method(&self) -> Method;
    fn algorithm(&self) -> Algorithm;
    fn digits(&self) -> Option<u32>;
    fn counter(&self) -> Option<u32>;

    fn restore(&self, provider: &ProvidersModel) -> Result<()> {
        let owned_token = self.secret();
        let token_exists =
            spawn_tokio_blocking(async move { keyring::token_exists(&owned_token).await })?;
        if !token_exists {
            let provider = provider.find_or_create(
                &self.issuer(),
                self.period(),
                self.method(),
                None,
                self.algorithm(),
                self.digits(),
                self.counter(),
                None,
                None,
            )?;

            let account =
                Account::create(&self.account(), &self.secret(), self.counter(), &provider)?;
            provider.add_account(&account);
        } else {
            tracing::info!(
                "Account {}/{} already exists",
                self.issuer(),
                self.account()
            );
        }
        Ok(())
    }
}

pub trait Backupable: Sized {
    /// Indicates that the GUI might need to prompt for a password.
    const ENCRYPTABLE: bool = false;
    // Used to define the `backup.$identifier` action
    const IDENTIFIER: &'static str;

    fn title() -> String;
    fn subtitle() -> String;
    // if no key is provided the backup code should save it as plain text
    fn backup(provider: &ProvidersModel, key: Option<&str>) -> Result<Vec<u8>>;
}

mod aegis;
mod andotp;
mod bitwarden;
mod freeotp;
mod freeotp_json;
mod google;
mod legacy;
mod raivootp;
pub use self::{
    aegis::Aegis, andotp::AndOTP, bitwarden::Bitwarden, freeotp::FreeOTP,
    freeotp_json::FreeOTPJSON, google::Google, legacy::LegacyAuthenticator, raivootp::RaivoOTP,
};
