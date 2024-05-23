use gtk::{glib, prelude::*, subclass::prelude::*};

use crate::models::Provider;

mod imp {
    use std::cell::OnceCell;

    use gtk::pango;

    use super::*;

    #[derive(Default, glib::Properties)]
    #[properties(wrapper_type = super::ProviderActionRow)]
    pub struct ProviderActionRow {
        #[property(get, set, construct_only)]
        pub provider: OnceCell<Provider>,
        pub title_label: gtk::Label,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ProviderActionRow {
        const NAME: &'static str = "ProviderActionRow";
        type Type = super::ProviderActionRow;
        type ParentType = gtk::ListBoxRow;
    }

    #[glib::derived_properties]
    impl ObjectImpl for ProviderActionRow {
        fn constructed(&self) {
            self.parent_constructed();
            self.title_label.set_margin_bottom(12);
            self.title_label.set_margin_end(6);
            self.title_label.set_margin_top(12);
            self.title_label.set_margin_start(6);
            self.title_label.set_valign(gtk::Align::Center);
            self.title_label.set_halign(gtk::Align::Start);
            self.title_label.set_wrap(true);
            self.title_label.set_ellipsize(pango::EllipsizeMode::End);

            self.obj()
                .provider()
                .bind_property("name", &self.title_label, "label")
                .sync_create()
                .build();

            self.obj().set_child(Some(&self.title_label));
        }
    }
    impl WidgetImpl for ProviderActionRow {}
    impl ListBoxRowImpl for ProviderActionRow {}
}

glib::wrapper! {
pub struct ProviderActionRow(ObjectSubclass<imp::ProviderActionRow>)
    @extends gtk::Widget, gtk::ListBoxRow;
}

impl ProviderActionRow {
    pub fn new(provider: &Provider) -> Self {
        glib::Object::builder()
            .property("provider", provider)
            .build()
    }
}
