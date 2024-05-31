use std::cell::{Cell, RefCell};

use adw::subclass::navigation_page::*;
use gettextrs::gettext;
use gtk::{
    gio,
    glib::{self, clone},
    prelude::*,
    subclass::prelude::*,
};

use crate::{
    config,
    models::keyring,
    utils::{spawn, spawn_tokio},
    widgets::ErrorRevealer,
};

mod imp {
    use std::cell::OnceCell;

    use glib::subclass::InitializingObject;

    use super::*;

    #[derive(Default, gtk::CompositeTemplate, glib::Properties)]
    #[properties(wrapper_type = super::PasswordPage)]
    #[template(resource = "/com/belmoussaoui/Authenticator/preferences_password_page.ui")]
    pub struct PasswordPage {
        #[property(get, set, construct_only)]
        pub actions: OnceCell<gio::SimpleActionGroup>,
        #[property(get, set, construct)]
        pub has_set_password: Cell<bool>,
        #[template_child]
        pub current_password_entry: TemplateChild<adw::PasswordEntryRow>,
        #[template_child]
        pub error_revealer: TemplateChild<ErrorRevealer>,
        #[template_child]
        pub password_entry: TemplateChild<adw::PasswordEntryRow>,
        #[template_child]
        pub confirm_password_entry: TemplateChild<adw::PasswordEntryRow>,
        #[template_child]
        pub status_page: TemplateChild<adw::StatusPage>,
        pub default_password_signal: RefCell<Option<glib::SignalHandlerId>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PasswordPage {
        const NAME: &'static str = "PasswordPage";
        type Type = super::PasswordPage;
        type ParentType = adw::NavigationPage;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.bind_template_instance_callbacks();
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for PasswordPage {
        fn constructed(&self) {
            self.parent_constructed();
            let page = self.obj();
            page.setup_actions();
            self.status_page.set_icon_name(Some(config::APP_ID));
            page.reset_validation();
            // Reset the validation whenever the password state changes
            page.connect_has_set_password_notify(clone!(@weak page  => move |_| {
                page.reset_validation();
            }));
        }
    }

    impl WidgetImpl for PasswordPage {
        fn unmap(&self) {
            self.parent_unmap();
            self.obj().reset();
        }
    }

    impl NavigationPageImpl for PasswordPage {}
}

glib::wrapper! {
    pub struct PasswordPage(ObjectSubclass<imp::PasswordPage>)
        @extends gtk::Widget, adw::NavigationPage;
}

#[gtk::template_callbacks]
impl PasswordPage {
    pub fn new(actions: &gio::SimpleActionGroup) -> Self {
        glib::Object::builder().property("actions", actions).build()
    }

    #[template_callback]
    fn validate(&self, _entry: Option<gtk::Editable>) {
        let imp = self.imp();

        let current_password = imp.current_password_entry.text();
        let password = imp.password_entry.text();
        let password_repeat = imp.confirm_password_entry.text();

        let is_valid = if self.has_set_password() {
            password_repeat == password && current_password != password && password != ""
        } else {
            password_repeat == password && password != ""
        };

        let save_password_action = self
            .actions()
            .lookup_action("save_password")
            .and_downcast::<gio::SimpleAction>()
            .unwrap();
        save_password_action.set_enabled(is_valid);
    }

    // Called when either the user sets/resets the password to bind/unbind the
    // the validation callback on the password entry
    fn reset_validation(&self) {
        let imp = self.imp();
        if self.has_set_password() {
            imp.current_password_entry
                .connect_changed(clone!(@weak self as page => move |_| page.validate(None)));
        } else if let Some(handler_id) = imp.default_password_signal.borrow_mut().take() {
            imp.current_password_entry.disconnect(handler_id);
        }
    }
   //สามารภเรียกวช้ actions ได้
    fn setup_actions(&self) {
        let actions = self.actions();
        let save_password = gio::ActionEntry::builder("save_password")
            .activate(clone!(@weak self as page => move |_, _, _| {
                spawn(clone!(@weak page => async move {
                    page.save().await;
                }));
            }))
            .build();
        let reset_password = gio::ActionEntry::builder("reset_password")
            .activate(clone!(@weak self as page => move |_, _, _| {
                spawn(clone!(@weak page => async move {
                    page.reset_password().await;
                }));
            }))
            .build();
        actions.add_action_entries([save_password, reset_password]);

        let save_password_action = actions
            .lookup_action("save_password")
            .and_downcast::<gio::SimpleAction>()
            .unwrap();
        save_password_action.set_enabled(false);

        let reset_password_action = actions.lookup_action("reset_password").unwrap();
        self.bind_property("has-set-password", &reset_password_action, "enabled")
            .sync_create()
            .build();
    }

    async fn reset_password(&self) {
        let imp = self.imp();

        let current_password = imp.current_password_entry.text();
        let is_current_password =
            spawn_tokio(async move { keyring::is_current_password(&current_password).await })
                .await
                .unwrap_or(false);
        if self.has_set_password() && !is_current_password {
            imp.error_revealer.popup(&gettext("Wrong Passphrase"));
            return;
        }

        let password_was_reset = spawn_tokio(keyring::reset_password()).await.is_ok();

        if password_was_reset {
            let actions = self.actions();
            actions.activate_action("close_page", None);

            let save_password_action = actions
                .lookup_action("save_password")
                .and_downcast::<gio::SimpleAction>()
                .unwrap();

            save_password_action.set_enabled(false);
            self.set_has_set_password(false);
        }
    }

    pub fn reset(&self) {
        let imp = self.imp();

        imp.current_password_entry.get().set_text("");
        imp.password_entry.get().set_text("");
        imp.confirm_password_entry.get().set_text("");
    }

    async fn save(&self) {
        let imp = self.imp();
        let actions = self.actions();

        let current_password = imp.current_password_entry.text();
        let password = imp.password_entry.text();
        let is_current_password = spawn_tokio(async move {
            keyring::is_current_password(&current_password)
                .await
                .unwrap_or(false)
        })
        .await;

        if self.has_set_password() && is_current_password {
            imp.error_revealer.popup(&gettext("Wrong Passphrase"));
            return;
        }
        let password_was_set =
            spawn_tokio(async move { keyring::set_password(&password).await.is_ok() }).await;
        if password_was_set {
            self.reset();
            let save_password_action = actions
                .lookup_action("save_password")
                .and_downcast::<gio::SimpleAction>()
                .unwrap();

            save_password_action.set_enabled(false);
            self.set_has_set_password(true);
            actions.activate_action("close_page", None);
        }
    }
}
