CREATE TABLE evidence_objects (
 hash TEXT PRIMARY KEY, bytes INTEGER NOT NULL, inline_json TEXT
);
CREATE TABLE observations (
 sequence INTEGER PRIMARY KEY AUTOINCREMENT, id TEXT NOT NULL UNIQUE,
 schema_version INTEGER NOT NULL DEFAULT 1, channel TEXT NOT NULL,
 source TEXT NOT NULL, source_version TEXT, position TEXT NOT NULL,
 observed_at TEXT, received_at TEXT NOT NULL, session_id TEXT, turn_id TEXT,
 native_id TEXT, kind TEXT NOT NULL, project TEXT,
 object_hash TEXT REFERENCES evidence_objects(hash), disposition TEXT NOT NULL,
 reason TEXT, sanitizer_version INTEGER NOT NULL DEFAULT 1
);
CREATE INDEX observations_session ON observations(session_id,sequence);
CREATE INDEX observations_turn ON observations(turn_id,sequence);
CREATE INDEX observations_project ON observations(project,sequence);
CREATE TABLE evidence_items (
 observation_id TEXT PRIMARY KEY REFERENCES observations(id),
 category TEXT NOT NULL, role TEXT, tool TEXT, call_id TEXT, parent_session_id TEXT,
 certainty TEXT NOT NULL DEFAULT 'observed', parser_version INTEGER NOT NULL DEFAULT 1
);
CREATE TABLE retro_tasks (
 id TEXT PRIMARY KEY, title TEXT NOT NULL, project TEXT, created_at TEXT NOT NULL
);
CREATE TABLE task_links (
 task_id TEXT NOT NULL REFERENCES retro_tasks(id), session_id TEXT NOT NULL,
 turn_id TEXT NOT NULL DEFAULT '', status TEXT NOT NULL CHECK(status IN ('confirmed','suggested')),
 evidence_id TEXT REFERENCES observations(id), PRIMARY KEY(task_id,session_id,turn_id)
);
CREATE TABLE task_marks (
 id INTEGER PRIMARY KEY, task_id TEXT NOT NULL REFERENCES retro_tasks(id),
 kind TEXT NOT NULL, note TEXT NOT NULL, created_at TEXT NOT NULL
);
CREATE TABLE workflow_roots (path TEXT PRIMARY KEY, enabled INTEGER NOT NULL DEFAULT 1);
CREATE TABLE evidence_cursors (source TEXT PRIMARY KEY, position INTEGER NOT NULL DEFAULT 0);
CREATE TABLE workflow_versions (
 id TEXT PRIMARY KEY, root TEXT NOT NULL, path TEXT NOT NULL,
 observed_at TEXT NOT NULL, object_hash TEXT REFERENCES evidence_objects(hash),
 disposition TEXT NOT NULL, reason TEXT, state TEXT NOT NULL DEFAULT 'discovered'
);
CREATE INDEX workflow_path_time ON workflow_versions(root,path,observed_at);
CREATE TABLE external_task_versions (
 id TEXT PRIMARY KEY, task_id TEXT NOT NULL REFERENCES retro_tasks(id),
 observation_id TEXT NOT NULL REFERENCES observations(id),
 object_hash TEXT NOT NULL REFERENCES evidence_objects(hash), observed_at TEXT NOT NULL,
 UNIQUE(task_id,observation_id)
);
INSERT INTO _schema_migrations(version,applied_at) VALUES(6,datetime('now'));
