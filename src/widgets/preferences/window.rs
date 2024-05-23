use adw::prelude::*;
use anyhow::Result;
use gettextrs::gettext;
use gtk::{
    gio,
    glib::{self, clone},
    subclass::prelude::*,
};

use super::{camera_page::CameraPage, password_page::PasswordPage};
use crate::{
    backup::{
        Aegis, AndOTP, Backupable, Bitwarden, FreeOTP, FreeOTPJSON, Google, LegacyAuthenticator,
        Operation, RaivoOTP, Restorable, RestorableItem,
    },
    models::{ProvidersModel, SETTINGS},
    utils::spawn,
    widgets::screenshot,
};

mod imp {
    use std::{
        cell::{Cell, OnceCell, RefCell},
        collections::HashMap,
    };

    use adw::subclass::prelude::*;
    use glib::subclass::Signal;
    use once_cell::sync::Lazy;

    use super::*;

    #[derive(gtk::CompositeTemplate, glib::Properties)]
    #[properties(wrapper_type = super::PreferencesWindow)]
    #[template(resource = "/com/belmoussaoui/Authenticator/preferences.ui")]
    pub struct PreferencesWindow {
        #[property(get, set, construct_only)]
        pub model: OnceCell<ProvidersModel>,
        #[property(get, set, construct)]
        pub has_set_password: Cell<bool>,
        pub actions: gio::SimpleActionGroup,
        pub backup_actions: gio::SimpleActionGroup,
        pub restore_actions: gio::SimpleActionGroup,
        pub camera_page: CameraPage,
        pub password_page: PasswordPage,
        #[template_child]
        pub backup_group: TemplateChild<adw::PreferencesGroup>,
        #[template_child]
        pub restore_group: TemplateChild<adw::PreferencesGroup>,
        #[template_child(id = "auto_lock_switch")]
        pub auto_lock: TemplateChild<adw::SwitchRow>,
        #[template_child(id = "download_favicons_switch")]
        pub download_favicons: TemplateChild<adw::SwitchRow>,
        #[template_child(id = "download_favicons_metered_switch")]
        pub download_favicons_metered: TemplateChild<adw::SwitchRow>,
        #[template_child(id = "lock_timeout_spin_btn")]
        pub lock_timeout: TemplateChild<adw::SpinRow>,
        pub key_entries: RefCell<HashMap<String, adw::PasswordEntryRow>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PreferencesWindow {
        const NAME: &'static str = "PreferencesWindow";
        type Type = super::PreferencesWindow;
        type ParentType = adw::PreferencesDialog;

        fn new() -> Self {
            let actions = gio::SimpleActionGroup::new();

            Self {
                has_set_password: Cell::default(), // Synced from the application
                camera_page: CameraPage::new(&actions),
                password_page: PasswordPage::new(&actions),
                actions,
                model: OnceCell::default(),
                backup_actions: gio::SimpleActionGroup::new(),
                restore_actions: gio::SimpleActionGroup::new(),
                auto_lock: TemplateChild::default(),
                download_favicons: TemplateChild::default(),
                download_favicons_metered: TemplateChild::default(),
                lock_timeout: TemplateChild::default(),
                backup_group: TemplateChild::default(),
                restore_group: TemplateChild::default(),
                key_entries: RefCell::default(),
            }
        }

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for PreferencesWindow {
        fn signals() -> &'static [Signal] {
            static SIGNALS: Lazy<Vec<Signal>> =
                Lazy::new(|| vec![Signal::builder("restore-completed").action().build()]);
            SIGNALS.as_ref()
        }

        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();

            obj.setup_actions();
            obj.setup_widget();
        }
    }
    impl WidgetImpl for PreferencesWindow {}
    impl AdwDialogImpl for PreferencesWindow {}
    impl PreferencesDialogImpl for PreferencesWindow {}
}

glib::wrapper! {
    pub struct PreferencesWindow(ObjectSubclass<imp::PreferencesWindow>)
        @extends gtk::Widget, adw::Dialog, adw::PreferencesDialog;
}

impl PreferencesWindow {
    pub fn new(model: &ProvidersModel) -> Self {
        glib::Object::builder().property("model", model).build()
    }

    pub fn connect_restore_completed<F>(&self, callback: F) -> glib::SignalHandlerId
    where
        F: Fn(&Self) + 'static,
    {
        self.connect_local(
            "restore-completed",
            false,
            clone!(@weak self as win => @default-return None, move |_| {
                callback(&win);
                None
            }),
        )
    }

    fn setup_widget(&self) {
        let imp = self.imp();

        SETTINGS
            .bind_download_favicons(&*imp.download_favicons, "active")
            .build();
        SETTINGS
            .bind_download_favicons_metred(&*imp.download_favicons_metered, "active")
            .build();
        SETTINGS.bind_auto_lock(&*imp.auto_lock, "active").build();
        SETTINGS
            .bind_auto_lock_timeout(&*imp.lock_timeout, "value")
            .build();

        imp.password_page
            .bind_property("has-set-password", self, "has-set-password")
            .sync_create()
            .bidirectional()
            .build();

        // FreeOTP is first in all of these lists, since its the way to backup
        // Authenticator for use with Authenticator. Others are sorted
        // alphabetically.

        self.register_backup::<FreeOTP>(&["text/plain"]);
        self.register_backup::<Aegis>(&["application/json"]);
        self.register_backup::<AndOTP>(&["application/json"]);

        self.register_restore::<FreeOTP>(&["text/plain"]);
        self.register_restore::<FreeOTPJSON>(&["application/json"]);
        self.register_restore::<Aegis>(&["application/json"]);
        self.register_restore::<AndOTP>(&["application/json"]);
        self.register_restore::<Bitwarden>(&["application/json"]);
        self.register_restore::<Google>(&[]);
        self.register_restore::<LegacyAuthenticator>(&["application/json"]);
        self.register_restore::<RaivoOTP>(&["application/zip"]);
    }

    fn register_backup<T: Backupable>(&self, filters: &'static [&str]) {
        let imp = self.imp();
        if T::ENCRYPTABLE {
            let row = adw::ExpanderRow::builder()
                .title(T::title())
                .subtitle(T::subtitle())
                .show_enable_switch(false)
                .enable_expansion(true)
                .use_underline(true)
                .build();
            let key_entry = adw::PasswordEntryRow::builder()
                .title(gettext("Key / Passphrase"))
                .build();
            row.add_row(&key_entry);
            imp.key_entries
                .borrow_mut()
                .insert(format!("backup.{}", T::IDENTIFIER), key_entry);

            let button_row = adw::ActionRow::new();
            let key_button = gtk::Button::builder()
                .valign(gtk::Align::Center)
                .halign(gtk::Align::End)
                .label(gettext("Select File"))
                .action_name(format!("backup.{}", T::IDENTIFIER))
                .build();
            button_row.add_suffix(&key_button);
            row.add_row(&button_row);

            imp.backup_group.add(&row);
        } else {
            let row = adw::ActionRow::builder()
                .title(T::title())
                .subtitle(T::subtitle())
                .activatable(true)
                .use_underline(true)
                .action_name(format!("backup.{}", T::IDENTIFIER))
                .build();

            imp.backup_group.add(&row);
        }

        let action = gio::ActionEntry::builder(T::IDENTIFIER)
            .activate(clone!(@weak self as win => move |_, _,_| {
                spawn(clone!(@weak win => async move {
                    if let Err(err) = win.backup_into_file::<T>(filters).await {
                        tracing::error!("Failed to backup into a file {err}");
                        win.add_toast(adw::Toast::new(&gettext("Failed to create a backup")));
                    }
                }));
            }))
            .build();
        imp.backup_actions.add_action_entries([action]);
    }

    async fn backup_into_file<T: Backupable>(&self, filters: &'static [&str]) -> Result<()> {
        let model = self.model();
        let file = self.select_file(filters, Operation::Backup).await?;
        let key = T::ENCRYPTABLE
            .then(|| self.encryption_key(Operation::Backup, T::IDENTIFIER))
            .flatten();
        let content = T::backup(&model, key.as_deref())?;
        file.replace_contents_future(
            content,
            None,
            false,
            gio::FileCreateFlags::REPLACE_DESTINATION,
        )
        .await
        .map_err(|e| e.1)?;
        Ok(())
    }

    fn register_restore<T: Restorable>(&self, filters: &'static [&str]) {
        let imp = self.imp();
        if T::ENCRYPTABLE {
            let row = adw::ExpanderRow::builder()
                .title(T::title())
                .subtitle(T::subtitle())
                .show_enable_switch(false)
                .enable_expansion(true)
                .use_underline(true)
                .build();
            let key_entry = adw::PasswordEntryRow::builder()
                .title(gettext("Key / Passphrase"))
                .build();
            row.add_row(&key_entry);

            imp.key_entries
                .borrow_mut()
                .insert(format!("restore.{}", T::IDENTIFIER), key_entry);

            let button_row = adw::ActionRow::new();
            let key_button = gtk::Button::builder()
                .valign(gtk::Align::Center)
                .halign(gtk::Align::End)
                .label(gettext("Select File"))
                .action_name(format!("restore.{}", T::IDENTIFIER))
                .build();
            button_row.add_suffix(&key_button);
            row.add_row(&button_row);
            imp.restore_group.add(&row);
        } else if T::SCANNABLE {
            let menu_button = gtk::MenuButton::builder()
                .css_classes(vec!["flat".to_string()])
                .halign(gtk::Align::Fill)
                .valign(gtk::Align::Center)
                .icon_name("qrscanner-symbolic")
                .tooltip_text(gettext("Scan QR Code"))
                .menu_model(&{
                    let menu = gio::Menu::new();

                    menu.append(
                        Some(&gettext("_Camera")),
                        Some(&format!("restore.{}.camera", T::IDENTIFIER)),
                    );
                    menu.append(
                        Some(&gettext("_Screenshot")),
                        Some(&format!("restore.{}.screenshot", T::IDENTIFIER)),
                    );

                    menu.append(
                        Some(&gettext("_QR Code Image")),
                        Some(&format!("restore.{}.file", T::IDENTIFIER)),
                    );

                    menu
                })
                .build();

            let row = adw::ActionRow::builder()
                .title(T::title())
                .subtitle(T::subtitle())
                .activatable(true)
                .activatable_widget(&menu_button)
                .use_underline(true)
                .build();

            row.add_suffix(&menu_button);

            imp.restore_group.add(&row);
        } else {
            let row = adw::ActionRow::builder()
                .title(T::title())
                .subtitle(T::subtitle())
                .activatable(true)
                .use_underline(true)
                .action_name(format!("restore.{}", T::IDENTIFIER))
                .build();

            imp.restore_group.add(&row);
        }
        if T::SCANNABLE {
            let camera_action = gio::ActionEntry::builder(&format!("{}.camera", T::IDENTIFIER))
                .activate(clone!(@weak self as win=> move |_, _, _| {
                    win.imp().actions.activate_action("show_camera_page", None);
                    spawn(clone!(@weak win => async move {
                        if let Err(err) = win.restore_from_camera::<T, T::Item>().await {
                            tracing::error!("Failed to restore from camera {err}");
                            win.add_toast(adw::Toast::new(&gettext("Failed to restore from camera")));
                        }
                    }));
                }))
                .build();
            let screenshot_action =
                gio::ActionEntry::builder(&format!("{}.screenshot", T::IDENTIFIER))
                    .activate(clone!(@weak self as win => move |_, _, _| {
                        spawn(clone!(@weak win => async move {
                            if let Err(err) = win.restore_from_screenshot::<T, T::Item>().await {
                                tracing::error!("Failed to restore from a screenshot {err}");
                                win.add_toast(adw::Toast::new(&gettext("Failed to restore from a screenshot")));
                            }
                        }));
                    }))
                    .build();
            let file_action = gio::ActionEntry::builder(&format!("{}.file", T::IDENTIFIER))
                .activate(clone!(@weak self as win => move |_, _, _| {
                    spawn(clone!(@weak win => async move {
                        if let Err(err) = win.restore_from_image::<T, T::Item>().await {
                            tracing::error!("Failed to restore from an image {err}");
                            win.add_toast(adw::Toast::new(&gettext("Failed to restore from an image")));
                        }
                    }));
                }))
                .build();
            imp.restore_actions
                .add_action_entries([camera_action, file_action, screenshot_action]);
        } else {
            let action = gio::ActionEntry::builder(T::IDENTIFIER)
                .activate(clone!(@weak self as win => move |_, _, _| {
                    spawn(clone!(@weak win => async move {
                        if let Err(err) = win.restore_from_file::<T, T::Item>(filters).await {
                            tracing::error!("Failed to restore from a file {err}");
                            win.add_toast(adw::Toast::new(&gettext("Failed to restore from a file")));
                        }
                    }));
                }))
                .build();

            imp.restore_actions.add_action_entries([action]);
        };
    }
    async fn restore_from_file<T: Restorable<Item = Q>, Q: RestorableItem>(
        &self,
        filters: &'static [&str],
    ) -> Result<()> {
        let file = self.select_file(filters, Operation::Restore).await?;
        let key = T::ENCRYPTABLE
            .then(|| self.encryption_key(Operation::Restore, T::IDENTIFIER))
            .flatten();
        let content = file.load_contents_future().await?;
        let items = T::restore_from_data(&content.0, key.as_deref())?;
        self.restore_items::<T, T::Item>(items);
        Ok(())
    }

    async fn restore_from_camera<T: Restorable<Item = Q>, Q: RestorableItem>(&self) -> Result<()> {
        let code = self.imp().camera_page.scan_from_camera().await?;
        let items = T::restore_from_data(code.as_bytes(), None)?;
        self.restore_items::<T, T::Item>(items);
        self.imp().actions.activate_action("close_page", None);
        Ok(())
    }

    async fn restore_from_screenshot<T: Restorable<Item = Q>, Q: RestorableItem>(
        &self,
    ) -> Result<()> {
        let code = self.imp().camera_page.scan_from_screenshot().await?;
        let items = T::restore_from_data(code.as_bytes(), None)?;
        self.restore_items::<T, T::Item>(items);
        Ok(())
    }

    async fn restore_from_image<T: Restorable<Item = Q>, Q: RestorableItem>(&self) -> Result<()> {
        let window = self.root().and_downcast::<gtk::Window>().unwrap();

        let images_filter = gtk::FileFilter::new();
        images_filter.set_name(Some(&gettext("Image")));
        images_filter.add_pixbuf_formats();
        let model = gio::ListStore::new::<gtk::FileFilter>();
        model.append(&images_filter);

        let dialog = gtk::FileDialog::builder()
            .modal(true)
            .filters(&model)
            .title(gettext("Select QR Code"))
            .build();
        let file = dialog.open_future(Some(&window)).await?;
        let (data, _) = file.load_contents_future().await?;
        let code = screenshot::scan(&data)?;
        let items = T::restore_from_data(code.as_bytes(), None)?;
        self.restore_items::<T, T::Item>(items);
        Ok(())
    }

    fn encryption_key(&self, mode: Operation, identifier: &str) -> Option<glib::GString> {
        let identifier = match mode {
            Operation::Backup => format!("backup.{identifier}",),
            Operation::Restore => format!("restore.{identifier}"),
        };
        self.imp()
            .key_entries
            .borrow()
            .get(&identifier)
            .map(|entry| entry.text())
    }

    fn restore_items<T: Restorable<Item = Q>, Q: RestorableItem>(&self, items: Vec<Q>) {
        let model = self.model();
        items
            .iter()
            .map(move |item| item.restore(&model))
            .for_each(|item| {
                if let Err(err) = item {
                    tracing::warn!("Failed to restore item {}", err);
                }
            });
        self.emit_by_name::<()>("restore-completed", &[]);
        self.close();
    }

    async fn select_file(
        &self,
        filters: &'static [&str],
        operation: Operation,
    ) -> Result<gio::File, glib::Error> {
        let filters_model = gio::ListStore::new::<gtk::FileFilter>();
        let window = self.root().and_downcast::<gtk::Window>().unwrap();
        filters.iter().for_each(|f| {
            let filter = gtk::FileFilter::new();
            filter.add_mime_type(f);
            filter.set_name(Some(f));
            filters_model.append(&filter);
        });

        match operation {
            Operation::Backup => {
                let dialog = gtk::FileDialog::builder()
                    .modal(true)
                    .filters(&filters_model)
                    .title(gettext("Backup"))
                    .build();
                dialog.save_future(Some(&window)).await
            }
            Operation::Restore => {
                let dialog = gtk::FileDialog::builder()
                    .modal(true)
                    .filters(&filters_model)
                    .title(gettext("Restore"))
                    .build();
                dialog.open_future(Some(&window)).await
            }
        }
    }

    fn setup_actions(&self) {
        let imp = self.imp();

        imp.camera_page
            .connect_map(clone!(@weak self as win => move |_| {
                win.set_search_enabled(false);
            }));

        imp.camera_page
            .connect_unmap(clone!(@weak self as win => move |_| {
                win.set_search_enabled(true);
            }));

        imp.password_page
            .connect_map(clone!(@weak self as win => move |_| {
                win.set_search_enabled(false);
            }));

        imp.password_page
            .connect_unmap(clone!(@weak self as win => move |_| {
                win.set_search_enabled(true);
            }));

        let show_camera_page = gio::ActionEntry::builder("show_camera_page")
            .activate(clone!(@weak self as win => move |_, _, _| {
                win.push_subpage(&win.imp().camera_page);
            }))
            .build();

        let show_password_page = gio::ActionEntry::builder("show_password_page")
            .activate(clone!(@weak self as win => move |_, _, _| {
                win.push_subpage(&win.imp().password_page);
            }))
            .build();

        let close_page = gio::ActionEntry::builder("close_page")
            .activate(clone!(@weak self as win => move |_, _, _| {
                win.pop_subpage();
            }))
            .build();

        imp.actions
            .add_action_entries([show_camera_page, show_password_page, close_page]);

        self.insert_action_group("preferences", Some(&imp.actions));
        self.insert_action_group("backup", Some(&imp.backup_actions));
        self.insert_action_group("restore", Some(&imp.restore_actions));
    }
}
