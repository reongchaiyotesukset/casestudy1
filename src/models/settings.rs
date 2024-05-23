use std::ops::Deref;

use gtk::{
    gio,
    glib::{self, thread_guard::ThreadGuard},
    prelude::*,
};

use crate::config;

pub struct Settings(ThreadGuard<gio::Settings>);

impl Settings {
    const KEY_KEYRINGS_MIGRATED: &'static str = "keyrings-migrated";
    const KEY_AUTO_LOCK: &'static str = "auto-lock";
    const KEY_AUTO_LOCK_TIMEOUT: &'static str = "auto-lock-timeout";
    const KEY_WINDOW_WIDTH: &'static str = "window-width";
    const KEY_WINDOW_HEIGHT: &'static str = "window-height";
    const KEY_IS_MAXIMIZED: &'static str = "is-maximized";
    const KEY_DOWNLOAD_FAVICONS: &'static str = "download-favicons";
    const KEY_DOWNLOAD_FAVICONS_METRED: &'static str = "download-favicons-metered";

    pub fn set_keyrings_migrated(&self, keyrings_migrated: bool) -> Result<(), glib::BoolError> {
        self.set_boolean(Self::KEY_KEYRINGS_MIGRATED, keyrings_migrated)
    }
    pub fn keyrings_migrated(&self) -> bool {
        self.boolean(Self::KEY_KEYRINGS_MIGRATED)
    }

    pub fn auto_lock(&self) -> bool {
        self.boolean(Self::KEY_AUTO_LOCK)
    }

    pub fn connect_auto_lock_changed<F>(&self, callback: F) -> glib::SignalHandlerId
    where
        F: Fn(bool) + 'static,
    {
        self.connect_changed(Some(Self::KEY_AUTO_LOCK), move |settings, _key| {
            callback(settings.boolean(Self::KEY_AUTO_LOCK))
        })
    }

    pub fn bind_auto_lock<'a>(
        &'a self,
        target: &'a impl IsA<glib::Object>,
        target_property: &'a str,
    ) -> gio::BindingBuilder<'a> {
        self.bind(Self::KEY_AUTO_LOCK, target, target_property)
    }

    pub fn auto_lock_timeout(&self) -> u32 {
        self.uint(Self::KEY_AUTO_LOCK_TIMEOUT)
    }

    pub fn connect_auto_lock_timeout_changed<F>(&self, callback: F) -> glib::SignalHandlerId
    where
        F: Fn(u32) + 'static,
    {
        self.connect_changed(Some(Self::KEY_AUTO_LOCK_TIMEOUT), move |settings, _key| {
            callback(settings.uint(Self::KEY_AUTO_LOCK_TIMEOUT))
        })
    }

    pub fn bind_auto_lock_timeout<'a>(
        &'a self,
        target: &'a impl IsA<glib::Object>,
        target_property: &'a str,
    ) -> gio::BindingBuilder<'a> {
        self.bind(Self::KEY_AUTO_LOCK_TIMEOUT, target, target_property)
    }

    pub fn set_window_height(&self, window_height: i32) -> Result<(), glib::BoolError> {
        self.set_int(Self::KEY_WINDOW_HEIGHT, window_height)
    }

    pub fn window_height(&self) -> i32 {
        self.int(Self::KEY_WINDOW_HEIGHT)
    }

    pub fn set_window_width(&self, window_width: i32) -> Result<(), glib::BoolError> {
        self.set_int(Self::KEY_WINDOW_WIDTH, window_width)
    }
    pub fn window_width(&self) -> i32 {
        self.int(Self::KEY_WINDOW_WIDTH)
    }

    pub fn is_maximized(&self) -> bool {
        self.boolean(Self::KEY_IS_MAXIMIZED)
    }
    pub fn set_is_maximized(&self, is_maximized: bool) -> Result<(), glib::BoolError> {
        self.set_boolean(Self::KEY_IS_MAXIMIZED, is_maximized)
    }

    pub fn download_favicons(&self) -> bool {
        self.boolean(Self::KEY_DOWNLOAD_FAVICONS)
    }

    pub fn bind_download_favicons<'a>(
        &'a self,
        target: &'a impl IsA<glib::Object>,
        target_property: &'a str,
    ) -> gio::BindingBuilder<'a> {
        self.bind(Self::KEY_DOWNLOAD_FAVICONS, target, target_property)
    }

    pub fn connect_download_favicons_changed<F>(&self, callback: F) -> glib::SignalHandlerId
    where
        F: Fn(bool) + 'static,
    {
        self.connect_changed(Some(Self::KEY_DOWNLOAD_FAVICONS), move |settings, _key| {
            callback(settings.boolean(Self::KEY_DOWNLOAD_FAVICONS))
        })
    }

    pub fn download_favicons_metered(&self) -> bool {
        self.boolean(Self::KEY_DOWNLOAD_FAVICONS_METRED)
    }

    pub fn bind_download_favicons_metred<'a>(
        &'a self,
        target: &'a impl IsA<glib::Object>,
        target_property: &'a str,
    ) -> gio::BindingBuilder<'a> {
        self.bind(Self::KEY_DOWNLOAD_FAVICONS_METRED, target, target_property)
    }

    pub fn connect_download_favicons_metered_changed<F>(&self, callback: F) -> glib::SignalHandlerId
    where
        F: Fn(bool) + 'static,
    {
        self.connect_changed(
            Some(Self::KEY_DOWNLOAD_FAVICONS_METRED),
            move |settings, _key| callback(settings.boolean(Self::KEY_DOWNLOAD_FAVICONS_METRED)),
        )
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self(ThreadGuard::new(gio::Settings::new(config::APP_ID)))
    }
}

impl Deref for Settings {
    type Target = gio::Settings;

    fn deref(&self) -> &Self::Target {
        self.0.get_ref()
    }
}

unsafe impl Send for Settings {}
unsafe impl Sync for Settings {}
