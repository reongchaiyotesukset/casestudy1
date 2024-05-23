use adw::prelude::*;
use gtk::{
    gio,
    glib::{self, clone},
};

mod imp {
    use std::cell::RefCell;

    use adw::subclass::prelude::*;

    use super::*;
    use crate::utils::spawn;

    #[derive(Default, glib::Properties)]
    #[properties(wrapper_type = super::UrlRow)]
    pub struct UrlRow {
        #[property(get, set = Self::set_uri)]
        pub uri: RefCell<Option<String>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for UrlRow {
        const NAME: &'static str = "UrlRow";
        type Type = super::UrlRow;
        type ParentType = adw::ActionRow;
    }

    #[glib::derived_properties]
    impl ObjectImpl for UrlRow {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();
            obj.set_activatable(true);
            obj.connect_activated(clone!(@weak obj as row => move |_| {
                if let Some(uri) = row.imp().uri.borrow().clone() {
                    spawn(async move {
                        let file = gio::File::for_uri(&uri);
                        let launcher = gtk::FileLauncher::new(Some(&file));
                        if let Err(err) = launcher.launch_future(gtk::Window::NONE).await {
                            tracing::error!("Failed to open URI {err}");
                        }
                    });
                };
            }));

            let image_suffix = gtk::Image::from_icon_name("link-symbolic");
            image_suffix.set_accessible_role(gtk::AccessibleRole::Presentation);
            obj.add_suffix(&image_suffix);
        }
    }
    impl WidgetImpl for UrlRow {}
    impl ListBoxRowImpl for UrlRow {}
    impl PreferencesRowImpl for UrlRow {}
    impl ActionRowImpl for UrlRow {}

    impl UrlRow {
        pub fn set_uri(&self, uri: &str) {
            self.obj().set_subtitle(uri);
            self.uri.borrow_mut().replace(uri.to_owned());
        }
    }
}

glib::wrapper! {
    pub struct UrlRow(ObjectSubclass<imp::UrlRow>)
        @extends gtk::Widget, gtk::ListBoxRow, adw::PreferencesRow, adw::ActionRow;
}
