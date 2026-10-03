use crate::{domain::events::IngestEvent, storage::Repository};
use rusqlite::{params, OptionalExtension, Result};
use serde::{Deserialize, Serialize};

/// Cumulative counters for one execution, never session totals or billable cost.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct TokenUsage {
    pub input_tokens: i64,
    pub cached_input_tokens: i64,
    pub output_tokens: i64,
    pub reasoning_output_tokens: i64,
    pub total_tokens: i64,
}

impl TokenUsage {
    pub fn valid(&self) -> bool {
        [
            self.input_tokens,
            self.cached_input_tokens,
            self.output_tokens,
            self.reasoning_output_tokens,
            self.total_tokens,
        ]
        .iter()
        .all(|n| *n >= 0)
            && self.cached_input_tokens <= self.input_tokens
            && self.reasoning_output_tokens <= self.output_tokens
    }

    pub fn checked_delta(&self, previous: &Self) -> Option<Self> {
        let delta = Self {
            input_tokens: self.input_tokens.checked_sub(previous.input_tokens)?,
            cached_input_tokens: self
                .cached_input_tokens
                .checked_sub(previous.cached_input_tokens)?,
            output_tokens: self.output_tokens.checked_sub(previous.output_tokens)?,
            reasoning_output_tokens: self
                .reasoning_output_tokens
                .checked_sub(previous.reasoning_output_tokens)?,
            total_tokens: self.total_tokens.checked_sub(previous.total_tokens)?,
        };
        delta.valid().then_some(delta)
    }

    pub fn checked_add(&self, delta: &Self) -> Option<Self> {
        Some(Self {
            input_tokens: self.input_tokens.checked_add(delta.input_tokens)?,
            cached_input_tokens: self
                .cached_input_tokens
                .checked_add(delta.cached_input_tokens)?,
            output_tokens: self.output_tokens.checked_add(delta.output_tokens)?,
            reasoning_output_tokens: self
                .reasoning_output_tokens
                .checked_add(delta.reasoning_output_tokens)?,
            total_tokens: self.total_tokens.checked_add(delta.total_tokens)?,
        })
    }
}

impl Repository {
    pub fn save_usage(&self, event: &IngestEvent, timestamp: &str) -> Result<()> {
        let id = event.execution_id.as_deref().ok_or_else(|| {
            rusqlite::Error::InvalidParameterName("usage.observed requires execution_id".into())
        })?;
        let usage: TokenUsage = serde_json::from_value(event.payload.clone())
            .map_err(|_| rusqlite::Error::InvalidParameterName("Invalid token counters".into()))?;
        if !usage.valid() {
            return Err(rusqlite::Error::InvalidParameterName(
                "Invalid token counters".into(),
            ));
        }
        self.db.with_conn(|conn| {
            conn.execute("INSERT INTO execution_usage VALUES(?1,?2,?3,?4,?5,?6,?7)
                ON CONFLICT(execution_id) DO UPDATE SET input_tokens=excluded.input_tokens,
                cached_input_tokens=excluded.cached_input_tokens,output_tokens=excluded.output_tokens,
                reasoning_output_tokens=excluded.reasoning_output_tokens,total_tokens=excluded.total_tokens,
                observed_at=excluded.observed_at WHERE julianday(excluded.observed_at)>=julianday(execution_usage.observed_at)",
                params![id,usage.input_tokens,usage.cached_input_tokens,usage.output_tokens,usage.reasoning_output_tokens,usage.total_tokens,timestamp])?;
            Ok(())
        })
    }

    pub fn get_usage(&self, id: &str) -> Result<Option<TokenUsage>> {
        self.db.with_conn(|conn| conn.query_row("SELECT input_tokens,cached_input_tokens,output_tokens,reasoning_output_tokens,total_tokens FROM execution_usage WHERE execution_id=?1",[id],|r| Ok(TokenUsage {
            input_tokens:r.get(0)?,cached_input_tokens:r.get(1)?,output_tokens:r.get(2)?,reasoning_output_tokens:r.get(3)?,total_tokens:r.get(4)?,
        })).optional())
    }
}
