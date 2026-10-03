use parking_lot::ReentrantMutex;
use rusqlite::{Connection, Result};
use std::cell::RefCell;
use std::path::Path;
use std::sync::Arc;

use super::migrations::run_migrations;

#[derive(Clone)]
pub struct Database {
    conn: Arc<ReentrantMutex<RefCell<Connection>>>,
}

impl Database {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
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
}
