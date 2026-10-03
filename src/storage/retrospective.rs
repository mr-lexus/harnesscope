use super::Repository;
use rusqlite::{params, OptionalExtension, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Review {
    pub outcome: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub experiment: String,
}

impl Review {
    pub fn validate(&self) -> std::result::Result<(), &'static str> {
        if !matches!(
            self.outcome.as_str(),
            "UNREVIEWED" | "ACCEPTED" | "REWORK" | "REJECTED"
        ) {
            return Err("outcome must be UNREVIEWED, ACCEPTED, REWORK or REJECTED");
        }
        if self.notes.len() > 10_000 || self.experiment.len() > 120 {
            return Err("Notes or experiment name is too long");
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedReview {
    pub outcome: String,
    pub notes: String,
    pub experiment: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize, Default)]
pub struct RetrospectiveFilter {
    pub days: Option<u32>,
    pub runner: Option<String>,
    pub project: Option<String>,
    pub scope: Option<String>,
    pub include_demo: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct Cohort {
    pub runner: String,
    pub model: String,
    pub scope: String,
    pub experiment: String,
    pub executions: i64,
    pub completed: i64,
    pub failed: i64,
    pub reviewed: i64,
    pub accepted: i64,
    pub rework: i64,
    pub rejected: i64,
    pub avg_duration_ms: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct AttentionItem {
    pub id: String,
    pub runner: String,
    pub status: String,
    pub started_at: String,
    pub prompt_summary: Option<String>,
    pub reason: String,
}

#[derive(Debug, Serialize)]
pub struct Retrospective {
    pub total: i64,
    pub running: i64,
    pub failed: i64,
    pub reviewed: i64,
    pub accepted: i64,
    pub unknown_model: i64,
    pub unknown_session: i64,
    pub ambiguous_git: i64,
    pub process_captures: i64,
    pub turn_captures: i64,
    pub unknown_scope: i64,
    pub cohorts: Vec<Cohort>,
    pub attention: Vec<AttentionItem>,
    pub runners: Vec<String>,
    pub projects: Vec<String>,
    pub last_event_at: Option<String>,
}

impl Repository {
    pub fn save_review(&self, id: &str, review: &Review) -> Result<()> {
        review
            .validate()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.into()))?;
        self.db.with_conn(|conn| {
            conn.execute("INSERT INTO execution_reviews(execution_id,outcome,notes,experiment,updated_at)
                VALUES (?1,?2,?3,?4,?5) ON CONFLICT(execution_id) DO UPDATE SET
                outcome=excluded.outcome, notes=excluded.notes, experiment=excluded.experiment, updated_at=excluded.updated_at",
                params![id, review.outcome, crate::redact::redact_secrets(&review.notes),
                    crate::redact::redact_secrets(review.experiment.trim()), chrono::Utc::now().to_rfc3339()])?;
            Ok(())
        })
    }

    pub fn get_review(&self, id: &str) -> Result<Option<SavedReview>> {
        self.db.with_conn(|conn| conn.query_row("SELECT outcome,notes,experiment,updated_at FROM execution_reviews WHERE execution_id=?1", [id], |r| {
            Ok(SavedReview { outcome:r.get(0)?, notes:r.get(1)?, experiment:r.get(2)?, updated_at:r.get(3)? })
        }).optional())
    }

    pub fn retrospective(&self, filter: &RetrospectiveFilter) -> Result<Retrospective> {
        self.db.with_conn(|conn| {
            let days = filter.days.unwrap_or(30).clamp(1, 3650);
            let cutoff = (chrono::Utc::now() - chrono::Duration::days(days.into())).to_rfc3339();
            let runner = filter.runner.as_deref().filter(|s| !s.is_empty());
            let project = filter.project.as_deref().filter(|s| !s.is_empty());
            let scope = filter.scope.as_deref().filter(|s| !s.is_empty());
            let include_demo = filter.include_demo.unwrap_or(false);
            let base = "FROM executions e JOIN runtime_instances r ON r.id=e.runtime_id
                JOIN sessions s ON s.id=e.session_id LEFT JOIN execution_reviews v ON v.execution_id=e.id
                WHERE julianday(e.started_at)>=julianday(?1) AND (?2 IS NULL OR r.runner_name=?2)
                AND (?3 IS NULL OR COALESCE(e.repo_root,NULLIF(r.cwd,''))=?3) AND (?4 IS NULL OR e.capture_scope=?4)
                AND (?5 OR e.capture_scope!='DEMO')";
            let args = params![cutoff,runner,project,scope,include_demo];
            let summary = format!("SELECT COUNT(*),COALESCE(SUM(e.status='RUNNING'),0),COALESCE(SUM(e.status='FAILED'),0),
                COALESCE(SUM(v.outcome!='UNREVIEWED'),0),COALESCE(SUM(v.outcome='ACCEPTED'),0),
                COALESCE(SUM(e.model='UNKNOWN'),0),COALESCE(SUM(s.native_session_id='UNKNOWN'),0),
                COALESCE(SUM(e.git_attribution='AMBIGUOUS'),0),COALESCE(SUM(e.capture_scope='PROCESS'),0),
                COALESCE(SUM(e.capture_scope='TURN'),0),COALESCE(SUM(e.capture_scope='UNKNOWN'),0) {base}");
            let mut result = conn.query_row(&summary,args,|r| Ok(Retrospective {
                total:r.get(0)?,running:r.get(1)?,failed:r.get(2)?,reviewed:r.get(3)?,accepted:r.get(4)?,
                unknown_model:r.get(5)?,unknown_session:r.get(6)?,ambiguous_git:r.get(7)?,process_captures:r.get(8)?,
                turn_captures:r.get(9)?,unknown_scope:r.get(10)?,cohorts:vec![],attention:vec![],runners:vec![],projects:vec![],last_event_at:None
            }))?;
            let sql = format!("SELECT r.runner_name,e.model,e.capture_scope,COALESCE(v.experiment,''),COUNT(*),
                SUM(e.status='COMPLETED'),SUM(e.status='FAILED'),COALESCE(SUM(v.outcome!='UNREVIEWED'),0),
                COALESCE(SUM(v.outcome='ACCEPTED'),0),COALESCE(SUM(v.outcome='REWORK'),0),COALESCE(SUM(v.outcome='REJECTED'),0),
                AVG(CASE WHEN e.status!='RUNNING' THEN e.duration_ms END) {base}
                GROUP BY r.runner_name,e.model,e.capture_scope,COALESCE(v.experiment,'') ORDER BY COUNT(*) DESC,r.runner_name,e.model,e.capture_scope,COALESCE(v.experiment,'')");
            result.cohorts = conn.prepare(&sql)?.query_map(args,|r| Ok(Cohort {
                runner:r.get(0)?,model:r.get(1)?,scope:r.get(2)?,experiment:r.get(3)?,executions:r.get(4)?,completed:r.get(5)?,
                failed:r.get(6)?,reviewed:r.get(7)?,accepted:r.get(8)?,rework:r.get(9)?,rejected:r.get(10)?,avg_duration_ms:r.get(11)?
            }))?.collect::<Result<_>>()?;
            let sql = format!("SELECT e.id,r.runner_name,e.status,e.started_at,e.prompt_summary,
                CASE WHEN e.status='RUNNING' THEN 'No completion event yet'
                WHEN e.status='FAILED' THEN 'Process or execution failed'
                WHEN e.git_attribution='AMBIGUOUS' THEN 'Overlapping worktree activity'
                ELSE 'Needs a human outcome review' END {base}
                AND (e.status IN ('RUNNING','FAILED') OR e.git_attribution='AMBIGUOUS' OR COALESCE(v.outcome,'UNREVIEWED')='UNREVIEWED')
                ORDER BY (e.status='RUNNING') DESC,(e.status='FAILED') DESC,e.started_at DESC,e.id DESC LIMIT 20");
            result.attention = conn.prepare(&sql)?.query_map(args,|r| Ok(AttentionItem {
                id:r.get(0)?,runner:r.get(1)?,status:r.get(2)?,started_at:r.get(3)?,prompt_summary:r.get(4)?,reason:r.get(5)?
            }))?.collect::<Result<_>>()?;
            result.runners = conn.prepare("SELECT DISTINCT runner_name FROM runtime_instances ORDER BY runner_name")?.query_map([],|r| r.get(0))?.collect::<Result<_>>()?;
            result.projects = conn.prepare("SELECT DISTINCT COALESCE(e.repo_root,NULLIF(r.cwd,'')) AS project FROM executions e JOIN runtime_instances r ON r.id=e.runtime_id WHERE project IS NOT NULL ORDER BY project")?.query_map([],|r| r.get(0))?.collect::<Result<_>>()?;
            result.last_event_at = conn.query_row("SELECT MAX(timestamp) FROM events",[],|r| r.get(0))?;
            Ok(result)
        })
    }
}
