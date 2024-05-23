use anyhow::Result;
use gtk::{gio, glib, prelude::*, subclass::prelude::*};

use super::{Account, Algorithm, Method, Provider, ProviderPatch, OTP};

mod imp {
    use std::cell::{Cell, RefCell};

    use super::*;

    #[derive(Default)]
    pub struct ProvidersModel(pub RefCell<Vec<Provider>>, pub Cell<bool>);

    #[glib::object_subclass]
    impl ObjectSubclass for ProvidersModel {
        const NAME: &'static str = "ProvidersModel";
        type Type = super::ProvidersModel;
        type Interfaces = (gio::ListModel,);
    }
    impl ObjectImpl for ProvidersModel {}
    impl ListModelImpl for ProvidersModel {
        fn item_type(&self) -> glib::Type {
            Provider::static_type()
        }
        fn n_items(&self) -> u32 {
            self.0.borrow().len() as u32
        }
        fn item(&self, position: u32) -> Option<glib::Object> {
            self.0
                .borrow()
                .get(position as usize)
                .map(|o| o.clone().upcast::<glib::Object>())
        }
    }
}

glib::wrapper! {
    pub struct ProvidersModel(ObjectSubclass<imp::ProvidersModel>)
        @implements gio::ListModel;
}

impl ProvidersModel {
    #[allow(clippy::too_many_arguments)]
    pub fn find_or_create(
        &self,
        name: &str,
        period: Option<u32>,
        method: Method,
        website: Option<String>,
        algorithm: Algorithm,
        digits: Option<u32>,
        default_counter: Option<u32>,
        help_url: Option<String>,
        image_uri: Option<String>,
    ) -> Result<Provider> {
        let provider = match self.find_by_name(name) {
            Some(p) => {
                // Update potenitally different properties than what we have in the pre-shipped
                // database Note this does a comparaison first to avoid a
                // uselesss rewrite
                p.update(&ProviderPatch {
                    name: name.to_owned(),
                    website,
                    help_url,
                    image_uri,
                    period: period.unwrap_or_else(|| p.period()) as i32,
                    digits: digits.unwrap_or_else(|| p.digits()) as i32,
                    default_counter: default_counter.unwrap_or_else(|| p.default_counter()) as i32,
                    algorithm: algorithm.to_string(),
                    method: method.to_string(),
                    is_backup_restore: true,
                })?;
                p
            }
            None => {
                let p = Provider::create(
                    name,
                    period.unwrap_or(OTP::DEFAULT_PERIOD),
                    algorithm,
                    website,
                    method,
                    digits.unwrap_or(OTP::DEFAULT_DIGITS),
                    default_counter.unwrap_or(OTP::DEFAULT_COUNTER),
                    help_url,
                    image_uri,
                )?;
                self.append(&p);
                p
            }
        };
        Ok(provider)
    }

    fn find_by_name(&self, name: &str) -> Option<Provider> {
        for pos in 0..self.n_items() {
            let provider = self.item(pos).and_downcast::<Provider>().unwrap();
            if provider.name() == name {
                return Some(provider);
            }
        }
        None
    }

    pub fn find_by_id(&self, id: u32) -> Option<Provider> {
        for pos in 0..self.n_items() {
            let provider = self.item(pos).and_downcast::<Provider>().unwrap();
            if provider.id() == id {
                return Some(provider);
            }
        }
        None
    }

    pub fn has_providers(&self) -> bool {
        let mut found = false;
        for pos in 0..self.n_items() {
            let provider = self.item(pos).and_downcast::<Provider>().unwrap();
            if provider.has_accounts() {
                found = true;
                break;
            }
        }
        found
    }

    #[allow(deprecated)]
    pub fn completion_model(&self) -> gtk::ListStore {
        let store = gtk::ListStore::new(&[u32::static_type(), String::static_type()]);
        for pos in 0..self.n_items() {
            let obj = self.item(pos).unwrap();
            let provider = obj.downcast_ref::<Provider>().unwrap();
            store.set(
                &store.append(),
                &[(0, &provider.id()), (1, &provider.name())],
            );
        }
        store
    }

    pub fn append(&self, provider: &Provider) {
        let pos = {
            let mut data = self.imp().0.borrow_mut();
            data.push(provider.clone());
            (data.len() - 1) as u32
        };
        self.items_changed(pos, 0, 1);
    }

    fn splice(&self, providers: &[Provider]) {
        let len = providers.len();
        let pos = {
            let mut data = self.imp().0.borrow_mut();
            let pos = data.len();
            data.extend_from_slice(providers);
            pos as u32
        };
        self.items_changed(pos, 0, len as u32);
    }

    pub fn delete_provider(&self, provider: &Provider) {
        let mut provider_pos = None;
        for pos in 0..self.n_items() {
            let p = self.item(pos).and_downcast::<Provider>().unwrap();
            if p.id() == provider.id() {
                provider_pos = Some(pos);
                break;
            }
        }
        if let Some(pos) = provider_pos {
            {
                let mut data = self.imp().0.borrow_mut();
                data.remove(pos as usize);
            }
            self.items_changed(pos, 1, 0);
        }
    }

    pub fn add_account(&self, account: &Account, provider: &Provider) {
        let mut found = false;
        for pos in 0..self.n_items() {
            let obj = self.item(pos).unwrap();
            let p = obj.downcast_ref::<Provider>().unwrap();
            if p.id() == provider.id() {
                found = true;
                p.add_account(account);
                break;
            }
        }
        if !found {
            provider.add_account(account);
            self.append(provider);
        }
    }

    pub fn find_accounts(&self, terms: &[String]) -> Vec<Account> {
        let mut results = vec![];

        for pos in 0..self.n_items() {
            let obj = self.item(pos).unwrap();
            let provider = obj.downcast_ref::<Provider>().unwrap();
            let accounts = provider.find_accounts(terms);
            results.extend(accounts);
        }
        results
    }

    /// Check whether the model was loaded from the database
    pub fn is_loaded(&self) -> bool {
        self.imp().1.get()
    }

    pub fn load(&self) {
        if self.is_loaded() {
            return;
        }
        tracing::info!("Loading providers");
        // fill in the providers from the database
        let providers = Provider::load()
            .expect("Failed to load providers from the database")
            .collect::<Vec<_>>();
        self.splice(&providers);
        self.imp().1.set(true);
    }
}

impl Default for ProvidersModel {
    fn default() -> Self {
        glib::Object::new()
    }
}
