use std::{cell::OnceCell, os::fd::OwnedFd, sync::Once};

use adw::subclass::prelude::*;
use anyhow::Result;
use ashpd::desktop::screenshot::ScreenshotRequest;
use gtk::{
    gio,
    glib::{self, clone},
    prelude::*,
};
use image::GenericImageView;
use once_cell::sync::Lazy;

use super::CameraRow;
use crate::utils::spawn_tokio;

pub mod screenshot {
    use super::*;

    pub fn scan(data: &[u8]) -> Result<String> {
        // remove the file after reading the data
        let img = image::load_from_memory(data)?;

        let (width, height) = img.dimensions();
        let img_data: Vec<u8> = img.to_luma8().to_vec();

        let mut scanner = zbar_rust::ZBarImageScanner::new();

        let results = scanner
            .scan_y800(&img_data, width, height)
            .map_err(|e| anyhow::format_err!(e))?;

        if let Some(result) = results.first() {
            let content = String::from_utf8(result.data.clone())?;
            return Ok(content);
        }
        anyhow::bail!("Invalid QR code")
    }

    pub async fn capture(window: Option<gtk::Window>) -> Result<gio::File> {
        let identifier = if let Some(ref window) = window {
            ashpd::WindowIdentifier::from_native(window).await
        } else {
            ashpd::WindowIdentifier::default()
        };
        let uri = spawn_tokio(async {
            ScreenshotRequest::default()
                .identifier(identifier)
                .modal(true)
                .interactive(true)
                .send()
                .await?
                .response()
        })
        .await?;

        Ok(gio::File::for_uri(uri.uri().as_str()))
    }
}

mod imp {
    use glib::subclass::{InitializingObject, Signal};

    use super::*;

    #[derive(gtk::CompositeTemplate, Default)]
    #[template(resource = "/com/belmoussaoui/Authenticator/camera.ui")]
    pub struct Camera {
        #[template_child]
        pub stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub viewfinder: TemplateChild<aperture::Viewfinder>,
        #[template_child]
        pub spinner: TemplateChild<gtk::Spinner>,
        #[template_child]
        pub screenshot: TemplateChild<gtk::Button>,
        #[template_child]
        pub camera_selection_button: TemplateChild<gtk::MenuButton>,
        #[template_child]
        pub toolbar_view: TemplateChild<adw::ToolbarView>,
        pub selection: gtk::SingleSelection,
        pub provider: OnceCell<aperture::DeviceProvider>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Camera {
        const NAME: &'static str = "Camera";
        type Type = super::Camera;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            klass.set_css_name("camera");
            klass.bind_template();
            klass.bind_template_instance_callbacks();
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for Camera {
        fn signals() -> &'static [Signal] {
            static SIGNALS: Lazy<Vec<Signal>> = Lazy::new(|| {
                vec![
                    Signal::builder("close").action().build(),
                    Signal::builder("code-detected")
                        .param_types([String::static_type()])
                        .run_first()
                        .build(),
                ]
            });
            SIGNALS.as_ref()
        }

        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();

            let provider = aperture::DeviceProvider::instance();
            self.provider.set(provider.clone()).unwrap();

            self.viewfinder
                .connect_state_notify(glib::clone!(@weak obj => move |_| {
                    obj.update_viewfinder_state();
                }));
            obj.update_viewfinder_state();

            self.viewfinder.connect_code_detected(
                glib::clone!(@weak obj => move|_, code_type, code| {
                    if matches!(code_type, aperture::CodeType::Qr) {
                        obj.emit_by_name::<()>("code-detected", &[&code]);
                    }
                }),
            );

            let popover = gtk::Popover::new();
            popover.add_css_class("menu");

            self.selection.set_model(Some(provider));
            let factory = gtk::SignalListItemFactory::new();
            factory.connect_setup(|_, item| {
                let camera_row = CameraRow::default();

                item.downcast_ref::<gtk::ListItem>()
                    .unwrap()
                    .set_child(Some(&camera_row));
            });
            let selection = &self.selection;
            factory.connect_bind(glib::clone!(@weak selection => move |_, item| {
                let item = item.downcast_ref::<gtk::ListItem>().unwrap();
                let child = item.child().unwrap();
                let row = child.downcast_ref::<CameraRow>().unwrap();

                let item = item.item().and_downcast::<aperture::Camera>().unwrap();
                row.set_label(&item.display_name());

                selection.connect_selected_item_notify(glib::clone!(@weak row, @weak item => move |selection| {
                    if let Some(selected_item) = selection.selected_item() {
                        row.set_selected(selected_item == item);
                    } else {
                        row.set_selected(false);
                    }
                }));
            }));
            let list_view = gtk::ListView::new(Some(self.selection.clone()), Some(factory));
            popover.set_child(Some(&list_view));

            self.selection.connect_selected_item_notify(
                glib::clone!(@weak obj, @weak popover => move |selection| {
                    if let Some(selected_item) = selection.selected_item() {
                        let camera = selected_item.downcast_ref::<aperture::Camera>();
                        obj.imp().viewfinder.set_camera(camera);
                    }
                    popover.popdown();
                }),
            );

            self.camera_selection_button.set_popover(Some(&popover));
        }
    }

    impl WidgetImpl for Camera {}
    impl BinImpl for Camera {}
}

glib::wrapper! {
    pub struct Camera(ObjectSubclass<imp::Camera>)
        @extends gtk::Widget, adw::Bin;
}

#[gtk::template_callbacks]
impl Camera {
    pub fn connect_close<F>(&self, callback: F) -> glib::SignalHandlerId
    where
        F: Fn(&Self) + 'static,
    {
        self.connect_local(
            "close",
            false,
            clone!(@weak self as camera => @default-return None, move |_| {
                callback(&camera);
                None
            }),
        )
    }

    pub fn connect_code_detected<F>(&self, callback: F) -> glib::SignalHandlerId
    where
        F: Fn(&Self, String) + 'static,
    {
        self.connect_local(
            "code-detected",
            false,
            clone!(@weak self as camera => @default-return None, move |args| {
                let code = args[1].get::<String>().unwrap();
                callback(&camera, code);
                None
            }),
        )
    }

    pub async fn scan_from_camera(&self) {
        static INIT: Once = Once::new();
        if INIT.is_completed() {
            return;
        }

        let provider = self.imp().provider.get().unwrap();
        match spawn_tokio(stream()).await {
            Ok(fd) => {
                if let Err(err) = provider.set_fd(fd) {
                    tracing::error!("Could not use the camera portal: {err}");
                } else if let Err(err) = provider.start_with_default(|camera| {
                    matches!(camera.location(), aperture::CameraLocation::Back)
                }) {
                    tracing::error!("Could not start the device provider: {err}");
                } else {
                    tracing::debug!("Device provider started");
                    INIT.call_once(|| ());
                };
            }
            Err(err) => tracing::error!("Failed to start the camera portal: {err}"),
        }
    }

    pub async fn scan_from_screenshot(&self) -> anyhow::Result<()> {
        let screenshot_file = screenshot::capture(
            self.root()
                .map(|root| root.downcast::<gtk::Window>().unwrap()),
        )
        .await?;
        let (data, _) = screenshot_file.load_contents_future().await?;
        if let Ok(code) = screenshot::scan(&data) {
            self.emit_by_name::<()>("code-detected", &[&code]);
        }
        if let Err(err) = screenshot_file.trash_future(glib::Priority::HIGH).await {
            tracing::error!("Failed to remove scanned screenshot {}", err);
        }
        Ok(())
    }

    fn update_viewfinder_state(&self) {
        let imp = self.imp();
        let state = imp.viewfinder.state();
        match state {
            aperture::ViewfinderState::Loading => {
                imp.stack.set_visible_child_name("loading");
            }
            aperture::ViewfinderState::Error => {
                imp.stack.set_visible_child_name("not-found");
            }
            aperture::ViewfinderState::NoCameras => {
                imp.stack.set_visible_child_name("not-found");
            }
            aperture::ViewfinderState::Ready => {
                imp.stack.set_visible_child_name("stream");
            }
        }
        tracing::info!("The camera state changed: {state:?}");

        let is_ready = matches!(state, aperture::ViewfinderState::Ready);
        imp.toolbar_view.set_extend_content_to_top_edge(is_ready);
        if is_ready {
            imp.toolbar_view.add_css_class("extended");
        } else {
            imp.toolbar_view.remove_css_class("extended");
        }

        if matches!(state, aperture::ViewfinderState::Loading) {
            imp.spinner.start();
        } else {
            imp.spinner.stop();
        }
    }

    #[template_callback]
    async fn on_screenshot_clicked(&self, _btn: gtk::Button) {
        if let Err(err) = self.scan_from_screenshot().await {
            tracing::error!("Failed to scan from screenshot {err}");
        }
    }
}

impl Default for Camera {
    fn default() -> Self {
        glib::Object::new()
    }
}

async fn stream() -> ashpd::Result<OwnedFd> {
    let proxy = ashpd::desktop::camera::Camera::new().await?;
    proxy.request_access().await?;

    proxy.open_pipe_wire_remote().await
}
