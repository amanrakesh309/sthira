pub mod migrations;
pub mod objects_repo;
pub mod providers_repo;
pub mod scheduled_topics_repo;
pub mod search_repo;

use crate::CoreError;
use rusqlite::Connection;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, Once};

static INIT_VEC: Once = Once::new();

#[allow(clippy::missing_transmute_annotations)]
pub fn ensure_sqlite_vec_registered() {
    INIT_VEC.call_once(|| {
        unsafe {
            rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute(
                sqlite_vec::sqlite3_vec_init as *const (),
            )));
        }
    });
}

#[derive(Debug)]
pub struct SqliteManager {
    path: PathBuf,
    read_only: bool,
}

impl r2d2::ManageConnection for SqliteManager {
    type Connection = Connection;
    type Error = rusqlite::Error;

    fn connect(&self) -> Result<Connection, rusqlite::Error> {
        ensure_sqlite_vec_registered();
        let conn = if self.read_only {
            let conn = Connection::open_with_flags(
                &self.path,
                rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
            )?;
            conn.execute_batch("PRAGMA query_only = ON;")?;
            conn
        } else {
            Connection::open(&self.path)?
        };

        conn.execute_batch(
            "PRAGMA foreign_keys = ON;
             PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA busy_timeout = 5000;",
        )?;
        let _: String = conn.query_row("SELECT vec_version();", [], |r| r.get(0))?;
        Ok(conn)
    }

    fn is_valid(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        conn.execute_batch("SELECT 1;")
    }

    fn has_broken(&self, _conn: &mut Connection) -> bool {
        false
    }
}

#[derive(Clone)]
pub struct Db {
    path: PathBuf,
    writer: Arc<Mutex<Connection>>,
    reader_pool: r2d2::Pool<SqliteManager>,
}

impl Db {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, CoreError> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        ensure_sqlite_vec_registered();
        let mut writer = Connection::open(&path)?;
        writer.execute_batch(
            "PRAGMA foreign_keys = ON;
             PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA busy_timeout = 5000;",
        )?;
        let _: String = writer.query_row("SELECT vec_version();", [], |r| r.get(0))?;

        migrations::run_migrations(&mut writer)?;

        let manager = SqliteManager {
            path: path.clone(),
            read_only: true,
        };
        let reader_pool = r2d2::Pool::builder()
            .max_size(16)
            .build(manager)
            .map_err(|e| CoreError::InvariantViolation(format!("Failed to build reader pool: {e}")))?;

        Ok(Self {
            path,
            writer: Arc::new(Mutex::new(writer)),
            reader_pool,
        })
    }

    pub fn writer(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.writer.lock().unwrap()
    }

    pub fn reader(&self) -> Result<r2d2::PooledConnection<SqliteManager>, CoreError> {
        self.reader_pool
            .get()
            .map_err(|e| CoreError::InvariantViolation(format!("Reader pool checkout failed: {e}")))
    }

    pub fn open_read_only(&self) -> Result<Connection, CoreError> {
        ensure_sqlite_vec_registered();
        let conn = Connection::open_with_flags(
            &self.path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        conn.execute_batch(
            "PRAGMA query_only = ON;
             PRAGMA foreign_keys = ON;
             PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA busy_timeout = 5000;",
        )?;
        let _: String = conn.query_row("SELECT vec_version();", [], |r| r.get(0))?;
        Ok(conn)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}
