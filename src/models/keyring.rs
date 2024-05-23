use std::collections::HashMap;

use once_cell::sync::OnceCell;
use rand::RngCore;

use crate::config;

pub static SECRET_SERVICE: OnceCell<oo7::Keyring> = OnceCell::new();

fn token_attributes(token_id: &str) -> HashMap<&str, &str> {
    HashMap::from([
        ("application", config::APP_ID),
        ("type", "token"),
        ("token_id", token_id),
    ])
}

fn password_attributes() -> HashMap<&'static str, &'static str> {
    HashMap::from([("application", config::APP_ID), ("type", "password")])
}

fn encode_argon2(secret: &str) -> anyhow::Result<String> {
    let password = secret.as_bytes();
    let mut salt = [0u8; 64];
    rand::thread_rng().fill_bytes(&mut salt);
    let config = argon2::Config::default();
    let hash = argon2::hash_encoded(password, &salt, &config)?;

    Ok(hash)
}

pub async fn store(label: &str, token: &str) -> anyhow::Result<String> {
    let token_id = encode_argon2(token)?;
    let attributes = token_attributes(&token_id);
    let base64_encoded_token = hex::encode(token.as_bytes());
    SECRET_SERVICE
        .get()
        .unwrap()
        .create_item(label, &attributes, base64_encoded_token.as_bytes(), true)
        .await?;
    Ok(token_id)
}

pub async fn token(token_id: &str) -> anyhow::Result<Option<String>> {
    let attributes = token_attributes(token_id);
    let items = SECRET_SERVICE
        .get()
        .unwrap()
        .search_items(&attributes)
        .await?;
    Ok(match items.first() {
        Some(e) => Some(String::from_utf8(hex::decode(&*e.secret().await?)?)?),
        _ => None,
    })
}

pub async fn remove_token(token_id: &str) -> anyhow::Result<()> {
    let attributes = token_attributes(token_id);
    SECRET_SERVICE.get().unwrap().delete(&attributes).await?;
    Ok(())
}

pub async fn token_exists(token: &str) -> anyhow::Result<bool> {
    let attributes = HashMap::from([("application", config::APP_ID), ("type", "token")]);
    let items = SECRET_SERVICE
        .get()
        .unwrap()
        .search_items(&attributes)
        .await?;
    for item in items {
        let item_token = String::from_utf8(hex::decode(&*item.secret().await?)?)?;
        if item_token == token {
            return Ok(true);
        }
    }
    Ok(false)
}

pub async fn has_set_password() -> anyhow::Result<bool> {
    let attributes = password_attributes();
    match SECRET_SERVICE
        .get()
        .unwrap()
        .search_items(&attributes)
        .await
    {
        Ok(items) => Ok(items.first().is_some()),
        _ => Ok(false),
    }
}

/// Stores password using the Argon2 algorithm with a random 128bit salt.
pub async fn set_password(password: &str) -> anyhow::Result<()> {
    let encoded_password = encode_argon2(password)?;
    let attributes = password_attributes();
    SECRET_SERVICE
        .get()
        .unwrap()
        .create_item(
            "Authenticator password",
            &attributes,
            encoded_password.as_bytes(),
            true,
        )
        .await?;
    Ok(())
}

pub async fn reset_password() -> anyhow::Result<()> {
    let attributes = password_attributes();
    SECRET_SERVICE.get().unwrap().delete(&attributes).await?;
    Ok(())
}

pub async fn is_current_password(password: &str) -> anyhow::Result<bool> {
    let attributes = password_attributes();
    let items = SECRET_SERVICE
        .get()
        .unwrap()
        .search_items(&attributes)
        .await?;
    Ok(match items.first() {
        Some(i) => {
            // Verifies that the hash generated by `password` corresponds
            // to `hash`.
            argon2::verify_encoded(
                &String::from_utf8_lossy(&i.secret().await?),
                password.as_bytes(),
            )?
        }
        None => false,
    })
}
