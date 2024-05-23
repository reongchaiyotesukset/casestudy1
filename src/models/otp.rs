use std::{
    convert::TryInto,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{anyhow, Result};
use data_encoding::BASE32_NOPAD;
use ring::hmac;
use zeroize::{Zeroize, ZeroizeOnDrop};

use super::Algorithm;

#[derive(Debug, Zeroize, ZeroizeOnDrop)]
#[allow(clippy::upper_case_acronyms)]
pub struct OTP {
    secret: Vec<u8>,
    #[zeroize(skip)]
    algorithm: Algorithm,
    #[zeroize(skip)]
    digits: u32,
}

impl OTP {
    const STEAM_CHARS: &'static str = "23456789BCDFGHJKMNPQRTVWXY";
    pub const STEAM_DEFAULT_PERIOD: u32 = 30;
    pub const STEAM_DEFAULT_DIGITS: u32 = 5;
    pub const DEFAULT_COUNTER: u32 = 1;
    pub const DEFAULT_DIGITS: u32 = 6;
    pub const DEFAULT_PERIOD: u32 = 30;

    // Validates if `secret` is a valid Base32 String.
    pub fn is_valid(secret: &str) -> bool {
        decode_secret(secret).is_ok()
    }

    fn time_based_counter(period: u32) -> u64 {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        timestamp / period as u64
    }

    pub fn from_bytes_steam(secret: impl AsRef<[u8]>) -> Self {
        Self::from_bytes(secret, Algorithm::SHA1, Self::STEAM_DEFAULT_DIGITS)
    }

    pub fn from_str_steam(secret: &str) -> Result<Self> {
        Self::from_str(secret, Algorithm::SHA1, Self::STEAM_DEFAULT_DIGITS)
    }

    pub fn from_str_with_defaults(secret: &str) -> Result<Self> {
        let decoded = decode_secret(secret)?;
        Ok(Self::from_bytes_with_defaults(decoded))
    }

    pub fn from_str(secret: &str, algorithm: Algorithm, digits: u32) -> Result<Self> {
        let decoded = decode_secret(secret)?;
        Ok(Self::from_bytes(decoded, algorithm, digits))
    }

    pub fn from_bytes_with_defaults(secret: impl AsRef<[u8]>) -> Self {
        Self {
            secret: secret.as_ref().to_owned(),
            algorithm: Algorithm::default(),
            digits: Self::DEFAULT_DIGITS,
        }
    }

    pub fn from_bytes(secret: impl AsRef<[u8]>, algorithm: Algorithm, digits: u32) -> Self {
        Self {
            secret: secret.as_ref().to_owned(),
            algorithm,
            digits,
        }
    }

    /// Performs the [HMAC-based One-time Password Algorithm](http://en.wikipedia.org/wiki/HMAC-based_One-time_Password_Algorithm)
    /// (HOTP) given an RFC4648 base32 encoded secret, and an integer counter.
    pub fn hotp(&self, counter: u64) -> Result<u32> {
        let digest = encode_digest(calc_digest(&self.secret, counter, self.algorithm))?;
        Ok(digest % 10_u32.pow(self.digits))
    }

    pub fn hotp_formatted(&self, counter: u64) -> Result<String> {
        self.hotp(counter).map(|d| format(d, self.digits as usize))
    }

    pub fn totp(&self, period: Option<u32>) -> Result<u32> {
        let counter = Self::time_based_counter(period.unwrap_or(Self::DEFAULT_PERIOD));
        self.hotp(counter)
    }

    pub fn totp_formatted(&self, period: Option<u32>) -> Result<String> {
        let counter = Self::time_based_counter(period.unwrap_or(Self::DEFAULT_PERIOD));
        self.hotp_formatted(counter)
    }

    pub fn steam(&self, counter: Option<u64>) -> Result<String> {
        let counter = counter.unwrap_or(Self::time_based_counter(Self::STEAM_DEFAULT_PERIOD));
        let mut full_token = encode_digest(calc_digest(&self.secret, counter, Algorithm::SHA1))?;

        let mut code = String::new();
        let total_chars = Self::STEAM_CHARS.len() as u32;
        for _ in 0..Self::STEAM_DEFAULT_DIGITS {
            let pos = full_token % total_chars;
            let charachter = Self::STEAM_CHARS.chars().nth(pos as usize).unwrap();
            code.push(charachter);
            full_token /= total_chars;
        }
        Ok(code)
    }

    pub fn secret(&self) -> String {
        data_encoding::BASE32_NOPAD.encode(&self.secret)
    }
}

/// Code graciously taken from the rust-otp crate.
/// <https://github.com/TimDumol/rust-otp/blob/master/src/lib.rs>

/// Decodes a secret (given as an RFC4648 base32-encoded ASCII string)
/// into a byte string. It fails if secret is not a valid Base32 string.
fn decode_secret(secret: &str) -> Result<Vec<u8>> {
    let secret = secret.trim().replace(' ', "").to_ascii_uppercase();
    // The buffer should have a length of secret.len() * 5 / 8.
    BASE32_NOPAD
        .decode(secret.as_bytes())
        .map_err(|_| anyhow!("Invalid Input"))
}

fn format(code: u32, digits: usize) -> String {
    let padded_code = format!("{code:0digits$}");
    let mut formated_code = String::new();
    for (idx, ch) in padded_code.chars().enumerate() {
        if (digits - idx) % 3 == 0 && (digits - idx) != 0 && idx != 0 {
            formated_code.push(' ');
        }
        formated_code.push(ch);
    }
    formated_code
}

/// Calculates the HMAC digest for the given secret and counter.
fn calc_digest(decoded_secret: impl AsRef<[u8]>, counter: u64, algorithm: Algorithm) -> hmac::Tag {
    let key = hmac::Key::new(algorithm.into(), decoded_secret.as_ref());
    hmac::sign(&key, &counter.to_be_bytes())
}

/// Encodes the HMAC digest into a n-digit integer.
fn encode_digest(digest: impl AsRef<[u8]>) -> Result<u32> {
    let digest = digest.as_ref();
    let offset = match digest.last() {
        Some(x) => *x & 0xf,
        None => anyhow::bail!("Invalid digest"),
    } as usize;
    let code_bytes: [u8; 4] = match digest[offset..offset + 4].try_into() {
        Ok(x) => x,
        Err(_) => anyhow::bail!("Invalid digest"),
    };
    let code = u32::from_be_bytes(code_bytes);
    Ok(code & 0x7fffffff)
}

// Some of the tests are heavily inspired(copy-paste) of the andOTP application
#[cfg(test)]
mod tests {
    use super::{format, Algorithm, OTP};

    #[test]
    fn totp() {
        let secret_sha1 = b"12345678901234567890";
        let secret_sha256 = b"12345678901234567890123456789012";
        let secret_sha512 = b"1234567890123456789012345678901234567890123456789012345678901234";

        let otp1 = OTP::from_bytes(secret_sha1, Algorithm::SHA1, 8);
        let otp2 = OTP::from_bytes(secret_sha256, Algorithm::SHA256, 8);
        let otp3 = OTP::from_bytes(secret_sha512, Algorithm::SHA512, 8);

        let counter1 = 59 / OTP::DEFAULT_PERIOD as u64;
        assert_eq!(Some(94287082), otp1.hotp(counter1).ok());
        assert_eq!(Some(46119246), otp2.hotp(counter1).ok());
        assert_eq!(Some(90693936), otp3.hotp(counter1).ok());

        let counter2 = 1111111109 / OTP::DEFAULT_PERIOD as u64;
        assert_eq!(Some(7081804), otp1.hotp(counter2).ok());
        assert_eq!(Some(68084774), otp2.hotp(counter2).ok());
        assert_eq!(Some(25091201), otp3.hotp(counter2).ok());

        let counter3 = 1111111111 / OTP::DEFAULT_PERIOD as u64;
        assert_eq!(Some(14050471), otp1.hotp(counter3).ok());
        assert_eq!(Some(67062674), otp2.hotp(counter3).ok());
        assert_eq!(Some(99943326), otp3.hotp(counter3).ok());

        let counter4 = 1234567890 / OTP::DEFAULT_PERIOD as u64;
        assert_eq!(Some(89005924), otp1.hotp(counter4).ok());
        assert_eq!(Some(91819424), otp2.hotp(counter4).ok());
        assert_eq!(Some(93441116), otp3.hotp(counter4).ok());

        let counter5 = 2000000000 / OTP::DEFAULT_PERIOD as u64;
        assert_eq!(Some(69279037), otp1.hotp(counter5).ok());
        assert_eq!(Some(90698825), otp2.hotp(counter5).ok());
        assert_eq!(Some(38618901), otp3.hotp(counter5).ok());

        let counter6 = 20000000000 / OTP::DEFAULT_PERIOD as u64;
        assert_eq!(Some(65353130), otp1.hotp(counter6).ok());
        assert_eq!(Some(77737706), otp2.hotp(counter6).ok());
        assert_eq!(Some(47863826), otp3.hotp(counter6).ok());
    }

    #[test]
    fn hotp() {
        let otp = OTP::from_str_with_defaults("BASE32SECRET3232").unwrap();
        assert_eq!(otp.hotp(0).ok(), Some(260182));
        assert_eq!(otp.hotp(1).ok(), Some(55283));
        assert_eq!(otp.hotp(1401).ok(), Some(316439));

        let otp = OTP::from_bytes_with_defaults(b"12345678901234567890");
        assert_eq!(Some(755224), otp.hotp(0).ok(),);
        assert_eq!(Some(287082), otp.hotp(1).ok());
        assert_eq!(Some(359152), otp.hotp(2).ok());
        assert_eq!(Some(969429), otp.hotp(3).ok());
        assert_eq!(Some(338314), otp.hotp(4).ok());
        assert_eq!(Some(254676), otp.hotp(5).ok());
        assert_eq!(Some(287922), otp.hotp(6).ok());
        assert_eq!(Some(162583), otp.hotp(7).ok());
        assert_eq!(Some(399871), otp.hotp(8).ok());
        assert_eq!(Some(520489), otp.hotp(9).ok());
    }

    #[test]
    fn steam() {
        let token = OTP::from_str_steam("BASE32SECRET3232").unwrap();
        assert_eq!(token.steam(Some(0)).ok(), Some("2TC8B".into()));
        assert_eq!(token.steam(Some(1)).ok(), Some("YKKK4".into()));
    }

    #[test]
    fn otp_format() {
        assert_eq!(format(1234, 5), "01 234");
        assert_eq!(format(1234, 6), "001 234");
        assert_eq!(format(123456, 6), "123 456");
        assert_eq!(format(1234, 7), "0 001 234");
        assert_eq!(format(1234567, 8), "01 234 567");
        assert_eq!(format(12345678, 8), "12 345 678");
    }
}
