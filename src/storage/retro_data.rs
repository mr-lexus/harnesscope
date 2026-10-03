use super::{evidence::EvidenceFilter, Repository};
use crate::capture;
use rusqlite::params;
use serde_json::{json, Value};

impl Repository {
    pub fn retro_tasks(&self, after: &str, limit: u32) -> rusqlite::Result<Value> {
        self.db.with_conn(|c| {
            let items:Vec<Value>=c.prepare("SELECT t.id,t.title,t.project,t.created_at,(SELECT COUNT(*) FROM task_links l WHERE l.task_id=t.id),(SELECT COUNT(*) FROM external_task_versions v WHERE v.task_id=t.id) FROM retro_tasks t WHERE t.id>?1 ORDER BY t.id LIMIT ?2")?.query_map(params![after,limit.clamp(1,200)],|r|Ok(json!({"id":r.get::<_,String>(0)?,"title":r.get::<_,String>(1)?,"project":r.get::<_,Option<String>>(2)?,"created_at":r.get::<_,String>(3)?,"links":r.get::<_,i64>(4)?,"versions":r.get::<_,i64>(5)?})))?.collect::<rusqlite::Result<_>>()?;
            Ok(json!({"items":items,"schema_version":1}))
        })
    }
    pub fn create_retro_task(
        &self,
        title: &str,
        project: Option<&str>,
    ) -> rusqlite::Result<String> {
        let id = uuid::Uuid::new_v4().to_string();
        let title = capture::metadata(title);
        if title.is_empty() || title.len() > 500 {
            return Err(rusqlite::Error::InvalidParameterName(
                "Task title must contain 1–500 characters".into(),
            ));
        }
        self.db.with_conn(|c| {
            c.execute(
                "INSERT INTO retro_tasks(id,title,project,created_at) VALUES(?1,?2,?3,?4)",
                params![
                    id,
                    title,
                    project.map(capture::metadata),
                    chrono::Utc::now().to_rfc3339()
                ],
            )
        })?;
        Ok(id)
    }
    pub fn link_retro_task(
        &self,
        task: &str,
        session: &str,
        turn: Option<&str>,
        confirmed: bool,
    ) -> rusqlite::Result<()> {
        self.db.with_conn(|c| {
            c.execute("INSERT INTO task_links(task_id,session_id,turn_id,status) VALUES(?1,?2,?3,?4) ON CONFLICT(task_id,session_id,turn_id) DO UPDATE SET status=excluded.status",params![task,capture::metadata(session),turn.map(capture::metadata).unwrap_or_default(),if confirmed {"confirmed"}else{"suggested"}])?; Ok(())
        })
    }
    pub fn mark_retro_task(&self, task: &str, kind: &str, note: &str) -> rusqlite::Result<()> {
        if ![
            "accepted",
            "rework",
            "repeated_mistake",
            "misunderstood",
            "successful_approach",
        ]
        .contains(&kind)
            || note.len() > 4000
        {
            return Err(rusqlite::Error::InvalidParameterName(
                "Invalid mark or note exceeds 4000 bytes".into(),
            ));
        }
        let safe = capture::sanitize(&json!(note));
        let note = safe
            .value()
            .and_then(Value::as_str)
            .unwrap_or("[excluded: unsafe content]");
        self.db.with_conn(|c| {
            c.execute(
                "INSERT INTO task_marks(task_id,kind,note,created_at) VALUES(?1,?2,?3,?4)",
                params![task, kind, note, chrono::Utc::now().to_rfc3339()],
            )?;
            Ok(())
        })
    }
    pub fn retro_task_detail(&self, task: &str) -> rusqlite::Result<Value> {
        self.db.with_conn(|c|{
            let links:Vec<Value>=c.prepare("SELECT session_id,turn_id,status FROM task_links WHERE task_id=?1 ORDER BY session_id,turn_id")?.query_map([task],|r|Ok(json!({"session_id":r.get::<_,String>(0)?,"turn_id":r.get::<_,String>(1)?,"status":r.get::<_,String>(2)?})))?.collect::<rusqlite::Result<_>>()?;
            let marks:Vec<Value>=c.prepare("SELECT kind,note,created_at FROM task_marks WHERE task_id=?1 ORDER BY id")?.query_map([task],|r|Ok(json!({"kind":r.get::<_,String>(0)?,"note":r.get::<_,String>(1)?,"created_at":r.get::<_,String>(2)?})))?.collect::<rusqlite::Result<_>>()?;
            let versions:Vec<Value>=c.prepare("SELECT id,observation_id,object_hash,observed_at FROM external_task_versions WHERE task_id=?1 ORDER BY observed_at LIMIT 200")?.query_map([task],|r|Ok(json!({"id":r.get::<_,String>(0)?,"observation_id":r.get::<_,String>(1)?,"object_hash":r.get::<_,String>(2)?,"observed_at":r.get::<_,String>(3)?})))?.collect::<rusqlite::Result<_>>()?;
            Ok(json!({"id":task,"links":links,"marks":marks,"versions":versions}))
        })
    }

    pub fn export_evidence(
        &self,
        filter: &EvidenceFilter,
        output: &std::path::Path,
    ) -> Result<Value, String> {
        if let Some(path) = self.database_path()? {
            let isolated = Repository::new(
                super::Database::open_read_only(&path)
                    .map_err(|_| "Cannot open export snapshot")?,
            );
            return isolated
                .db
                .read_snapshot(|| {
                    isolated
                        .export_snapshot(filter, output)
                        .map_err(rusqlite::Error::InvalidParameterName)
                })
                .map_err(|e| e.to_string());
        }
        self.db
            .read_snapshot(|| {
                self.export_snapshot(filter, output)
                    .map_err(rusqlite::Error::InvalidParameterName)
            })
            .map_err(|e| e.to_string())
    }

    fn export_snapshot(
        &self,
        filter: &EvidenceFilter,
        output: &std::path::Path,
    ) -> Result<Value, String> {
        use std::io::Write;
        if output.exists() {
            return Err("Export destination exists".into());
        }
        let parent = output
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(std::path::Path::new("."));
        let mut file =
            tempfile::NamedTempFile::new_in(parent).map_err(|_| "Cannot create export")?;
        let safe =
            capture::sanitize(&serde_json::to_value(filter).map_err(|_| "Invalid export filter")?);
        let mut f: EvidenceFilter =
            serde_json::from_value(safe.value().cloned().ok_or("Unsafe export filter")?)
                .map_err(|_| "Invalid export filter")?;
        f.after = None;
        f.limit = Some(100);
        let page = self.evidence_page(&f).map_err(|_| "Cannot freeze export")?;
        f.through = page["through"].as_i64();
        let manifest = json!({"type":"manifest","schema_version":1,"parser_version":1,"sanitizer_version":1,"filter":f,"created_at":chrono::Utc::now().to_rfc3339(),"coverage":self.evidence_coverage().map_err(|_|"Cannot read coverage")?});
        writeln!(file, "{manifest}").map_err(|_| "Cannot write export")?;
        let mut count = 0;
        loop {
            let page = self
                .evidence_page(&f)
                .map_err(|_| "Cannot read export page")?;
            for item in page["items"].as_array().unwrap() {
                let object = item["object_hash"]
                    .as_str()
                    .map(|h| self.evidence_object(h))
                    .transpose()
                    .map_err(|_| "Cannot read object")?
                    .flatten();
                writeln!(
                    file,
                    "{}",
                    json!({"type":"observation","metadata":item,"content":object})
                )
                .map_err(|_| "Cannot write export")?;
                count += 1;
            }
            f.after = page["next"].as_i64();
            if f.after.is_none() {
                break;
            }
        }
        // Workflow observations and human/task context travel with the evidence.
        // Never substitute today's files: these records refer to captured objects.
        let mut workflow_after = 0;
        loop {
            let page = self
                .workflow_versions(f.project.as_deref(), workflow_after)
                .map_err(|_| "Cannot export workflow")?;
            let items = page["items"].as_array().unwrap();
            for item in items {
                workflow_after = item["cursor"].as_i64().unwrap();
                if item["observed_at"].as_str() > manifest["created_at"].as_str() {
                    continue;
                }
                let content = item["object_hash"]
                    .as_str()
                    .map(|h| self.evidence_object(h))
                    .transpose()
                    .map_err(|_| "Cannot export workflow object")?
                    .flatten();
                writeln!(
                    file,
                    "{}",
                    json!({"type":"workflow","metadata":item,"content":content})
                )
                .map_err(|_| "Cannot write workflow export")?;
            }
            if items.len() < 100 {
                break;
            }
        }
        let mut task_after = String::new();
        loop {
            let page = self
                .retro_tasks(&task_after, 100)
                .map_err(|_| "Cannot export tasks")?;
            let items = page["items"].as_array().unwrap();
            for item in items {
                let id = item["id"].as_str().unwrap();
                task_after = id.to_owned();
                if f.task.as_deref().is_some_and(|task| task != id) {
                    continue;
                }
                let relevant = self
                    .evidence_page(&EvidenceFilter {
                        task: Some(id.into()),
                        limit: Some(1),
                        after: None,
                        ..f.clone()
                    })
                    .map_err(|_| "Cannot select tasks")?;
                if relevant["items"].as_array().unwrap().is_empty() {
                    continue;
                }
                let detail = self
                    .retro_task_detail(id)
                    .map_err(|_| "Cannot export task detail")?;
                writeln!(
                    file,
                    "{}",
                    json!({"type":"task","metadata":item,"detail":detail})
                )
                .map_err(|_| "Cannot write task export")?;
            }
            if items.len() < 100 {
                break;
            }
        }
        file.as_file()
            .sync_all()
            .map_err(|_| "Cannot sync export")?;
        file.persist_noclobber(output)
            .map_err(|_| "Cannot publish export")?;
        Ok(json!({"observations":count,"through":f.through,"path":output,"schema_version":1}))
    }

    pub fn import_retro_context(&self, record: &Value) -> rusqlite::Result<()> {
        let safe = capture::sanitize(record);
        let Some(record) = safe.value() else {
            return Err(rusqlite::Error::InvalidParameterName(
                "Unsafe package metadata".into(),
            ));
        };
        let m = &record["metadata"];
        let get = |k: &str| m[k].as_str().ok_or(rusqlite::Error::InvalidQuery);
        match record["type"].as_str(){
            Some("workflow")=>{
                let content=if record["content"].is_null(){capture::SafeContent::excluded("excluded_in_package")}else{capture::sanitize(&record["content"])};
                self.transaction(||{
                    let hash=self.store_object(&content)?;
                    self.db.with_conn(|c|{c.execute("INSERT OR IGNORE INTO workflow_versions(id,root,path,observed_at,object_hash,disposition,reason) VALUES(?1,?2,?3,?4,?5,?6,?7)",params![get("id")?,get("root")?,get("path")?,get("observed_at")?,hash,content.disposition,content.reason])?;Ok(())})
                })
            },
            Some("task")=>self.transaction(||{
                let id=get("id")?;
                self.db.with_conn(|c|{
                    c.execute("INSERT OR IGNORE INTO retro_tasks(id,title,project,created_at) VALUES(?1,?2,?3,?4)",params![id,get("title")?,m["project"].as_str(),get("created_at")?])?;
                    if let Some(links)=record["detail"]["links"].as_array(){for link in links {
                        if let Some(session)=link["session_id"].as_str(){c.execute("INSERT OR IGNORE INTO task_links(task_id,session_id,turn_id,status) VALUES(?1,?2,?3,?4)",params![id,session,link["turn_id"].as_str().unwrap_or(""),if link["status"]=="confirmed"{"confirmed"}else{"suggested"}])?;}
                    }}
                    if let Some(marks)=record["detail"]["marks"].as_array(){for mark in marks {
                        let kind=mark["kind"].as_str().ok_or(rusqlite::Error::InvalidQuery)?;let note=mark["note"].as_str().unwrap_or("");let time=mark["created_at"].as_str().ok_or(rusqlite::Error::InvalidQuery)?;
                        c.execute("INSERT INTO task_marks(task_id,kind,note,created_at) SELECT ?1,?2,?3,?4 WHERE NOT EXISTS(SELECT 1 FROM task_marks WHERE task_id=?1 AND kind=?2 AND note=?3 AND created_at=?4)",params![id,kind,note,time])?;
                    }}Ok(())
                })
            }),
            _=>Err(rusqlite::Error::InvalidQuery),
        }
    }
}
