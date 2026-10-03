use parking_lot::ReentrantMutex;
use rusqlite::{Connection, Result};
use std::cell::RefCell;
use std::path::Path;
use std::sync::Arc;

use super::migrations::{run_migrations, SCHEMA_VERSION};

#[derive(Clone)]
pub struct Database {
    conn: Arc<ReentrantMutex<RefCell<Connection>>>,
}

impl Database {
    pub fn open_read_only(path: &Path) -> Result<Self> {
        let conn = Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        conn.busy_timeout(std::time::Duration::from_secs(2))?;
        conn.pragma_update(None, "query_only", true)?;
        conn.pragma_update(None, "trusted_schema", false)?;
        let version: i64 =
            conn.query_row("SELECT MAX(version) FROM _schema_migrations", [], |r| {
                r.get(0)
            })?;
        if version != SCHEMA_VERSION {
            return Err(rusqlite::Error::InvalidParameterName(
                "Start Harnesscope to migrate this database before using MCP".into(),
            ));
        }
        Ok(Self {
            conn: Arc::new(ReentrantMutex::new(RefCell::new(conn))),
        })
    }
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        if path.is_file() {
            let old =
                Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
            let version: Option<i64> = old
                .query_row("SELECT MAX(version) FROM _schema_migrations", [], |r| {
                    r.get(0)
                })
                .unwrap_or(None);
            drop(old);
            if version.is_some_and(|v| v < SCHEMA_VERSION) {
                let mut backup = path.as_os_str().to_owned();
                backup.push(format!(
                    ".before-v{SCHEMA_VERSION}-{}.db",
                    uuid::Uuid::new_v4()
                ));
                super::backup::create(path, Path::new(&backup)).map_err(|_| {
                    rusqlite::Error::InvalidParameterName(
                        "Pre-migration backup failed; database left unchanged".into(),
                    )
                })?;
            }
        }
        let mut conn = Connection::open(path)?;
        Self::configure_and_migrate(&mut conn)?;
        Ok(Self {
            conn: Arc::new(ReentrantMutex::new(RefCell::new(conn))),
        })
    }

    pub fn open_in_memory() -> Result<Self> {
        let mut conn = Connection::open_in_memory()?;
        Self::configure_and_migrate(&mut conn)?;
        Ok(Self {
            conn: Arc::new(ReentrantMutex::new(RefCell::new(conn))),
        })
    }

    fn configure_and_migrate(conn: &mut Connection) -> Result<()> {
        conn.pragma_update(None, "busy_timeout", 5000)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;

        run_migrations(conn)?;
        Ok(())
    }

    pub fn with_conn<F, R>(&self, f: F) -> Result<R>
    where
        F: FnOnce(&mut Connection) -> Result<R>,
    {
        let guard = self.conn.lock();
        let result = f(&mut guard.borrow_mut());
        result
    }

    /// Hold the connection lock across correlation reads and writes. Individual
    /// repository calls re-enter the lock, borrowing the connection only briefly.
    pub fn transaction<T>(&self, f: impl FnOnce() -> Result<T>) -> Result<T> {
        let guard = self.conn.lock();
        if !guard.borrow().is_autocommit() {
            guard
                .borrow_mut()
                .execute_batch("SAVEPOINT evidence_nested")?;
            return match f() {
                Ok(value) => {
                    guard
                        .borrow_mut()
                        .execute_batch("RELEASE evidence_nested")?;
                    Ok(value)
                }
                Err(error) => {
                    let _ = guard
                        .borrow_mut()
                        .execute_batch("ROLLBACK TO evidence_nested; RELEASE evidence_nested");
                    Err(error)
                }
            };
        }
        guard.borrow_mut().execute_batch("BEGIN IMMEDIATE")?;
        struct Rollback<'a>(&'a RefCell<Connection>);
        impl Drop for Rollback<'_> {
            fn drop(&mut self) {
                let _ = self.0.borrow_mut().execute_batch("ROLLBACK");
            }
        }
        let rollback = Rollback(&guard);
        let value = f()?;
        guard.borrow_mut().execute_batch("COMMIT")?;
        std::mem::forget(rollback);
        Ok(value)
    }

    pub fn read_snapshot<T>(&self, f: impl FnOnce() -> Result<T>) -> Result<T> {
        let guard = self.conn.lock();
        guard.borrow_mut().execute_batch("BEGIN DEFERRED")?;
        struct Rollback<'a>(&'a RefCell<Connection>);
        impl Drop for Rollback<'_> {
            fn drop(&mut self) {
                let _ = self.0.borrow_mut().execute_batch("ROLLBACK");
            }
        }
        let rollback = Rollback(&guard);
        let value = f()?;
        guard.borrow_mut().execute_batch("COMMIT")?;
        std::mem::forget(rollback);
        Ok(value)
    }
}
