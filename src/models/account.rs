use anyhow::{Context, Result};
use diesel::prelude::*;
use gtk::{
    glib::{self, clone},
    prelude::*,
    subclass::prelude::*,
};

use crate::{
    models::{database, keyring, DieselProvider, Method, OTPUri, Provider, OTP, RUNTIME},
    schema::accounts,
    utils::spawn_tokio_blocking,
};

#[derive(Insertable)]
#[diesel(table_name = accounts)]
struct NewAccount {
    pub name: String,
    pub token_id: String,
    pub provider_id: i32,
    pub counter: i32,
}

#[derive(Identifiable, Queryable, Associations)]
#[diesel(belongs_to(DieselProvider, foreign_key = provider_id))]
#[diesel(table_name = accounts)]
pub struct DieselAccount {
    pub id: i32,
    pub name: String,
    pub counter: i32,
    pub token_id: String,
    pub provider_id: i32,
}

#[doc(hidden)]
mod imp {
    use std::cell::{Cell, OnceCell, RefCell};

    use glib::ParamSpecObject;
    use once_cell::sync::Lazy;

    use super::*;

    #[derive(glib::Properties)]
    #[properties(wrapper_type = super::Account)]
    pub struct Account {
        #[property(get, set, construct)]
        pub id: Cell<u32>,
        #[property(get, set)]
        pub code: RefCell<String>,
        #[property(get, set = Self::set_name)]
        pub name: RefCell<String>,
        #[property(get, set = Self::set_counter, default = OTP::DEFAULT_COUNTER)]
        pub counter: Cell<u32>,
        pub otp: OnceCell<OTP>,
        #[property(get, set, construct_only)]
        pub token_id: RefCell<String>,
        // We don't use property here as we can't mark the getter as not nullable
        pub provider: RefCell<Option<Provider>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Account {
        const NAME: &'static str = "Account";
        type Type = super::Account;

        fn new() -> Self {
            Self {
                id: Cell::default(),
                counter: Cell::new(OTP::DEFAULT_COUNTER),
                name: RefCell::default(),
                code: RefCell::default(),
                token_id: RefCell::default(),
                provider: RefCell::default(),
                otp: OnceCell::default(),
            }
        }
    }

    impl ObjectImpl for Account {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                let mut props = Account::derived_properties().to_vec();
                props.push(ParamSpecObject::builder::<Provider>("provider").build());
                props
            });
            PROPERTIES.as_ref()
        }

        fn set_property(&self, id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            match pspec.name() {
                "provider" => {
                    let provider = value.get().unwrap();
                    self.provider.replace(provider);
                }
                _ => self.derived_set_property(id, value, pspec),
            }
        }

        fn property(&self, id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "provider" => self.provider.borrow().to_value(),
                _ => self.derived_property(id, pspec),
            }
        }
    }

    impl Account {
        fn set_name_inner(&self, id: i32, name: &str) -> Result<()> {
            let db = database::connection();
            let mut conn = db.get()?;

            let target = accounts::table.filter(accounts::columns::id.eq(id));
            diesel::update(target)
                .set(accounts::columns::name.eq(name))
                .execute(&mut conn)?;
            Ok(())
        }

        fn set_name(&self, name: &str) {
            match self.set_name_inner(self.obj().id() as i32, name) {
                Ok(_) => {
                    self.name.replace(name.to_owned());
                }
                Err(err) => {
                    tracing::warn!("Failed to update account name {err}");
                }
            }
        }

        fn set_counter_inner(&self, id: i32, counter: u32) -> Result<()> {
            let db = database::connection();
            let mut conn = db.get()?;

            let target = accounts::table.filter(accounts::columns::id.eq(id));
            diesel::update(target)
                .set(accounts::columns::counter.eq(counter as i32))
                .execute(&mut conn)?;
            Ok(())
        }

        fn set_counter(&self, counter: u32) {
            match self.set_counter_inner(self.obj().id() as i32, counter) {
                Ok(_) => {
                    self.counter.set(counter);
                }
                Err(err) => {
                    tracing::warn!("Failed to update account counter {err}");
                }
            }
        }
    }
}

glib::wrapper! {
    pub struct Account(ObjectSubclass<imp::Account>);
}

impl Account {
    pub fn create(
        name: &str,
        token: &str,
        counter: Option<u32>,
        provider: &Provider,
    ) -> Result<Account> {
        let db = database::connection();
        let mut conn = db.get()?;

        let label = format!("{} - {name}", provider.name());
        let token_send = token.to_owned();
        let token_id = spawn_tokio_blocking(async move {
            keyring::store(&label, &token_send)
                .await
                .context("Failed to save token")
        })?;

        diesel::insert_into(accounts::table)
            .values(NewAccount {
                name: name.to_string(),
                token_id,
                provider_id: provider.id() as i32,
                counter: counter.unwrap_or_else(|| provider.default_counter()) as i32,
            })
            .execute(&mut conn)?;

        accounts::table
            .order(accounts::columns::id.desc())
            .first::<DieselAccount>(&mut conn)
            .map_err(From::from)
            .map(|account| {
                Self::new(
                    account.id as u32,
                    &account.name,
                    &account.token_id,
                    account.counter as u32,
                    provider,
                    Some(token),
                )
                .unwrap()
            })
    }

    pub fn load(p: &Provider) -> Result<impl Iterator<Item = Self>> {
        let db = database::connection();
        let mut conn = db.get()?;

        let dip = DieselProvider::from(p);
        let results = DieselAccount::belonging_to(&dip)
            .load::<DieselAccount>(&mut conn)?
            .into_iter()
            .filter_map(clone!(@strong p => move |account| {
                match Self::new(
                    account.id  as u32,
                    &account.name,
                    &account.token_id,
                    account.counter as u32,
                    &p,
                    None,
                )
                {
                    Ok(account) => Some(account),
                    Err(e) => {
                        let name = account.name;
                        let provider = p.name();
                        tracing::error!("Failed to load account '{name}' / '{provider}' with error {e}");
                        None
                    }
                }
            }));

        Ok(results)
    }

    pub fn new(
        id: u32,
        name: &str,
        token_id: &str,
        counter: u32,
        provider: &Provider,
        secret: Option<&str>,
    ) -> Result<Account> {
        let account = glib::Object::builder::<Self>()
            .property("id", id)
            .property("name", name)
            .property("token-id", token_id)
            .property("provider", provider)
            .property("counter", counter)
            .build();

        let secret = if let Some(t) = secret {
            t.to_string()
        } else {
            let token_id = token_id.to_owned();
            spawn_tokio_blocking(async move {
                keyring::token(&token_id).await?.with_context(|| {
                    format!("Could not get item with token identifier '{token_id}' from keyring")
                })
            })?
        };
        let otp = OTP::from_str(&secret, provider.algorithm(), provider.digits())?;
        account.imp().otp.set(otp).unwrap();
        account.generate_otp();
        Ok(account)
    }

    pub fn generate_otp(&self) {
        let provider = self.provider();

        let otp_password = match provider.method() {
            Method::Steam => self.otp().steam(None),
            Method::TOTP => self.otp().totp_formatted(Some(provider.period())),
            Method::HOTP => self.otp().hotp_formatted(self.counter() as u64),
        };

        let label = match otp_password {
            Ok(password) => password,
            Err(err) => {
                tracing::warn!("Failed to generate the OTP {}", err);
                "Error".to_string()
            }
        };

        self.set_code(label);
    }

    /// Increment the internal counter in case of a HOTP account
    pub fn increment_counter(&self) -> Result<()> {
        let new_value = self.counter() + 1;
        self.imp().counter.set(new_value);

        let db = database::connection();
        let mut conn = db.get()?;

        let target = accounts::table.filter(accounts::columns::id.eq(self.id() as i32));
        diesel::update(target)
            .set(accounts::columns::counter.eq(new_value as i32))
            .execute(&mut conn)?;
        Ok(())
    }

    pub fn copy_otp(&self) {
        let display = gtk::gdk::Display::default().unwrap();
        let clipboard = display.clipboard();
        // The codes come with the white space shown in the label.
        let code = self.code().replace(' ', "");
        clipboard.set_text(&code);

        // Indirectly increment the counter once the token was copied
        if self.provider().method().is_event_based() {
            self.generate_otp();
        }
    }

    pub fn provider(&self) -> Provider {
        self.imp().provider.borrow().clone().unwrap()
    }

    pub fn set_provider(&self, provider: &Provider) -> Result<()> {
        let db = database::connection();
        let mut conn = db.get()?;

        let target = accounts::table.filter(accounts::columns::id.eq(self.id() as i32));
        diesel::update(target)
            .set(accounts::columns::provider_id.eq(provider.id() as i32))
            .execute(&mut conn)?;
        self.imp().provider.replace(Some(provider.clone()));
        self.notify("provider");
        Ok(())
    }

    pub fn otp(&self) -> &OTP {
        self.imp().otp.get().unwrap()
    }

    pub fn otp_uri(&self) -> OTPUri {
        self.into()
    }

    pub fn delete(&self) -> Result<()> {
        let token_id = self.token_id();
        RUNTIME.spawn(async move {
            if let Err(err) = keyring::remove_token(&token_id).await {
                tracing::error!("Failed to remove the token from secret service {}", err);
            }
        });
        let db = database::connection();
        let mut conn = db.get()?;
        diesel::delete(accounts::table.filter(accounts::columns::id.eq(self.id() as i32)))
            .execute(&mut conn)?;
        Ok(())
    }
}
