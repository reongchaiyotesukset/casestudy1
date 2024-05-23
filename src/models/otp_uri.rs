use std::{fmt::Write, str::FromStr};

use percent_encoding::{percent_decode_str, utf8_percent_encode, NON_ALPHANUMERIC};
use url::Url;
use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::{
    backup::RestorableItem,
    models::{Account, Algorithm, Method, OTP},
};

#[allow(clippy::upper_case_acronyms)]
#[derive(Zeroize, ZeroizeOnDrop)]
pub struct OTPUri {
    #[zeroize(skip)]
    pub(crate) algorithm: Algorithm,
    #[zeroize(skip)]
    pub(crate) label: String,
    pub(crate) secret: String,
    #[zeroize(skip)]
    pub(crate) issuer: String,
    #[zeroize(skip)]
    pub(crate) method: Method,
    #[zeroize(skip)]
    pub(crate) digits: Option<u32>,
    #[zeroize(skip)]
    pub(crate) period: Option<u32>,
    #[zeroize(skip)]
    pub(crate) counter: Option<u32>,
}

impl RestorableItem for OTPUri {
    fn account(&self) -> String {
        self.label.clone()
    }

    fn issuer(&self) -> String {
        self.issuer.clone()
    }

    fn secret(&self) -> String {
        self.secret.clone()
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

impl TryFrom<Url> for OTPUri {
    type Error = anyhow::Error;
    fn try_from(url: Url) -> Result<Self, Self::Error> {
        if url.scheme() != "otpauth" {
            anyhow::bail!(
                "Invalid OTP uri format, expected otpauth, got {}",
                url.scheme()
            );
        }

        let mut period = None;
        let mut counter = None;
        let mut digits = None;
        let mut provider_name = None;
        let mut algorithm = None;
        let mut secret = None;

        let pairs = url.query_pairs();

        let method = Method::from_str(url.host_str().unwrap())?;

        let account_info = url
            .path()
            .trim_start_matches('/')
            .split(':')
            .collect::<Vec<&str>>();

        let account_name = if account_info.len() == 1 {
            account_info.first().unwrap()
        } else {
            // If we have "Provider:Account"
            provider_name = Some(account_info.first().unwrap().to_string());
            account_info.get(1).unwrap()
        };

        pairs.for_each(|(key, value)| match key.into_owned().as_str() {
            "period" => {
                period = value.parse::<u32>().ok();
            }
            "digits" => {
                digits = value.parse::<u32>().ok();
            }
            "counter" => {
                counter = value.parse::<u32>().ok();
            }
            "issuer" => {
                provider_name = Some(value.to_string());
            }
            "algorithm" => {
                algorithm = Algorithm::from_str(&value).ok();
            }
            "secret" => {
                secret = Some(value.to_string());
            }
            _ => (),
        });

        if secret.is_none() {
            anyhow::bail!("OTP uri must contain a secret");
        }

        let label = percent_decode_str(account_name).decode_utf8()?.into_owned();
        let issuer = if let Some(n) = provider_name {
            percent_decode_str(&n).decode_utf8()?.into_owned()
        } else {
            "Default".to_string()
        };

        Ok(Self {
            method,
            label,
            secret: secret.unwrap(),
            issuer,
            algorithm: algorithm.unwrap_or_default(),
            digits,
            period,
            counter,
        })
    }
}

impl FromStr for OTPUri {
    type Err = anyhow::Error;
    fn from_str(uri: &str) -> Result<Self, Self::Err> {
        let url = Url::parse(uri)?;
        OTPUri::try_from(url)
    }
}

impl From<OTPUri> for String {
    fn from(val: OTPUri) -> Self {
        let mut otp_uri = format!(
            "otpauth://{}/{}?secret={}&issuer={}&algorithm={}",
            val.method.to_string(),
            utf8_percent_encode(&val.label, NON_ALPHANUMERIC),
            val.secret,
            utf8_percent_encode(&val.issuer, NON_ALPHANUMERIC),
            val.algorithm.to_string().to_uppercase(),
        );
        if let Some(digits) = val.digits {
            write!(otp_uri, "&digits={digits}").unwrap();
        }
        if val.method.is_event_based() {
            write!(
                otp_uri,
                "&counter={}",
                val.counter.unwrap_or(OTP::DEFAULT_COUNTER)
            )
            .unwrap();
        } else {
            write!(
                otp_uri,
                "&period={}",
                val.period.unwrap_or(OTP::DEFAULT_PERIOD)
            )
            .unwrap();
        }
        otp_uri
    }
}

impl From<&Account> for OTPUri {
    fn from(a: &Account) -> Self {
        Self {
            method: a.provider().method(),
            label: a.name(),
            secret: a.otp().secret(),
            issuer: a.provider().name(),
            algorithm: a.provider().algorithm(),
            digits: Some(a.provider().digits()),
            period: Some(a.provider().period()),
            counter: Some(a.counter()),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::OTPUri;
    use crate::{
        backup::RestorableItem,
        models::{Algorithm, Method},
    };

    #[test]
    fn decode() {
        let uri = OTPUri::from_str(
            "otpauth://totp/Example:alice@google.com?secret=JBSWY3DPEHPK3PXP&issuer=Example",
        )
        .unwrap();
        assert_eq!(uri.method(), Method::TOTP);
        assert_eq!(uri.issuer(), "Example");
        assert_eq!(uri.secret(), "JBSWY3DPEHPK3PXP");
        assert_eq!(uri.account(), "alice@google.com");

        let uri = OTPUri::from_str("otpauth://totp/ACME%20Co:john.doe@email.com?secret=HXDMVJECJJWSRB3HWIZR4IFUGFTMXBOZ&issuer=ACME%20Co&algorithm=SHA1&digits=6&period=30").unwrap();
        assert_eq!(uri.period(), Some(30));
        assert_eq!(uri.digits(), Some(6));
        assert_eq!(uri.algorithm(), Algorithm::SHA1);
        assert_eq!(uri.issuer(), "ACME Co");
        assert_eq!(uri.secret(), "HXDMVJECJJWSRB3HWIZR4IFUGFTMXBOZ");
        assert_eq!(uri.account(), "john.doe@email.com");
        assert_eq!(uri.method(), Method::TOTP);

        let uri = OTPUri::from_str("otpauth://totp/GitLab:sbeve72?secret=[secret]&issuer=GitLab")
            .unwrap();
        assert_eq!(uri.issuer(), "GitLab");
        assert_eq!(uri.account(), "sbeve72");
        assert_eq!(uri.secret(), "[secret]");
        assert_eq!(uri.method(), Method::TOTP);
    }

    #[test]
    fn encode() {
        let uri = OTPUri {
            algorithm: Algorithm::SHA1,
            label: "account test".to_owned(),
            secret: "dznF36H0IIg17rK".to_owned(),
            issuer: "Test".to_owned(),
            method: Method::TOTP,
            digits: Some(6),
            period: Some(30),
            counter: None,
        };
        assert_eq!(String::from(uri), "otpauth://totp/account%20test?secret=dznF36H0IIg17rK&issuer=Test&algorithm=SHA1&digits=6&period=30");
    }
}
