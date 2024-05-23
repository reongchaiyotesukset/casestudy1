use adw::subclass::prelude::*;
use gtk::{gio, glib, prelude::*};

mod imp {
    use super::*;

    #[derive(Debug, Default, gtk::CompositeTemplate)]
    #[template(resource = "/com/belmoussaoui/Authenticator/keyring_error_dialog.ui")]
    pub struct KeyringErrorDialog {
        #[template_child]
        status_page: TemplateChild<adw::StatusPage>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for KeyringErrorDialog {
        const NAME: &'static str = "KeyringErrorDialog";
        type Type = super::KeyringErrorDialog;
        type ParentType = adw::ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for KeyringErrorDialog {
        fn constructed(&self) {
            self.parent_constructed();
            self.status_page.set_icon_name(Some(crate::config::APP_ID));
        }
    }
    impl WidgetImpl for KeyringErrorDialog {}
    impl WindowImpl for KeyringErrorDialog {}
    impl ApplicationWindowImpl for KeyringErrorDialog {}
    impl AdwWindowImpl for KeyringErrorDialog {}
    impl AdwApplicationWindowImpl for KeyringErrorDialog {}
}

glib::wrapper! {
    pub struct KeyringErrorDialog(ObjectSubclass<imp::KeyringErrorDialog>)
        @extends gtk::Widget, adw::ApplicationWindow, gtk::Window;
}

impl KeyringErrorDialog {
    pub fn new<A: IsA<gio::Application>>(app: &A) -> Self {
        glib::Object::builder().property("application", app).build()
    }
}
