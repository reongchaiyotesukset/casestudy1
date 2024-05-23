use adw::{
    prelude::*,
    subclass::{navigation_page::*, prelude::*},
};
use gettextrs::gettext;
use gtk::{
    gdk_pixbuf, gio,
    glib::{self, translate::IntoGlib},
};

use crate::{
    models::{i18n, Algorithm, Method, Provider, ProviderPatch, FAVICONS_PATH, OTP},
    widgets::{ErrorRevealer, ProviderImage},
};

mod imp {
    use std::cell::RefCell;

    use glib::subclass::Signal;

    use super::*;

    #[derive(gtk::CompositeTemplate)]
    #[template(resource = "/com/belmoussaoui/Authenticator/provider_page.ui")]
    pub struct ProviderPage {
        pub actions: gio::SimpleActionGroup,
        pub methods_model: adw::EnumListModel,
        pub algorithms_model: adw::EnumListModel,
        #[template_child]
        pub error_revealer: TemplateChild<ErrorRevealer>,
        #[template_child]
        pub image: TemplateChild<ProviderImage>,
        #[template_child]
        pub name_entry: TemplateChild<adw::EntryRow>,
        #[template_child]
        pub period_spinbutton: TemplateChild<adw::SpinRow>,
        #[template_child]
        pub digits_spinbutton: TemplateChild<adw::SpinRow>,
        #[template_child]
        pub default_counter_spinbutton: TemplateChild<adw::SpinRow>,
        #[template_child]
        pub provider_website_entry: TemplateChild<adw::EntryRow>,
        #[template_child]
        pub provider_help_entry: TemplateChild<adw::EntryRow>,
        #[template_child]
        pub method_comborow: TemplateChild<adw::ComboRow>,
        #[template_child]
        pub algorithm_comborow: TemplateChild<adw::ComboRow>,
        #[template_child]
        pub delete_button: TemplateChild<gtk::Button>,
        pub selected_provider: RefCell<Option<Provider>>,
        pub selected_image: RefCell<Option<gio::File>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ProviderPage {
        const NAME: &'static str = "ProviderPage";
        type Type = super::ProviderPage;
        type ParentType = adw::NavigationPage;

        fn new() -> Self {
            let methods_model = adw::EnumListModel::new(Method::static_type());
            let algorithms_model = adw::EnumListModel::new(Algorithm::static_type());

            Self {
                actions: gio::SimpleActionGroup::new(),
                image: TemplateChild::default(),
                error_revealer: TemplateChild::default(),
                name_entry: TemplateChild::default(),
                period_spinbutton: TemplateChild::default(),
                digits_spinbutton: TemplateChild::default(),
                default_counter_spinbutton: TemplateChild::default(),
                provider_website_entry: TemplateChild::default(),
                provider_help_entry: TemplateChild::default(),
                method_comborow: TemplateChild::default(),
                algorithm_comborow: TemplateChild::default(),
                delete_button: TemplateChild::default(),
                methods_model,
                algorithms_model,
                selected_provider: RefCell::default(),
                selected_image: RefCell::default(),
            }
        }

        fn class_init(klass: &mut Self::Class) {
            Method::static_type();
            Algorithm::static_type();
            klass.bind_template();
            klass.bind_template_instance_callbacks();

            klass.install_action("providers.save", None, |page, _, _| {
                if let Err(err) = page.save() {
                    tracing::warn!("Failed to save provider {}", err);
                }
            });
            klass.install_action("providers.delete", None, |page, _, _| {
                if let Err(err) = page.delete_provider() {
                    tracing::warn!("Failed to delete the provider {}", err);
                }
            });

            klass.install_action("providers.reset_image", None, |page, _, _| {
                page.reset_image();
            });
            klass.install_action_async("providers.select_image", None, |page, _, _| async move {
                page.open_select_image().await;
            });
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for ProviderPage {
        fn signals() -> &'static [Signal] {
            use once_cell::sync::Lazy;
            static SIGNALS: Lazy<Vec<Signal>> = Lazy::new(|| {
                vec![
                    Signal::builder("created")
                        .param_types([Provider::static_type()])
                        .build(),
                    Signal::builder("updated")
                        .param_types([Provider::static_type()])
                        .build(),
                    Signal::builder("deleted")
                        .param_types([Provider::static_type()])
                        .build(),
                ]
            });
            SIGNALS.as_ref()
        }

        fn constructed(&self) {
            self.parent_constructed();
            self.algorithm_comborow
                .set_model(Some(&self.algorithms_model));
            self.method_comborow.set_model(Some(&self.methods_model));
            self.obj().action_set_enabled("providers.save", false);
        }
    }
    impl WidgetImpl for ProviderPage {}
    impl NavigationPageImpl for ProviderPage {}
}

glib::wrapper! {
    pub struct ProviderPage(ObjectSubclass<imp::ProviderPage>)
        @extends gtk::Widget, adw::NavigationPage;
}

#[gtk::template_callbacks]
impl ProviderPage {
    pub fn set_provider(&self, provider: Option<Provider>) {
        let imp = self.imp();
        if let Some(provider) = provider {
            imp.delete_button.set_visible(true);
            imp.name_entry.set_text(&provider.name());
            imp.period_spinbutton.set_value(provider.period() as f64);

            if let Some(ref website) = provider.website() {
                imp.provider_website_entry.set_text(website);
            } else {
                imp.provider_website_entry.set_text("");
            }

            if let Some(ref website) = provider.help_url() {
                imp.provider_help_entry.set_text(website);
            } else {
                imp.provider_help_entry.set_text("");
            }

            imp.algorithm_comborow.set_selected(
                imp.algorithms_model
                    .find_position(provider.algorithm().into_glib()),
            );

            imp.default_counter_spinbutton
                .set_value(provider.default_counter() as f64);
            imp.digits_spinbutton.set_value(provider.digits() as f64);

            imp.method_comborow.set_selected(
                imp.methods_model
                    .find_position(provider.method().into_glib()),
            );
            imp.image.set_provider(Some(&provider));
            self.set_title(&i18n::i18n_f("Editing Provider: {}", &[&provider.name()]));
            imp.selected_provider.replace(Some(provider));
        } else {
            imp.name_entry.set_text("");
            imp.delete_button.set_visible(false);
            imp.period_spinbutton.set_value(OTP::DEFAULT_PERIOD as f64);
            imp.provider_website_entry.set_text("");
            imp.provider_help_entry.set_text("");

            imp.algorithm_comborow.set_selected(
                imp.algorithms_model
                    .find_position(Algorithm::default().into_glib()),
            );

            imp.default_counter_spinbutton
                .set_value(OTP::DEFAULT_COUNTER as f64);
            imp.digits_spinbutton.set_value(OTP::DEFAULT_DIGITS as f64);

            imp.method_comborow.set_selected(
                imp.methods_model
                    .find_position(Method::default().into_glib()),
            );
            imp.image.set_provider(None::<Provider>);
            self.set_title(&gettext("New Provider"));
            imp.selected_provider.replace(None);
        }
    }

    pub fn name_entry(&self) -> adw::EntryRow {
        self.imp().name_entry.clone()
    }

    // Save the provider & emit a signal when one is created/updated
    fn save(&self) -> anyhow::Result<()> {
        let imp = self.imp();

        let name = imp.name_entry.text();
        let website = imp.provider_website_entry.text().to_string();
        let help_url = imp.provider_help_entry.text().to_string();
        let period = imp.period_spinbutton.value() as u32;
        let digits = imp.digits_spinbutton.value() as u32;
        let method = Method::from(imp.method_comborow.selected());
        let algorithm = Algorithm::from(imp.algorithm_comborow.selected());
        let default_counter = imp.default_counter_spinbutton.value() as u32;

        let image_uri = if let Some(file) = imp.selected_image.borrow().as_ref() {
            let basename = file.basename().unwrap();
            let icon_name = glib::base64_encode(basename.to_str().unwrap().as_bytes());
            let small_icon_name = format!("{icon_name}_32x32");
            let large_icon_name = format!("{icon_name}_96x96");

            // Create a 96x96 & 32x32 variants
            let stream = file.read(gio::Cancellable::NONE)?;
            let pixbuf = gdk_pixbuf::Pixbuf::from_stream(&stream, gio::Cancellable::NONE)?;
            tracing::debug!("Creating a 32x32 variant of the selected favicon");
            let small_pixbuf = pixbuf
                .scale_simple(32, 32, gdk_pixbuf::InterpType::Bilinear)
                .unwrap();
            small_pixbuf.savev(FAVICONS_PATH.join(small_icon_name), "png", &[])?;

            tracing::debug!("Creating a 96x96 variant of the selected favicon");
            let large_pixbuf = pixbuf
                .scale_simple(96, 96, gdk_pixbuf::InterpType::Bilinear)
                .unwrap();
            large_pixbuf.savev(FAVICONS_PATH.join(large_icon_name), "png", &[])?;

            Some(icon_name.to_string())
        } else {
            None
        };

        if let Some(provider) = imp.selected_provider.borrow().as_ref() {
            provider.update(&ProviderPatch {
                name: name.to_string(),
                website: Some(website),
                help_url: Some(help_url),
                image_uri,
                period: period as i32,
                digits: digits as i32,
                default_counter: default_counter as i32,
                algorithm: algorithm.to_string(),
                method: method.to_string(),
                is_backup_restore: false,
            })?;
            self.emit_by_name::<()>("updated", &[provider]);
        } else {
            let provider = Provider::create(
                &name,
                period,
                algorithm,
                Some(website),
                method,
                digits,
                default_counter,
                Some(help_url),
                image_uri,
            )?;
            self.emit_by_name::<()>("created", &[&provider]);
        }
        Ok(())
    }

    async fn open_select_image(&self) {
        let parent = self.root().and_downcast::<gtk::Window>().unwrap();

        let images_filter = gtk::FileFilter::new();
        images_filter.set_name(Some(&gettext("Image")));
        images_filter.add_pixbuf_formats();
        let model = gio::ListStore::new::<gtk::FileFilter>();
        model.append(&images_filter);

        let file_chooser = gtk::FileDialog::builder()
            .modal(true)
            .filters(&model)
            .build();

        if let Ok(file) = file_chooser.open_future(Some(&parent)).await {
            self.set_image(file);
        };
    }

    fn set_image(&self, file: gio::File) {
        let imp = self.imp();

        imp.image.set_from_file(&file);
        imp.selected_image.replace(Some(file));
    }

    fn reset_image(&self) {
        let imp = self.imp();
        imp.image.reset();
        imp.selected_image.replace(None);
    }

    fn delete_provider(&self) -> anyhow::Result<()> {
        let imp = self.imp();
        if let Some(provider) = imp.selected_provider.borrow().as_ref() {
            if provider.has_accounts() {
                imp.error_revealer.popup(&gettext(
                    "The provider has accounts assigned to it, please remove them first",
                ));
            } else if provider.delete().is_ok() {
                self.emit_by_name::<()>("deleted", &[provider]);
            }
        } else {
            anyhow::bail!("Can't remove a provider as none are selected");
        }
        Ok(())
    }

    // Validate the information typed by the user in order to enable/disable the
    // save action Note that we don't validate the urls other than: does `url`
    // crate can parse it or not
    #[template_callback]
    fn entry_validate(&self, _entry: adw::EntryRow) {
        let imp = self.imp();

        let provider_name = imp.name_entry.text();
        let provider_website = imp.provider_website_entry.text();
        let provider_help_url = imp.provider_help_entry.text();

        let is_valid = !provider_name.is_empty()
            && (provider_website.is_empty() || url::Url::parse(&provider_website).is_ok())
            && (provider_help_url.is_empty() || url::Url::parse(&provider_help_url).is_ok());

        self.action_set_enabled("providers.save", is_valid);
    }

    #[template_callback]
    fn on_method_changed(&self, _pspec: glib::ParamSpec, combo_row: adw::ComboRow) {
        let imp = self.imp();

        let selected = Method::from(combo_row.selected());
        match selected {
            Method::TOTP => {
                imp.default_counter_spinbutton.set_visible(false);
                imp.period_spinbutton.set_visible(true);
                imp.digits_spinbutton.set_value(OTP::DEFAULT_DIGITS as f64);
                imp.period_spinbutton.set_value(OTP::DEFAULT_PERIOD as f64);
            }
            Method::HOTP => {
                imp.default_counter_spinbutton.set_visible(true);
                imp.period_spinbutton.set_visible(false);
                imp.default_counter_spinbutton
                    .set_value(OTP::DEFAULT_COUNTER as f64);
                imp.digits_spinbutton.set_value(OTP::DEFAULT_DIGITS as f64);
            }
            Method::Steam => {
                imp.default_counter_spinbutton.set_visible(false);
                imp.period_spinbutton.set_visible(true);
                imp.digits_spinbutton
                    .set_value(OTP::STEAM_DEFAULT_DIGITS as f64);
                imp.period_spinbutton
                    .set_value(OTP::STEAM_DEFAULT_PERIOD as f64);
                imp.algorithm_comborow
                    .set_selected(Algorithm::default().into_glib() as u32);
            }
        }

        imp.algorithm_comborow
            .set_sensitive(selected != Method::Steam);
        imp.period_spinbutton
            .set_sensitive(selected != Method::Steam);
        imp.digits_spinbutton
            .set_sensitive(selected != Method::Steam);
    }

    #[template_callback]
    fn otp_method_to_locale_string(item: adw::EnumListItem) -> String {
        let method = Method::from(item.value() as u32);
        method.to_locale_string()
    }

    #[template_callback]
    fn algorithm_to_locale_string(item: adw::EnumListItem) -> String {
        let algorithm = Algorithm::from(item.value() as u32);
        algorithm.to_locale_string()
    }
}

impl Default for ProviderPage {
    fn default() -> Self {
        glib::Object::new()
    }
}
