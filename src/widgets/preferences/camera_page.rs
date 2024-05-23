use std::{cell::Cell, rc::Rc};

use adw::subclass::{navigation_page::*, prelude::*};
use anyhow::Result;
use gtk::{
    gio,
    glib::{self, clone},
    prelude::*,
};
use tokio::{
    select,
    sync::oneshot,
    time::{sleep, Duration},
};

use crate::{utils::spawn_tokio, widgets::Camera};

mod imp {
    use std::cell::OnceCell;

    use super::*;

    #[derive(Default, gtk::CompositeTemplate, glib::Properties)]
    #[template(resource = "/com/belmoussaoui/Authenticator/preferences_camera_page.ui")]
    #[properties(wrapper_type = super::CameraPage)]
    pub struct CameraPage {
        #[property(get, set, construct_only)]
        pub actions: OnceCell<gio::SimpleActionGroup>,
        #[template_child]
        pub camera: TemplateChild<Camera>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for CameraPage {
        const NAME: &'static str = "CameraPage";
        type Type = super::CameraPage;
        type ParentType = adw::NavigationPage;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.bind_template_instance_callbacks();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for CameraPage {}

    impl WidgetImpl for CameraPage {}
    impl NavigationPageImpl for CameraPage {}
}

glib::wrapper! {
    pub struct CameraPage(ObjectSubclass<imp::CameraPage>)
        @extends gtk::Widget, adw::NavigationPage;
}

#[gtk::template_callbacks]
impl CameraPage {
    pub fn new(actions: &gio::SimpleActionGroup) -> Self {
        glib::Object::builder().property("actions", actions).build()
    }

    pub async fn scan_from_camera(&self) -> Result<String> {
        let imp = self.imp();

        let (tx, rx) = oneshot::channel();

        // This is required because for whatever reason `glib::clone!` wouldn't let it
        // be moved into the closure.
        let tx = Rc::new(Cell::new(Some(tx)));

        // This is to make it safe to access `src` inside of the connected closure to
        // disconnect it after being called.
        let src = Rc::new(Cell::new(None));

        src.set(Some(imp.camera.connect_code_detected(clone!(
            @weak self as camera_page, @strong src, @strong tx
            => move |_, code| {
                match tx.take().unwrap().send(code) {
                    Ok(()) => (),
                    Err(_) => {
                        tracing::error!(concat!(
                            "CameraPage::scan_from_camera failed to send the resulting QR ",
                            "code to the recipient because the recipient already received a ",
                            "QR code or was dropped. This should never occur.",
                        ));
                    }
                }
                camera_page.imp().camera.disconnect(src.take().unwrap());
            }
        ))));

        drop(tx);
        drop(src);

        imp.camera.scan_from_camera().await;

        match rx.await {
            Ok(code) => Ok(code),
            Err(error) => {
                tracing::error!(concat!(
                    "CameraPage::scan_from_camera failed to receive the resulting QR code from ",
                    "the sender because the sender was dropped without sending a QR code. This ",
                    "should never occur."
                ));
                Err(error.into())
            }
        }
    }

    pub async fn scan_from_screenshot(&self) -> Result<String> {
        let imp = self.imp();

        let (tx, rx) = oneshot::channel();

        // This is required because for whatever reason `glib::clone!` wouldn't let it
        // be moved into the closure.
        let tx = Rc::new(Cell::new(Some(tx)));

        // This is to make it safe to access `src` inside of the connected closure to
        // disconnect it after being called.
        let src = Rc::new(Cell::new(None));

        src.set(Some(imp.camera.connect_code_detected(clone!(
            @weak self as camera_page, @strong src, @strong tx
            => move |_, code| {
                match tx.take().unwrap().send(code) {
                    Ok(()) => (),
                    Err(_) => {
                        tracing::error!(concat!(
                            "CameraPage::scan_from_screenshot failed to send the resulting QR ",
                            "code to the recipient because the recipient already received a ",
                            "QR code or was dropped. This should never occur.",
                        ));
                    }
                }
                camera_page.imp().camera.disconnect(src.take().unwrap());
            }
        ))));

        drop(tx);

        select! {
            biased;
            result = rx => result.map_err(|error| {
                tracing::error!(concat!(
                    "CameraPage::scan_from_screenshot failed to receive the resulting QR code ",
                    "from the sender because the sender was dropped without sending a QR ",
                    "code. This should never occur.",
                ));

                error.into()
            }),
            result = async move {
                imp.camera.scan_from_screenshot().await?;

                // Give the GLib event loop a whole 2.5 seconds to dispatch the "code-detected"
                // action before we assume that its not going to be dispatched at all.
                spawn_tokio(async { sleep(Duration::from_millis(2500)).await; }).await;

                // Disconnect the signal handler.
                imp.camera.disconnect(src.take().unwrap());

                anyhow::bail!(concat!(
                    "CameraPage::scan_from_screenshot failed to receive the resulting QR code in ",
                    "a reasonable amount of time."
                ));
            } => result.map_err(From::from),
        }
    }

    #[template_callback]
    fn on_camera_close(&self) {
        self.actions().activate_action("close_page", None);
    }
}
