use adw::{prelude::*, subclass::prelude::*};
use anyhow::Result;
use gettextrs::gettext;
use gtk::{
    gio,
    glib::{self, clone, ControlFlow},
};

use crate::{
    backup::RestorableItem,
    models::{Account, OTPUri, Provider, ProvidersModel, OTP},
    widgets::{providers::ProviderPage, screenshot, Camera, ErrorRevealer, ProviderImage, UrlRow},
};

mod imp {
    use std::cell::{OnceCell, RefCell};

    use glib::subclass::{InitializingObject, Signal};
    use once_cell::sync::Lazy;

    use super::*;

    #[derive(Default, gtk::CompositeTemplate, glib::Properties)]
    #[template(resource = "/com/belmoussaoui/Authenticator/account_add.ui")]
    #[properties(wrapper_type = super::AccountAddDialog)]
    pub struct AccountAddDialog {
        #[property(get, set, construct_only)]
        pub model: OnceCell<ProvidersModel>,
        pub selected_provider: RefCell<Option<Provider>>,
        #[template_child]
        pub camera: TemplateChild<Camera>,
        #[template_child]
        pub navigation_view: TemplateChild<adw::NavigationView>,
        #[template_child]
        pub image: TemplateChild<ProviderImage>,
        #[template_child]
        pub provider_website_row: TemplateChild<UrlRow>,
        #[template_child]
        pub provider_help_row: TemplateChild<UrlRow>,
        #[template_child]
        pub username_entry: TemplateChild<adw::EntryRow>,
        #[template_child]
        pub token_entry: TemplateChild<adw::PasswordEntryRow>,
        #[template_child]
        pub more_list: TemplateChild<gtk::ListBox>,
        #[template_child]
        pub period_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub digits_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub provider_entry: TemplateChild<gtk::Entry>,
        #[template_child]
        pub method_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub algorithm_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub counter_spinbutton: TemplateChild<adw::SpinRow>,
        #[template_child]
        pub period_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub provider_completion: TemplateChild<gtk::EntryCompletion>,
        #[template_child]
        pub error_revealer: TemplateChild<ErrorRevealer>,
        #[template_child]
        pub provider_page: TemplateChild<ProviderPage>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for AccountAddDialog {
        const NAME: &'static str = "AccountAddDialog";
        type Type = super::AccountAddDialog;
        type ParentType = adw::Dialog;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.bind_template_instance_callbacks();

            klass.install_action("add.previous", None, |dialog, _, _| {
                let imp = dialog.imp();
                if imp
                    .navigation_view
                    .visible_page()
                    .and_then(|page| page.tag())
                    .is_some_and(|tag| tag != "main")
                {
                    imp.navigation_view.pop();
                } else {
                    dialog.close();
                }
            });

            klass.install_action_async("add.qrcode", None, |dialog, _, _| async move {
                if let Err(err) = dialog.open_qr_code().await {
                    tracing::error!("Failed to load from QR Code file: {err}");
                }
            });

            klass.install_action("add.save", None, |dialog, _, _| {
                if dialog.save().is_ok() {
                    dialog.close();
                }
            });

            klass.install_action_async("add.camera", None, |dialog, _, _| async move {
                dialog.scan_from_camera().await;
            });

            klass.install_action_async("add.screenshot", None, |dialog, _, _| async move {
                dialog.scan_from_screenshot().await;
            });
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for AccountAddDialog {
        fn signals() -> &'static [Signal] {
            static SIGNALS: Lazy<Vec<Signal>> =
                Lazy::new(|| vec![Signal::builder("added").action().build()]);
            SIGNALS.as_ref()
        }

        fn constructed(&self) {
            self.parent_constructed();
            self.obj().action_set_enabled("add.save", false);

            self.provider_completion
                .set_model(Some(&self.model.get().unwrap().completion_model()));
        }
    }
    impl WidgetImpl for AccountAddDialog {}
    impl AdwDialogImpl for AccountAddDialog {}
}
glib::wrapper! {
    pub struct AccountAddDialog(ObjectSubclass<imp::AccountAddDialog>)
        @extends gtk::Widget, adw::Dialog;
}

#[gtk::template_callbacks]
impl AccountAddDialog {
    pub fn new(model: &ProvidersModel) -> Self {
        glib::Object::builder().property("model", model).build()
    }

    pub fn connect_added<F>(&self, callback: F) -> glib::SignalHandlerId
    where
        F: Fn(&Self) + 'static,
    {
        self.connect_local(
            "added",
            false,
            clone!(@weak self as dialog => @default-return None, move |_| {
                callback(&dialog);
                None
            }),
        )
    }

    #[template_callback]
    fn input_validate(&self, _: Option<gtk::Editable>) {
        let imp = self.imp();
        let username = imp.username_entry.text();
        let token = imp.token_entry.text();
        let has_provider = imp.selected_provider.borrow().is_some();

        let is_valid = !username.is_empty() && !token.is_empty() && has_provider;
        self.action_set_enabled("add.save", is_valid);
    }

    #[template_callback]
    fn match_selected(&self, store: gtk::ListStore, iter: gtk::TreeIter) -> ControlFlow {
        let provider_id = store.get::<u32>(&iter, 0);
        let provider = self.model().find_by_id(provider_id);
        self.set_provider(provider);

        ControlFlow::Break
    }

    #[template_callback]
    fn no_matches_selected(&self, completion: gtk::EntryCompletion) {
        // in case the provider doesn't exists, let the user create a new one by showing
        // a dialog for that TODO: replace this whole completion provider thing
        // with a custom widget
        let imp = self.imp();
        let entry = completion.entry().unwrap();

        imp.navigation_view.push_by_tag("create-provider");
        imp.provider_page.set_provider(None);

        let name_entry = imp.provider_page.name_entry();
        name_entry.set_text(&entry.text());
        name_entry.set_position(entry.cursor_position());
    }

    #[template_callback]
    fn camera_closed(&self, _camera: Camera) {
        self.activate_action("add.previous", None).unwrap();
    }

    #[template_callback]
    fn camera_code_detected(&self, code: &str, _camera: Camera) {
        match code.parse::<OTPUri>() {
            Ok(otp_uri) => {
                self.set_from_otp_uri(&otp_uri);
            }
            Err(err) => {
                tracing::error!("Failed to parse OTP uri code {err}");
            }
        }
    }

    #[template_callback]
    fn provider_created(&self, provider: Provider, _page: ProviderPage) {
        let imp = self.imp();
        let model = self.model();
        model.append(&provider);

        imp.provider_completion
            .set_model(Some(&model.completion_model()));
        self.set_provider(Some(provider));
        imp.navigation_view.pop();
    }

    async fn scan_from_screenshot(&self) {
        if let Err(err) = self.imp().camera.scan_from_screenshot().await {
            tracing::error!("Failed to scan from screenshot {}", err);
        }
    }

    async fn scan_from_camera(&self) {
        let imp = self.imp();
        imp.camera.scan_from_camera().await;
        imp.navigation_view.push_by_tag("camera");
    }

    pub fn set_from_otp_uri(&self, otp_uri: &OTPUri) {
        let imp = self.imp();
        imp.navigation_view.pop(); // Switch back the form view

        imp.token_entry.set_text(&otp_uri.secret());
        imp.username_entry.set_text(&otp_uri.account());

        let provider = self
            .model()
            .find_or_create(
                &otp_uri.issuer(),
                otp_uri.period(),
                otp_uri.method(),
                None,
                otp_uri.algorithm(),
                otp_uri.digits(),
                otp_uri.counter(),
                None,
                None,
            )
            .ok();

        self.set_provider(provider);
    }

    async fn open_qr_code(&self) -> Result<()> {
        let window = self.root().and_downcast::<gtk::Window>().unwrap();

        let images_filter = gtk::FileFilter::new();
        images_filter.set_name(Some(&gettext("Image")));
        images_filter.add_pixbuf_formats();
        let model = gio::ListStore::new::<gtk::FileFilter>();
        model.append(&images_filter);

        let dialog = gtk::FileDialog::builder()
            .modal(true)
            .filters(&model)
            .title(gettext("Select QR Code"))
            .build();
        let file = dialog.open_future(Some(&window)).await?;
        let (data, _) = file.load_contents_future().await?;
        let code = screenshot::scan(&data)?;
        let uri = code.parse::<OTPUri>()?;
        self.set_from_otp_uri(&uri);
        Ok(())
    }

    fn save(&self) -> Result<()> {
        let imp = self.imp();

        if let Some(ref provider) = *imp.selected_provider.borrow() {
            let username = imp.username_entry.text();
            let token = imp.token_entry.text();
            let token = token.trim_end_matches('=');
            if !OTP::is_valid(token) {
                imp.error_revealer.popup(&gettext("Invalid Token"));
                anyhow::bail!("Token {} is not a valid Base32 secret", &token);
            }

            let account = Account::create(&username, token, None, provider)?;

            self.model().add_account(&account, provider);
            self.emit_by_name::<()>("added", &[]);
        // TODO: display an error message saying there was an error form keyring
        } else {
            anyhow::bail!("Could not find provider");
        }
        Ok(())
    }

    fn set_provider(&self, provider: Option<Provider>) {
        let imp = self.imp();
        if let Some(provider) = provider {
            imp.more_list.set_visible(true);
            imp.provider_entry.set_text(&provider.name());
            imp.period_label.set_text(&provider.period().to_string());

            imp.image.set_provider(Some(&provider));

            imp.method_label
                .set_text(&provider.method().to_locale_string());

            imp.algorithm_label
                .set_text(&provider.algorithm().to_locale_string());

            imp.digits_label.set_text(&provider.digits().to_string());

            if provider.method().is_time_based() {
                imp.counter_spinbutton.set_visible(false);
                imp.period_row.set_visible(true);
            } else {
                imp.counter_spinbutton.set_visible(true);
                imp.period_row.set_visible(false);
            }

            if let Some(website) = provider.website() {
                imp.provider_website_row.set_uri(website);
            }
            if let Some(help_url) = provider.help_url() {
                imp.provider_help_row.set_uri(help_url);
            }
            imp.selected_provider.borrow_mut().replace(provider);
        } else {
            imp.selected_provider.borrow_mut().take();
        }
        self.input_validate(None);
    }
}
