//! Arrival-order cursors remain stable when delayed events have older timestamps.
use super::Repository;
use crate::domain::models::Event;
use rusqlite::{params, Result};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy)]
pub enum EventOwner {
    Execution,
    Session,
}
impl EventOwner {
    fn column(self) -> &'static str {
        match self {
            Self::Execution => "execution_id",
            Self::Session => "session_id",
        }
    }
}

#[derive(Default, Deserialize)]
pub struct EventQuery {
    pub before_id: Option<i64>,
    pub limit: Option<u32>,
    pub search: Option<String>,
}
#[derive(Serialize)]
pub struct EventPage {
    pub items: Vec<Event>,
    pub next_cursor: Option<i64>,
    pub total: u64,
}
impl Repository {
    pub fn count_events(&self, owner: EventOwner, id: &str) -> Result<u64> {
        self.db.with_conn(|c| {
            c.query_row(
                &format!("SELECT COUNT(*) FROM events WHERE {}=?1", owner.column()),
                [id],
                |r| r.get(0),
            )
        })
    }

    pub fn event_page(&self, owner: EventOwner, id: &str, query: &EventQuery) -> Result<EventPage> {
        self.db.with_conn(|c| {
            let tx = c.transaction()?;
            let limit = query.limit.unwrap_or(50).clamp(1, 100) as usize;
            let search = query.search.as_deref().unwrap_or("").trim().to_lowercase();
            let filter = if search.is_empty() { String::new() } else {
                " AND (instr(lower(event_type),?2)>0 OR instr(lower(source),?2)>0)".into()
            };
            // Use a separate no-search query so COUNT(*) can use the covering owner index.
            let count_sql = format!("SELECT COUNT(*) FROM events WHERE {}=?1{filter}", owner.column());
            let total = if search.is_empty() {
                tx.query_row(&count_sql, [id], |r| r.get(0))?
            } else {
                tx.query_row(&count_sql, params![id,search], |r| r.get(0))?
            };
            let sql = format!("SELECT id,event_id,timestamp,runtime_id,session_id,execution_id,agent_instance_id,event_type,source,payload_json
                FROM events WHERE {}=?1 AND id<?3{filter} ORDER BY id DESC LIMIT ?4", owner.column());
            let mut items = {
                let mut stmt = tx.prepare(&sql)?;
                let rows = stmt.query_map(params![id,search,query.before_id.unwrap_or(i64::MAX),limit+1], |r| Ok(Event {
                    id:Some(r.get(0)?), event_id:r.get(1)?, timestamp:r.get(2)?, runtime_id:r.get(3)?,
                    session_id:r.get(4)?, execution_id:r.get(5)?, agent_instance_id:r.get(6)?,
                    event_type:r.get(7)?, source:r.get(8)?, payload_json:r.get(9)?,
                }))?;
                rows.collect::<Result<Vec<_>>>()?
            };
            let next_cursor = if items.len()>limit {
                items.truncate(limit);
                items.last().and_then(|e| e.id)
            } else { None };
            tx.commit()?;
            Ok(EventPage { items, next_cursor, total })
        })
    }
}
