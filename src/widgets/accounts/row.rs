use gtk::{gdk, glib, prelude::*};

use crate::models::Account;

mod imp {
    use std::cell::OnceCell;

    use adw::subclass::prelude::*;
    use gettextrs::gettext;
    use glib::subclass;

    use super::*;
    use crate::widgets::Window;

    #[derive(Default, gtk::CompositeTemplate, glib::Properties)]
    #[properties(wrapper_type = super::AccountRow)]
    #[template(resource = "/com/belmoussaoui/Authenticator/account_row.ui")]
    pub struct AccountRow {
        #[property(get, set, construct_only)]
        pub account: OnceCell<Account>,
        #[template_child]
        pub increment_btn: TemplateChild<gtk::Button>,
        #[template_child]
        pub otp_label: TemplateChild<gtk::Label>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for AccountRow {
        const NAME: &'static str = "AccountRow";
        type Type = super::AccountRow;
        type ParentType = adw::ActionRow;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();

            klass.add_binding_action(
                gdk::Key::c,
                gdk::ModifierType::CONTROL_MASK,
                "account.copy-otp",
            );

            klass.install_action("account.copy-otp", None, |row, _, _| {
                row.account().copy_otp();
                let window = row.root().and_downcast::<Window>().unwrap();
                let toast = adw::Toast::new(&gettext("One-Time password copied"));
                toast.set_timeout(3);
                window.add_toast(toast);
            });
            klass.install_action("account.increment-counter", None, |row, _, _| {
                match row.account().increment_counter() {
                    Ok(_) => row.account().generate_otp(),
                    Err(err) => tracing::error!("Failed to increment the counter {err}"),
                };
            });
        }

        fn instance_init(obj: &subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for AccountRow {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();
            let account = obj.account();
            account
                .bind_property("name", &*obj, "title")
                .sync_create()
                .build();

            account
                .bind_property("name", &*obj, "tooltip-text")
                .sync_create()
                .build();

            account
                .bind_property("code", &*self.otp_label, "label")
                .sync_create()
                .build();

            // Only display the increment button if it is a HOTP account
            self.increment_btn
                .set_visible(account.provider().method().is_event_based());
        }
    }
    impl WidgetImpl for AccountRow {}
    impl ListBoxRowImpl for AccountRow {}
    impl PreferencesRowImpl for AccountRow {}
    impl ActionRowImpl for AccountRow {}
}

glib::wrapper! {
    pub struct AccountRow(ObjectSubclass<imp::AccountRow>)
        @extends gtk::Widget, gtk::ListBoxRow, adw::PreferencesRow, adw::ActionRow;
}

impl AccountRow {
    pub fn new(account: &Account) -> Self {
        glib::Object::builder().property("account", account).build()
    }
}
