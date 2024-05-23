#![doc(
    html_logo_url = "https://gitlab.gnome.org/World/Authenticator/-/raw/master/data/icons/com.belmoussaoui.Authenticator.svg?inline=false",
    html_favicon_url = "https://gitlab.gnome.org/World/Authenticator/-/raw/master/data/icons/com.belmoussaoui.Authenticator-symbolic.svg?inline=false"
)]

use gtk::{gio, glib};

mod utils;
use gettextrs::*;
mod application;
mod backup;
mod config;
mod models;
mod schema;
mod widgets;

use application::Application;

fn init_i18n() -> anyhow::Result<()> {
    setlocale(LocaleCategory::LcAll, "");
    bindtextdomain(config::GETTEXT_PACKAGE, config::LOCALEDIR)?;
    textdomain(config::GETTEXT_PACKAGE)?;

    Ok(())
}

fn main() -> glib::ExitCode {
    tracing_subscriber::fmt::init();
    gtk::init().expect("failed to init gtk");
    aperture::init(config::APP_ID);

    if let Err(err) = init_i18n() {
        tracing::error!("Failed to initialize i18n {}", err);
    }

    let res = gio::Resource::load(config::PKGDATADIR.to_owned() + "/authenticator.gresource")
        .expect("Could not load resources");
    gio::resources_register(&res);

    glib::set_application_name(&gettext("Authenticator"));

    Application::run()
}
