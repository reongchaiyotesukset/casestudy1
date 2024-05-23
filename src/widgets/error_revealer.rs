use gtk::{glib, prelude::*, subclass::prelude::*};

mod imp {
    use super::*;

    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/com/belmoussaoui/Authenticator/error_revealer.ui")]
    pub struct ErrorRevealer {
        #[template_child]
        pub label: TemplateChild<gtk::Label>,
        #[template_child]
        pub revealer: TemplateChild<gtk::Revealer>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ErrorRevealer {
        const NAME: &'static str = "ErrorRevealer";
        type Type = super::ErrorRevealer;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.set_layout_manager_type::<gtk::BinLayout>();
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for ErrorRevealer {
        fn dispose(&self) {
            self.revealer.unparent();
            self.label.unparent();
        }
    }

    impl WidgetImpl for ErrorRevealer {}
}

glib::wrapper! {
    pub struct ErrorRevealer(ObjectSubclass<imp::ErrorRevealer>)
        @extends gtk::Widget;
}

impl ErrorRevealer {
    pub fn popup(&self, text: &str) {
        let imp = self.imp();
        imp.label.set_text(text);
        self.set_visible(true);
        imp.revealer.set_reveal_child(true);
        glib::timeout_add_seconds_local(
            2,
            glib::clone!(@weak self as error_revealer => @default-return glib::ControlFlow::Break, move || {
                error_revealer.imp().revealer.set_reveal_child(false);
                error_revealer.set_visible(false);
                glib::ControlFlow::Break
            }),
        );
    }
}
