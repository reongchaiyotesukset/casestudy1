use gtk::{
    gio,
    glib::{self, clone},
    prelude::*,
    subclass::prelude::*,
};

use crate::{
    models::{Provider, FAVICONS_PATH, RUNTIME, SETTINGS},
    utils::spawn,
};

mod imp {
    use std::cell::{Cell, RefCell};

    use glib::subclass;

    use super::*;
    #[derive(gtk::CompositeTemplate, glib::Properties)]
    #[properties(wrapper_type  = super::ProviderImage)]
    #[template(resource = "/com/belmoussaoui/Authenticator/provider_image.ui")]
    pub struct ProviderImage {
        #[property(get, set, minimum = 32, maximum = 96, default = 48, construct)]
        pub size: Cell<u32>,
        pub was_downloaded: Cell<bool>,
        #[property(get, set = Self::set_provider, nullable)]
        pub provider: RefCell<Option<Provider>>,
        #[template_child]
        pub stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub image: TemplateChild<gtk::Image>,
        #[template_child]
        pub spinner: TemplateChild<gtk::Spinner>,
        pub signal_id: RefCell<Option<glib::SignalHandlerId>>,
        pub join_handle: RefCell<Option<tokio::task::JoinHandle<()>>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ProviderImage {
        const NAME: &'static str = "ProviderImage";
        type Type = super::ProviderImage;
        type ParentType = gtk::Box;

        fn new() -> Self {
            Self {
                size: Cell::new(96),
                was_downloaded: Cell::new(false),
                stack: TemplateChild::default(),
                image: TemplateChild::default(),
                spinner: TemplateChild::default(),
                provider: RefCell::default(),
                signal_id: RefCell::default(),
                join_handle: RefCell::default(),
            }
        }

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for ProviderImage {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().setup_widget();
        }
    }
    impl WidgetImpl for ProviderImage {}
    impl BoxImpl for ProviderImage {}

    impl ProviderImage {
        fn set_provider(&self, provider: Option<Provider>) {
            let obj = self.obj();
            if let Some(provider) = provider {
                self.provider.borrow_mut().replace(provider.clone());
                if provider.website().is_some() || provider.image_uri().is_some() {
                    self.stack.set_visible_child_name("loading");
                    self.spinner.start();
                    obj.on_provider_image_changed();
                }
                let signal_id =
                    provider.connect_image_uri_notify(clone!(@weak obj as image => move |_| {
                        image.on_provider_image_changed();
                    }));
                self.signal_id.replace(Some(signal_id));
                return;
            } else if let (Some(signal_id), Some(provider)) =
                (self.signal_id.borrow_mut().take(), obj.provider())
            {
                let _ = self.provider.take();
                provider.disconnect(signal_id);
            }

            self.image.set_from_icon_name(Some("provider-fallback"));
        }
    }
}

glib::wrapper! {
    pub struct ProviderImage(ObjectSubclass<imp::ProviderImage>)
        @extends gtk::Widget, gtk::Box;
}
impl ProviderImage {
    fn on_provider_image_changed(&self) {
        let imp = self.imp();
        let provider = self.provider().unwrap();
        match provider.image_uri() {
            Some(uri) => {
                // Very dirty hack to store that we couldn't find an icon
                // to avoid re-hitting the website every time we have to display it
                if uri == "invalid" {
                    imp.image.set_from_icon_name(Some("provider-fallback"));
                    imp.stack.set_visible_child_name("image");
                    return;
                }
                let small_file = gio::File::for_path(FAVICONS_PATH.join(format!("{uri}_32x32")));
                let large_file = gio::File::for_path(FAVICONS_PATH.join(format!("{uri}_96x96")));
                if !small_file.query_exists(gio::Cancellable::NONE)
                    || !large_file.query_exists(gio::Cancellable::NONE)
                {
                    self.fetch();
                    return;
                }
                if imp.size.get() == 32 {
                    imp.image.set_from_file(small_file.path());
                } else {
                    imp.image.set_from_file(large_file.path());
                }
                imp.was_downloaded.set(true);
                imp.stack.set_visible_child_name("image");
            }
            _ => {
                self.fetch();
            }
        }
    }

    fn fetch(&self) {
        let imp = self.imp();
        let network_monitor = gio::NetworkMonitor::default();
        if (network_monitor.is_network_metered() && !SETTINGS.download_favicons_metered())
            || !SETTINGS.download_favicons()
        {
            imp.image.set_from_icon_name(Some("provider-fallback"));
            imp.stack.set_visible_child_name("image");
            return;
        }
        if let Some(handle) = imp.join_handle.borrow_mut().take() {
            handle.abort();
        }
        if let Some(provider) = self.provider() {
            imp.stack.set_visible_child_name("loading");
            imp.spinner.start();

            if let Some(website) = provider.website() {
                let id = provider.id();
                let name = provider.name();
                let (sender, receiver) = tokio::sync::oneshot::channel();
                let future = async move {
                    match Provider::favicon(website, name, id).await {
                        Ok(cache_name) => {
                            sender.send(Some(cache_name)).unwrap();
                        }
                        Err(err) => {
                            tracing::error!("Failed to load favicon {}", err);
                            sender.send(None).unwrap();
                        }
                    };
                };
                let join_handle = RUNTIME.spawn(future);
                imp.join_handle.borrow_mut().replace(join_handle);

                spawn(clone!(@weak self as this => async move {
                   let imp = this.imp();
                   imp.was_downloaded.set(true);
                    let image_path = match receiver.await {
                        // TODO: handle network failure and other errors differently
                        Ok(None) => {
                            imp.image.set_from_icon_name(Some("provider-fallback"));
                            "invalid".to_string()
                        }
                        Ok(Some(cache_name)) => {
                            if imp.size.get() == 32 {
                                imp.image
                                    .set_from_file(Some(&FAVICONS_PATH.join(format!("{cache_name}_32x32"))));
                            } else {
                                imp.image
                                    .set_from_file(Some(&FAVICONS_PATH.join(format!("{cache_name}_96x96"))));
                            }
                            cache_name
                        }
                        Err(e) => {
                            tracing::error!("Failed to receive data {e}");
                            return;
                        }
                    };
                    if let Some(provider) = this.provider() {
                        let guard = provider.freeze_notify();
                        provider.set_image_uri(image_path);
                        drop(guard);
                    }
                    imp.stack.set_visible_child_name("image");
                    imp.spinner.stop();
                }));
            }
        }
    }

    pub fn reset(&self) {
        self.imp()
            .image
            .set_from_icon_name(Some("provider-fallback"));
        self.fetch();
    }

    pub fn set_from_file(&self, file: &gio::File) {
        let imp = self.imp();

        imp.image.set_from_file(file.path());
        imp.stack.set_visible_child_name("image");
    }

    fn setup_widget(&self) {
        let imp = self.imp();
        self.bind_property("size", &*imp.image, "pixel-size")
            .sync_create()
            .build();
        SETTINGS.connect_download_favicons_changed(clone!(@weak self as image => move |state| {
            if state && !image.imp().was_downloaded.get() {
                image.fetch();
            }
        }));

        SETTINGS.connect_download_favicons_metered_changed(
            clone!(@weak self as image => move |state| {
                let network_monitor = gio::NetworkMonitor::default();
                    if !image.imp().was_downloaded.get() && (network_monitor.is_network_metered() && state) || !network_monitor.is_network_metered() {
                        image.fetch();
                    }
            }),
        );
    }
}
