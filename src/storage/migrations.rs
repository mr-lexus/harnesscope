use rusqlite::{Connection, Result};

pub fn run_migrations(conn: &mut Connection) -> Result<()> {
    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let conn = &tx;
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS _schema_migrations (
            version INTEGER PRIMARY KEY,
            applied_at TEXT NOT NULL
        );
        "#,
    )?;

    let current_version: i64 = conn.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM _schema_migrations",
        [],
        |row| row.get(0),
    )?;

    if current_version > 6 {
        return Err(rusqlite::Error::InvalidParameterName(
            "Database schema is newer than this binary".into(),
        ));
    }

    if current_version < 1 {
        apply_migration_v1(conn)?;
        conn.execute(
            "INSERT INTO _schema_migrations (version, applied_at) VALUES (1, datetime('now'))",
            [],
        )?;
    }

    if current_version < 2 {
        apply_migration_v2(conn)?;
        conn.execute(
            "INSERT INTO _schema_migrations (version, applied_at) VALUES (2, datetime('now'))",
            [],
        )?;
    }

    if current_version < 3 {
        conn.execute_batch(r#"
            CREATE TABLE session_aliases (
                alias TEXT PRIMARY KEY,
                session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE
            );
            ALTER TABLE executions ADD COLUMN capture_scope TEXT NOT NULL DEFAULT 'UNKNOWN';
            CREATE TABLE execution_reviews (
                execution_id TEXT PRIMARY KEY REFERENCES executions(id) ON DELETE CASCADE,
                outcome TEXT NOT NULL CHECK(outcome IN ('UNREVIEWED','ACCEPTED','REWORK','REJECTED')),
                notes TEXT NOT NULL DEFAULT '',
                experiment TEXT NOT NULL DEFAULT '',
                updated_at TEXT NOT NULL
            );
            CREATE INDEX idx_reviews_experiment ON execution_reviews(experiment);
            CREATE INDEX idx_executions_status ON executions(status);
            INSERT INTO _schema_migrations(version, applied_at) VALUES (3, datetime('now'));
        "#)?;
    }
    if current_version < 4 {
        conn.execute_batch(r#"
            CREATE TABLE telemetry_sources (
                id TEXT PRIMARY KEY, adapter TEXT NOT NULL, path TEXT NOT NULL UNIQUE,
                enabled INTEGER NOT NULL DEFAULT 1, include_content INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL, last_scan_at TEXT, last_success_at TEXT, last_error TEXT
            );
            CREATE TABLE source_files (
                source_id TEXT NOT NULL REFERENCES telemetry_sources(id) ON DELETE CASCADE,
                path TEXT NOT NULL, byte_offset INTEGER NOT NULL DEFAULT 0, line_number INTEGER NOT NULL DEFAULT 0,
                prefix_hash TEXT NOT NULL DEFAULT '', state_json TEXT NOT NULL DEFAULT '{}',
                events_count INTEGER NOT NULL DEFAULT 0, ignored_count INTEGER NOT NULL DEFAULT 0,
                status TEXT NOT NULL DEFAULT 'NEW', last_error TEXT, updated_at TEXT NOT NULL,
                file_size INTEGER NOT NULL DEFAULT 0, modified_stamp TEXT NOT NULL DEFAULT '',
                PRIMARY KEY(source_id,path)
            );
            CREATE TABLE execution_usage (
                execution_id TEXT PRIMARY KEY REFERENCES executions(id) ON DELETE CASCADE,
                input_tokens INTEGER NOT NULL, cached_input_tokens INTEGER NOT NULL,
                output_tokens INTEGER NOT NULL, reasoning_output_tokens INTEGER NOT NULL,
                total_tokens INTEGER NOT NULL, observed_at TEXT NOT NULL
            );
            INSERT INTO _schema_migrations(version,applied_at) VALUES(4,datetime('now'));
        "#)?;
    }
    if current_version < 5 {
        conn.execute_batch(
            "CREATE TABLE runtime_observations (
            runtime_id TEXT PRIMARY KEY REFERENCES runtime_instances(id) ON DELETE CASCADE,
            observed_at TEXT NOT NULL, received_at TEXT NOT NULL);
            INSERT INTO _schema_migrations(version,applied_at) VALUES(5,datetime('now'));",
        )?;
    }
    if current_version < 6 {
        conn.execute_batch(include_str!("evidence.sql"))?;
        // Existing event IDs are idempotent. Replay available originals to fill
        // the new archive; user reviews and all old events remain untouched.
        conn.execute("DELETE FROM source_files", [])?;
    }
    tx.commit()
}

fn apply_migration_v2(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        -- Add forking fields to sessions
        ALTER TABLE sessions ADD COLUMN parent_session_id TEXT REFERENCES sessions(id);
        ALTER TABLE sessions ADD COLUMN fork_reason TEXT;
        ALTER TABLE sessions ADD COLUMN forked_at TEXT;

        -- Create session_conflicts table
        CREATE TABLE IF NOT EXISTS session_conflicts (
            id TEXT PRIMARY KEY,
            session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
            conflicting_session_id TEXT REFERENCES sessions(id) ON DELETE CASCADE,
            execution_id TEXT REFERENCES executions(id) ON DELETE CASCADE,
            conflict_type TEXT NOT NULL,
            severity TEXT NOT NULL DEFAULT 'WARNING',
            detected_at TEXT NOT NULL,
            resolved_at TEXT,
            details_json TEXT
        );

        CREATE INDEX IF NOT EXISTS idx_sessions_parent ON sessions(parent_session_id);
        CREATE INDEX IF NOT EXISTS idx_conflicts_session ON session_conflicts(session_id);
        CREATE INDEX IF NOT EXISTS idx_conflicts_detected ON session_conflicts(detected_at);
        "#,
    )?;
    Ok(())
}

fn apply_migration_v1(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        -- 1. runtime_instances
        CREATE TABLE IF NOT EXISTS runtime_instances (
            id TEXT PRIMARY KEY,
            runner_name TEXT NOT NULL,
            runner_version TEXT NOT NULL DEFAULT 'UNKNOWN',
            surface TEXT NOT NULL DEFAULT 'cli',
            pid INTEGER,
            hostname TEXT NOT NULL,
            os TEXT NOT NULL,
            cwd TEXT NOT NULL,
            command_line TEXT NOT NULL,
            started_at TEXT NOT NULL,
            ended_at TEXT,
            exit_code INTEGER,
            status TEXT NOT NULL DEFAULT 'RUNNING'
        );

        -- 2. sessions
        CREATE TABLE IF NOT EXISTS sessions (
            id TEXT PRIMARY KEY,
            runner_name TEXT NOT NULL,
            native_session_id TEXT NOT NULL DEFAULT 'UNKNOWN',
            title TEXT,
            started_at TEXT NOT NULL,
            ended_at TEXT,
            status TEXT NOT NULL DEFAULT 'ACTIVE',
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        );

        -- 3. runtime_session_bindings
        CREATE TABLE IF NOT EXISTS runtime_session_bindings (
            id TEXT PRIMARY KEY,
            runtime_id TEXT NOT NULL REFERENCES runtime_instances(id) ON DELETE CASCADE,
            session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
            bound_at TEXT NOT NULL,
            unbound_at TEXT,
            reason TEXT NOT NULL DEFAULT 'START'
        );

        -- 4. executions
        CREATE TABLE IF NOT EXISTS executions (
            id TEXT PRIMARY KEY,
            session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
            runtime_id TEXT NOT NULL REFERENCES runtime_instances(id) ON DELETE CASCADE,
            native_execution_id TEXT NOT NULL DEFAULT 'UNKNOWN',
            turn_index INTEGER NOT NULL DEFAULT 0,
            prompt_summary TEXT,
            model TEXT NOT NULL DEFAULT 'UNKNOWN',
            reasoning_effort TEXT NOT NULL DEFAULT 'UNKNOWN',
            selected_agent_role TEXT NOT NULL DEFAULT 'UNKNOWN',
            started_at TEXT NOT NULL,
            ended_at TEXT,
            duration_ms INTEGER,
            status TEXT NOT NULL DEFAULT 'RUNNING',
            exit_code INTEGER,
            error_message TEXT,
            repo_root TEXT,
            worktree_path TEXT,
            branch TEXT,
            head_sha TEXT,
            git_attribution TEXT NOT NULL DEFAULT 'UNKNOWN'
        );

        -- 5. agent_instances
        CREATE TABLE IF NOT EXISTS agent_instances (
            id TEXT PRIMARY KEY,
            execution_id TEXT NOT NULL REFERENCES executions(id) ON DELETE CASCADE,
            parent_agent_id TEXT REFERENCES agent_instances(id),
            agent_role TEXT NOT NULL DEFAULT 'main',
            agent_name TEXT NOT NULL,
            model TEXT NOT NULL DEFAULT 'UNKNOWN',
            started_at TEXT NOT NULL,
            ended_at TEXT,
            status TEXT NOT NULL DEFAULT 'RUNNING'
        );

        -- 6. components
        CREATE TABLE IF NOT EXISTS components (
            id TEXT PRIMARY KEY,
            component_type TEXT NOT NULL,
            name TEXT NOT NULL,
            version TEXT DEFAULT 'UNKNOWN',
            description TEXT,
            UNIQUE(component_type, name, version)
        );

        -- 7. execution_components
        CREATE TABLE IF NOT EXISTS execution_components (
            execution_id TEXT NOT NULL REFERENCES executions(id) ON DELETE CASCADE,
            component_id TEXT NOT NULL REFERENCES components(id) ON DELETE CASCADE,
            state TEXT NOT NULL DEFAULT 'DISCOVERED',
            invocations_count INTEGER NOT NULL DEFAULT 0,
            details_json TEXT,
            PRIMARY KEY (execution_id, component_id)
        );

        -- 8. git_snapshots
        CREATE TABLE IF NOT EXISTS git_snapshots (
            id TEXT PRIMARY KEY,
            execution_id TEXT NOT NULL REFERENCES executions(id) ON DELETE CASCADE,
            snapshot_type TEXT NOT NULL,
            captured_at TEXT NOT NULL,
            repo_root TEXT NOT NULL,
            worktree_path TEXT NOT NULL,
            branch TEXT NOT NULL,
            head_commit TEXT NOT NULL,
            is_dirty INTEGER NOT NULL DEFAULT 0,
            changed_files_count INTEGER NOT NULL DEFAULT 0,
            diff_stat TEXT,
            changed_files_json TEXT,
            attribution TEXT NOT NULL DEFAULT 'UNKNOWN'
        );

        -- 9. config_snapshots
        CREATE TABLE IF NOT EXISTS config_snapshots (
            id TEXT PRIMARY KEY,
            content_sha256 TEXT NOT NULL UNIQUE,
            config_type TEXT NOT NULL,
            file_path TEXT,
            raw_content_redacted TEXT NOT NULL,
            parsed_json TEXT,
            captured_at TEXT NOT NULL
        );

        -- 10. events
        CREATE TABLE IF NOT EXISTS events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            event_id TEXT NOT NULL UNIQUE,
            timestamp TEXT NOT NULL,
            runtime_id TEXT,
            session_id TEXT,
            execution_id TEXT,
            agent_instance_id TEXT,
            event_type TEXT NOT NULL,
            source TEXT NOT NULL,
            payload_json TEXT NOT NULL
        );

        -- Indices
        CREATE INDEX IF NOT EXISTS idx_sessions_runner_native ON sessions(runner_name, native_session_id);
        CREATE INDEX IF NOT EXISTS idx_bindings_session ON runtime_session_bindings(session_id);
        CREATE INDEX IF NOT EXISTS idx_bindings_runtime ON runtime_session_bindings(runtime_id);
        CREATE INDEX IF NOT EXISTS idx_executions_session ON executions(session_id);
        CREATE INDEX IF NOT EXISTS idx_executions_runtime ON executions(runtime_id);
        CREATE INDEX IF NOT EXISTS idx_executions_started ON executions(started_at);
        CREATE INDEX IF NOT EXISTS idx_executions_worktree ON executions(worktree_path);
        CREATE INDEX IF NOT EXISTS idx_agent_instances_exec ON agent_instances(execution_id);
        CREATE INDEX IF NOT EXISTS idx_exec_components_exec ON execution_components(execution_id);
        CREATE INDEX IF NOT EXISTS idx_git_snapshots_exec ON git_snapshots(execution_id);
        CREATE INDEX IF NOT EXISTS idx_events_execution ON events(execution_id);
        CREATE INDEX IF NOT EXISTS idx_events_session ON events(session_id);
        CREATE INDEX IF NOT EXISTS idx_events_timestamp ON events(timestamp);
        "#,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn version_two() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        apply_migration_v1(&conn).unwrap();
        apply_migration_v2(&conn).unwrap();
        conn.execute_batch("CREATE TABLE _schema_migrations(version INTEGER PRIMARY KEY,applied_at TEXT NOT NULL); INSERT INTO _schema_migrations VALUES(1,'before'),(2,'before');").unwrap();
        conn
    }
    #[test]
    fn upgrades_v2_once_and_preserves_existing_records() {
        let mut conn = version_two();
        conn.execute("INSERT INTO sessions(id,runner_name,started_at) VALUES('existing','custom','2026-01-01T00:00:00Z')",[]).unwrap();
        run_migrations(&mut conn).unwrap();
        run_migrations(&mut conn).unwrap();
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM sessions", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            1
        );
        assert_eq!(
            conn.query_row("SELECT MAX(version) FROM _schema_migrations", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            6
        );
    }
    #[test]
    fn failed_upgrade_rolls_back_ddl_and_version() {
        let mut conn = version_two();
        conn.execute_batch("CREATE TABLE execution_reviews(existing TEXT);")
            .unwrap();
        assert!(run_migrations(&mut conn).is_err());
        assert_eq!(
            conn.query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE name='session_aliases'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            0
        );
        assert_eq!(
            conn.query_row("SELECT MAX(version) FROM _schema_migrations", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            2
        );
        assert!(conn
            .prepare("SELECT capture_scope FROM executions")
            .is_err());
    }
}
