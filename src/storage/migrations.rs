use rusqlite::{Connection, Result};

pub fn run_migrations(conn: &mut Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS _schema_migrations (
            version INTEGER PRIMARY KEY,
            applied_at TEXT NOT NULL
        );
        "#,
    )?;

    let current_version: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM _schema_migrations",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    if current_version < 1 {
        apply_migration_v1(conn)?;
        conn.execute(
            "INSERT INTO _schema_migrations (version, applied_at) VALUES (1, datetime('now'))",
            [],
        )?;
    }

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
