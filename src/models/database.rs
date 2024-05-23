use std::{fs, fs::File, path::PathBuf};

use anyhow::Result;
use diesel::{prelude::*, r2d2, r2d2::ConnectionManager};
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use once_cell::sync::Lazy;

type Pool = r2d2::Pool<ConnectionManager<SqliteConnection>>;

static DB_PATH: Lazy<PathBuf> = Lazy::new(|| gtk::glib::user_data_dir().join("authenticator"));
static POOL: Lazy<Pool> = Lazy::new(|| init_pool().expect("Failed to create a pool"));

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations/");

pub(crate) fn connection() -> Pool {
    POOL.clone()
}

fn init_pool() -> Result<Pool> {
    fs::create_dir_all(&*DB_PATH)?;
    let db_path = DB_PATH.join("authenticator.db");
    if !db_path.exists() {
        File::create(&db_path)?;
    }
    let manager = ConnectionManager::<SqliteConnection>::new(db_path.to_str().unwrap());
    let pool = r2d2::Pool::builder().build(manager)?;

    {
        let mut db = pool.get()?;
        tracing::info!("Running DB Migrations...");
        db.run_pending_migrations(MIGRATIONS)
            .expect("Failed to run migrations");
    }
    tracing::info!("Database pool initialized.");
    Ok(pool)
}
