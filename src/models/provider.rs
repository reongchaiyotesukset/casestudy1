use std::{
    string::ToString,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::Result;
use diesel::prelude::*;
use gtk::{
    gdk_pixbuf, gio,
    glib::{self, clone},
    prelude::*,
    subclass::prelude::*,
};
use url::Url;

use crate::{
    models::{database, Account, AccountsModel, Algorithm, Method, FAVICONS_PATH, OTP},
    schema::providers,
};

pub struct ProviderPatch {
    pub name: String,
    pub website: Option<String>,
    pub help_url: Option<String>,
    pub image_uri: Option<String>,
    pub period: i32,
    pub digits: i32,
    pub default_counter: i32,
    pub algorithm: String,
    pub method: String,
    pub is_backup_restore: bool,
}

#[derive(Insertable)]
#[diesel(table_name = providers)]
struct NewProvider {
    pub name: String,
    pub website: Option<String>,
    pub help_url: Option<String>,
    pub image_uri: Option<String>,
    pub period: i32,
    pub digits: i32,
    pub default_counter: i32,
    pub algorithm: String,
    pub method: String,
}

#[derive(Identifiable, Queryable)]
#[diesel(table_name = providers)]
pub struct DieselProvider {
    pub id: i32,
    pub name: String,
    pub website: Option<String>,
    pub help_url: Option<String>,
    pub image_uri: Option<String>,
    pub period: i32,
    pub digits: i32,
    pub default_counter: i32,
    pub algorithm: String,
    pub method: String,
}

mod imp {
    use std::cell::{Cell, RefCell};

    use super::*;

    #[derive(glib::Properties)]
    #[properties(wrapper_type = super::Provider)]
    pub struct Provider {
        #[property(get, set, construct_only)]
        pub id: Cell<u32>,
        #[property(get, set)]
        pub name: RefCell<String>,
        #[property(get, set, maximum = 1000, default = OTP::DEFAULT_PERIOD)]
        pub period: Cell<u32>,
        #[property(get, set, builder(Method::default()))]
        pub method: Cell<Method>,
        #[property(get, set, default = OTP::DEFAULT_COUNTER)]
        pub default_counter: Cell<u32>,
        #[property(get, set, builder(Algorithm::default()))]
        pub algorithm: Cell<Algorithm>,
        #[property(get, set, maximum = 1000, default = OTP::DEFAULT_DIGITS)]
        pub digits: Cell<u32>,
        #[property(get, set)]
        pub website: RefCell<Option<String>>,
        #[property(get, set)]
        pub help_url: RefCell<Option<String>>,
        #[property(get, set = Self::set_image_uri, explicit_notify)]
        pub image_uri: RefCell<Option<String>>,
        #[property(get, set)]
        pub remaining_time: Cell<u64>,
        #[property(get)]
        pub accounts_model: AccountsModel,
        pub filter_model: gtk::FilterListModel,
        pub tick_callback: RefCell<Option<glib::SourceId>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Provider {
        const NAME: &'static str = "Provider";
        type Type = super::Provider;

        fn new() -> Self {
            let model = AccountsModel::default();
            Self {
                id: Cell::default(),
                default_counter: Cell::new(OTP::DEFAULT_COUNTER),
                algorithm: Cell::new(Algorithm::default()),
                digits: Cell::new(OTP::DEFAULT_DIGITS),
                name: RefCell::default(),
                website: RefCell::default(),
                help_url: RefCell::default(),
                image_uri: RefCell::default(),
                method: Cell::new(Method::default()),
                period: Cell::new(OTP::DEFAULT_PERIOD),
                filter_model: gtk::FilterListModel::new(Some(model.clone()), None::<gtk::Filter>),
                accounts_model: model,
                tick_callback: RefCell::default(),
                remaining_time: Cell::default(),
            }
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for Provider {
        fn dispose(&self) {
            // Stop ticking
            if let Some(source_id) = self.tick_callback.borrow_mut().take() {
                source_id.remove();
            }
        }
    }

    impl Provider {
        fn set_image_uri_inner(&self, id: i32, uri: Option<&str>) -> anyhow::Result<()> {
            let db = database::connection();
            let mut conn = db.get()?;

            let target = providers::table.filter(providers::columns::id.eq(id));
            diesel::update(target)
                .set(providers::columns::image_uri.eq(uri))
                .execute(&mut conn)?;

            Ok(())
        }

        fn set_image_uri(&self, uri: Option<&str>) {
            let obj = self.obj();
            if let Err(err) = self.set_image_uri_inner(obj.id() as i32, uri) {
                tracing::warn!("Failed to update provider image {}", err);
            }
            self.image_uri.replace(uri.map(ToOwned::to_owned));
            obj.notify_image_uri();
        }
    }
}

glib::wrapper! {
    pub struct Provider(ObjectSubclass<imp::Provider>);
}

impl Provider {
    #[allow(clippy::too_many_arguments)]
    pub fn create(
        name: &str,
        period: u32,
        algorithm: Algorithm,
        website: Option<String>,
        method: Method,
        digits: u32,
        default_counter: u32,
        help_url: Option<String>,
        image_uri: Option<String>,
    ) -> Result<Self> {
        let db = database::connection();
        let mut conn = db.get()?;

        diesel::insert_into(providers::table)
            .values(NewProvider {
                name: name.to_string(),
                period: period as i32,
                method: method.to_string(),
                website,
                algorithm: algorithm.to_string(),
                digits: digits as i32,
                default_counter: default_counter as i32,
                help_url,
                image_uri,
            })
            .execute(&mut conn)?;

        providers::table
            .order(providers::columns::id.desc())
            .first::<DieselProvider>(&mut conn)
            .map_err(From::from)
            .map(From::from)
    }

    pub fn load() -> Result<impl Iterator<Item = Self>> {
        use crate::schema::providers::dsl::*;
        let db = database::connection();
        let mut conn = db.get()?;

        let results = providers
            .load::<DieselProvider>(&mut conn)?
            .into_iter()
            .map(From::from)
            .map(|p: Provider| {
                let accounts = Account::load(&p).unwrap().collect::<Vec<_>>();
                p.add_accounts(&accounts);
                p
            });
        Ok(results)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: u32,
        name: &str,
        period: u32,
        method: Method,
        algorithm: Algorithm,
        digits: u32,
        default_counter: u32,
        website: Option<String>,
        help_url: Option<String>,
        image_uri: Option<String>,
    ) -> Provider {
        glib::Object::builder()
            .property("id", id)
            .property("name", name)
            .property("website", website)
            .property("help-url", help_url)
            .property("image-uri", image_uri)
            .property("period", period)
            .property("method", method)
            .property("algorithm", algorithm)
            .property("digits", digits)
            .property("default-counter", default_counter)
            .build()
    }

    pub async fn favicon(
        website: String,
        name: String,
        id: u32,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let website_url = Url::parse(&website)?;
        let favicon = favicon_scrapper::Scrapper::from_url(&website_url).await?;
        tracing::debug!("Found the following icons {:#?} for {}", favicon, name);

        let icon_name = format!("{id}_{}", name.replace(' ', "_"));
        let icon_name = glib::base64_encode(icon_name.as_bytes());
        let small_icon_name = format!("{icon_name}_32x32");
        let large_icon_name = format!("{icon_name}_96x96");
        // TODO: figure out why trying to grab icons at specific size causes stack size
        // errors We need two sizes:
        // - 32x32 for the accounts lists
        // - 96x96 elsewhere
        if let Some(best_favicon) = favicon.find_best().await {
            tracing::debug!("Largest favicon found is {:#?}", best_favicon);
            let cache_path = FAVICONS_PATH.join(&*icon_name);
            best_favicon.save(cache_path.clone()).await?;
            // Don't try to scale down svg variants
            if !best_favicon.metadata().format().is_svg() {
                tracing::debug!("Creating scaled down variants for {:#?}", cache_path);
                {
                    let pixbuf = gdk_pixbuf::Pixbuf::from_file(cache_path.clone())?;
                    tracing::debug!("Creating a 32x32 variant of the favicon");
                    let small_pixbuf = pixbuf
                        .scale_simple(32, 32, gdk_pixbuf::InterpType::Bilinear)
                        .unwrap();

                    let mut small_cache = cache_path.clone();
                    small_cache.set_file_name(small_icon_name);
                    small_pixbuf.savev(small_cache.clone(), "png", &[])?;

                    tracing::debug!("Creating a 96x96 variant of the favicon");
                    let large_pixbuf = pixbuf
                        .scale_simple(96, 96, gdk_pixbuf::InterpType::Bilinear)
                        .unwrap();
                    let mut large_cache = cache_path.clone();
                    large_cache.set_file_name(large_icon_name);
                    large_pixbuf.savev(large_cache.clone(), "png", &[])?;
                };
                tokio::fs::remove_file(cache_path).await?;
            } else {
                let mut small_cache = cache_path.clone();
                small_cache.set_file_name(small_icon_name);
                tokio::fs::symlink(&cache_path, small_cache).await?;

                let mut large_cache = cache_path.clone();
                large_cache.set_file_name(large_icon_name);
                tokio::fs::symlink(&cache_path, large_cache).await?;
            }
            Ok(icon_name.to_string())
        } else {
            Err(Box::new(favicon_scrapper::Error::NoResults))
        }
    }

    pub fn delete(&self) -> Result<()> {
        let db = database::connection();
        let mut conn = db.get()?;
        diesel::delete(providers::table.filter(providers::columns::id.eq(self.id() as i32)))
            .execute(&mut conn)?;
        Ok(())
    }

    pub fn update(&self, patch: &ProviderPatch) -> Result<()> {
        // Can't implement PartialEq because of how GObject works
        if patch.name == self.name()
            && patch.website == self.website()
            && patch.help_url == self.help_url()
            && patch.image_uri == self.image_uri()
            && patch.period == self.period() as i32
            && patch.digits == self.digits() as i32
            && patch.default_counter == self.default_counter() as i32
            && patch.algorithm == self.algorithm().to_string()
            && patch.method == self.method().to_string()
        {
            return Ok(());
        }

        let db = database::connection();
        let mut conn = db.get()?;

        let target = providers::table.filter(providers::columns::id.eq(self.id() as i32));
        diesel::update(target)
            .set((
                providers::columns::algorithm.eq(&patch.algorithm),
                providers::columns::method.eq(&patch.method),
                providers::columns::digits.eq(&patch.digits),
                providers::columns::period.eq(&patch.period),
                providers::columns::default_counter.eq(&patch.default_counter),
                providers::columns::name.eq(&patch.name),
            ))
            .execute(&mut conn)?;
        if !patch.is_backup_restore {
            diesel::update(target)
                .set((
                    providers::columns::image_uri.eq(&patch.image_uri),
                    providers::columns::website.eq(&patch.website),
                    providers::columns::help_url.eq(&patch.help_url),
                ))
                .execute(&mut conn)?;
        };

        self.set_properties(&[
            ("name", &patch.name),
            ("period", &(patch.period as u32)),
            ("method", &patch.method.parse::<Method>()?),
            ("digits", &(patch.digits as u32)),
            ("algorithm", &patch.algorithm.parse::<Algorithm>()?),
            ("default-counter", &(patch.default_counter as u32)),
        ]);

        if !patch.is_backup_restore {
            self.set_properties(&[
                ("image-uri", &patch.image_uri),
                ("website", &patch.website),
                ("help-url", &patch.help_url),
            ]);
        }
        Ok(())
    }

    pub fn open_help(&self) {
        if let Some(ref url) = self.help_url() {
            gio::AppInfo::launch_default_for_uri(url, None::<&gio::AppLaunchContext>).unwrap();
        }
    }

    fn tick(&self) {
        let period = self.period() as u64;
        let remaining_time: u64 = period
            - SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs()
                % period;
        if period == remaining_time {
            self.regenerate_otp();
        }
        self.set_remaining_time(remaining_time);
    }

    fn setup_tick_callback(&self) {
        if self.imp().tick_callback.borrow().is_some() || self.method().is_event_based() {
            return;
        }
        self.set_remaining_time(self.period() as u64);

        match self.method() {
            Method::TOTP | Method::Steam => {
                let source_id = glib::timeout_add_seconds_local(
                    1,
                    clone!(@weak self as provider => @default-return glib::ControlFlow::Break, move || {
                        provider.tick();
                        glib::ControlFlow::Continue
                    }),
                );
                self.imp().tick_callback.replace(Some(source_id));
            }
            _ => (),
        };
    }

    fn regenerate_otp(&self) {
        let accounts = self.accounts();
        for i in 0..accounts.n_items() {
            let item = accounts.item(i).unwrap();
            let account = item.downcast_ref::<Account>().unwrap();
            account.generate_otp();
        }
    }

    pub fn has_accounts(&self) -> bool {
        self.accounts_model().n_items() != 0
    }

    fn add_accounts(&self, accounts: &[Account]) {
        self.accounts_model().splice(accounts);
        self.setup_tick_callback();
    }

    pub fn add_account(&self, account: &Account) {
        self.accounts_model().append(account);
        self.setup_tick_callback();
    }

    fn tokenize_search(account_name: &str, provider_name: &str, term: &str) -> bool {
        let term = term.to_ascii_lowercase();
        let provider_name = provider_name.to_ascii_lowercase();
        let account_name = account_name.to_ascii_lowercase();

        account_name.split_ascii_whitespace().any(|x| x == term)
            || provider_name.split_ascii_whitespace().any(|x| x == term)
            || account_name.contains(term.as_str())
            || provider_name.contains(term.as_str())
    }

    pub fn find_accounts(&self, terms: &[String]) -> Vec<Account> {
        let mut results = vec![];
        let model = self.accounts_model();
        let provider_name = self.name();
        for pos in 0..model.n_items() {
            let account = model.item(pos).and_downcast::<Account>().unwrap();
            let account_name = account.name();

            if terms
                .iter()
                .any(|term| Self::tokenize_search(&account_name, &provider_name, term))
            {
                results.push(account);
            }
        }
        results
    }

    pub fn accounts(&self) -> &gtk::FilterListModel {
        &self.imp().filter_model
    }

    pub fn filter(&self, text: String) {
        let filter = gtk::CustomFilter::new(
            glib::clone!(@weak self as provider => @default-return false, move |obj| {
                let account = obj.downcast_ref::<Account>().unwrap();
                let account_name = account.name();
                let provider_name = provider.name();

                Self::tokenize_search(&account_name, &provider_name, &text)
            }),
        );
        self.imp().filter_model.set_filter(Some(&filter));
    }

    pub fn remove_account(&self, account: &Account) {
        let imp = self.imp();
        let model = self.accounts_model();
        if let Some(pos) = model.find_position_by_id(account.id()) {
            model.remove(pos);
            if !self.has_accounts() && self.method().is_time_based() {
                // Stop ticking
                if let Some(source_id) = imp.tick_callback.borrow_mut().take() {
                    source_id.remove();
                }
            }
        }
    }
}

impl From<DieselProvider> for Provider {
    fn from(p: DieselProvider) -> Self {
        Self::new(
            p.id as u32,
            &p.name,
            p.period as u32,
            p.method.parse::<Method>().unwrap(),
            p.algorithm.parse::<Algorithm>().unwrap(),
            p.digits as u32,
            p.default_counter as u32,
            p.website,
            p.help_url,
            p.image_uri,
        )
    }
}

impl From<&Provider> for DieselProvider {
    fn from(p: &Provider) -> Self {
        Self {
            id: p.id() as i32,
            name: p.name(),
            period: p.period() as i32,
            method: p.method().to_string(),
            algorithm: p.algorithm().to_string(),
            digits: p.digits() as i32,
            default_counter: p.default_counter() as i32,
            website: p.website(),
            help_url: p.help_url(),
            image_uri: p.image_uri(),
        }
    }
}
