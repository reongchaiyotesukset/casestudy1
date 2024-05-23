use adw::{prelude::*, subclass::navigation_page::*};
use gettextrs::gettext;
use gtk::{
    gdk,
    glib::{self, clone, ControlFlow},
    subclass::prelude::*,
};

use super::{QRCodeData, QRCodePaintable};
use crate::{
    models::{Account, Provider, ProvidersModel},
    widgets::UrlRow,
};
mod imp {
    use std::cell::{OnceCell, RefCell};

    use glib::subclass::Signal;
    use once_cell::sync::Lazy;

    use super::*;

    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/com/belmoussaoui/Authenticator/account_details_page.ui")]
    pub struct AccountDetailsPage {
        #[template_child]
        pub website_row: TemplateChild<UrlRow>,
        #[template_child]
        pub qrcode_picture: TemplateChild<gtk::Picture>,
        #[template_child]
        pub account_label: TemplateChild<adw::EntryRow>,
        #[template_child(id = "list")]
        pub listbox: TemplateChild<gtk::ListBox>,
        #[template_child]
        pub algorithm_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub method_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub counter_spinbutton: TemplateChild<adw::SpinRow>,
        #[template_child]
        pub period_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub digits_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub period_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub help_row: TemplateChild<UrlRow>,
        pub qrcode_paintable: QRCodePaintable,
        pub account: RefCell<Option<Account>>,
        #[template_child]
        pub provider_completion: TemplateChild<gtk::EntryCompletion>,
        #[template_child]
        pub provider_entry: TemplateChild<gtk::Entry>,
        pub selected_provider: RefCell<Option<Provider>>,
        pub providers_model: OnceCell<ProvidersModel>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for AccountDetailsPage {
        const NAME: &'static str = "AccountDetailsPage";
        type Type = super::AccountDetailsPage;
        type ParentType = adw::NavigationPage;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.bind_template_instance_callbacks();

            klass.install_action("account.delete", None, |page, _, _| {
                page.delete_account();
            });
            klass.install_action("account.save", None, |page, _, _| {
                if let Err(err) = page.save() {
                    tracing::error!("Failed to save account details {}", err);
                }
            });

            klass.install_action("account.back", None, |page, _, _| {
                page.activate_action("win.back", None).unwrap();
            });

            klass.add_binding_action(gdk::Key::Escape, gdk::ModifierType::empty(), "account.back");
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for AccountDetailsPage {
        fn signals() -> &'static [Signal] {
            static SIGNALS: Lazy<Vec<Signal>> = Lazy::new(|| {
                vec![
                    Signal::builder("removed")
                        .param_types([Account::static_type()])
                        .action()
                        .build(),
                    Signal::builder("provider-changed").action().build(),
                ]
            });
            SIGNALS.as_ref()
        }

        fn constructed(&self) {
            self.parent_constructed();
            self.qrcode_picture
                .set_paintable(Some(&self.qrcode_paintable));
        }
    }
    impl WidgetImpl for AccountDetailsPage {}
    impl NavigationPageImpl for AccountDetailsPage {}
}

glib::wrapper! {
    pub struct AccountDetailsPage(ObjectSubclass<imp::AccountDetailsPage>)
        @extends gtk::Widget, adw::NavigationPage;
}

#[gtk::template_callbacks]
impl AccountDetailsPage {
    fn delete_account(&self) {
        let parent = self.root().and_downcast::<gtk::Window>().unwrap();

        let dialog = adw::AlertDialog::builder()
            .heading(gettext("Are you sure you want to delete the account?"))
            .body(gettext("This action is irreversible"))
            .build();
        dialog.add_responses(&[("no", &gettext("No")), ("yes", &gettext("Yes"))]);
        dialog.set_response_appearance("yes", adw::ResponseAppearance::Destructive);
        dialog.connect_response(
            None,
            clone!(@weak self as page => move |dialog, response| {
                if response == "yes" {
                    let account = page.imp().account.borrow().as_ref().unwrap().clone();
                    page.emit_by_name::<()>("removed", &[&account]);
                }
                dialog.close();
            }),
        );

        dialog.present(&parent);
    }

    pub fn set_account(&self, account: &Account) {
        let imp = self.imp();
        let qr_code = QRCodeData::from(String::from(account.otp_uri()));
        imp.qrcode_paintable.set_qrcode(qr_code);

        if account.provider().method().is_event_based() {
            imp.counter_spinbutton.set_value(account.counter() as f64);
        }
        self.set_provider(account.provider());
        imp.account_label.set_text(&account.name());
        imp.account.replace(Some(account.clone()));
    }

    pub fn set_providers_model(&self, model: ProvidersModel) {
        self.imp()
            .provider_completion
            .set_model(Some(&model.completion_model()));
        self.imp().providers_model.set(model).unwrap();
    }

    fn set_provider(&self, provider: Provider) {
        let imp = self.imp();
        imp.provider_entry.set_text(&provider.name());
        imp.algorithm_label
            .set_text(&provider.algorithm().to_locale_string());
        imp.method_label
            .set_text(&provider.method().to_locale_string());
        if provider.method().is_event_based() {
            imp.counter_spinbutton.set_visible(true);
            imp.period_row.set_visible(false);
        } else {
            imp.counter_spinbutton.set_visible(false);
            imp.period_row.set_visible(true);
            imp.period_label.set_text(&provider.period().to_string());
        }
        imp.digits_label.set_text(&provider.digits().to_string());
        if let Some(help) = provider.help_url() {
            imp.help_row.set_uri(help);
            imp.help_row.set_visible(true);
        } else {
            imp.help_row.set_visible(false);
        }
        if let Some(website) = provider.website() {
            imp.website_row.set_uri(website);
            imp.website_row.set_visible(true);
        } else {
            imp.website_row.set_visible(false);
        }
        imp.selected_provider.replace(Some(provider));
    }

    fn save(&self) -> anyhow::Result<()> {
        let imp = self.imp();

        if let Some(account) = imp.account.borrow().as_ref() {
            account.set_name(imp.account_label.text());

            if let Some(selected_provider) = imp.selected_provider.borrow().as_ref() {
                let current_provider = account.provider();
                if selected_provider.id() != current_provider.id() {
                    selected_provider.add_account(account);
                    current_provider.remove_account(account);
                    account.set_provider(selected_provider)?;
                    imp.provider_entry.set_text(&selected_provider.name());
                    self.emit_by_name::<()>("provider-changed", &[]);
                }
            }

            let old_counter = account.counter();
            account.set_counter(imp.counter_spinbutton.value() as u32);
            // regenerate the otp value if the counter value was changed
            if old_counter != account.counter() && account.provider().method().is_event_based() {
                account.generate_otp();
            }
        }
        Ok(())
    }

    #[template_callback]
    fn provider_match_selected(&self, store: gtk::ListStore, iter: gtk::TreeIter) -> ControlFlow {
        let provider_id = store.get::<u32>(&iter, 0);
        let model = self.imp().providers_model.get().unwrap();
        let provider = model.find_by_id(provider_id);
        self.set_provider(
            provider.unwrap_or_else(clone!(@strong self as page => move || {
                page.imp().account.borrow().as_ref().unwrap().provider()
            })),
        );
        ControlFlow::Break
    }
}
