use rusqlite::{Connection, Result};
use std::path::Path;
use std::sync::{Arc, Mutex};

use super::migrations::run_migrations;

#[derive(Clone)]
pub struct Database {
    conn: Arc<Mutex<Connection>>,
}

impl Database {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let mut conn = Connection::open(path)?;
        Self::configure_and_migrate(&mut conn)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn open_in_memory() -> Result<Self> {
        let mut conn = Connection::open_in_memory()?;
        Self::configure_and_migrate(&mut conn)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    fn configure_and_migrate(conn: &mut Connection) -> Result<()> {
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.pragma_update(None, "busy_timeout", 5000)?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;

        run_migrations(conn)?;
        Ok(())
    }

    pub fn with_conn<F, R>(&self, f: F) -> Result<R>
    where
        F: FnOnce(&mut Connection) -> Result<R>,
    {
        let mut guard = self.conn.lock().unwrap();
        f(&mut guard)
    }
}
