# Database backup and recovery

Schema 6 snapshots include every referenced sanitized evidence object. External
`<database>.objects` files are hash-checked and embedded into the portable SQLite
backup one object at a time; restore needs only that backup file. Pending
`<database>.capture` hook records are excluded, like the wrapper outbox. Preserve
queues separately when moving an offline collector. A pre-v6 database is backed
up before migration. See [evidence setup](EVIDENCE.md).

`backup create` uses SQLite's online backup API. Committed records still in WAL are included; copying the `.db` file with a filesystem command while the server is running is not equivalent.

```sh
harnesscope backup create --output /existing/backup-directory/harnesscope-2026-10-03.db
harnesscope backup verify --file /existing/backup-directory/harnesscope-2026-10-03.db
harnesscope backup restore --file /existing/backup-directory/harnesscope-2026-10-03.db --output /existing/data-directory/restored.db
```

Create reads the configured `HARNESSCOPE_DB_PATH` (or the usual default). Verify and restore use their explicit input paths. All commands return a JSON report and a nonzero exit code on failure. Parent directories must already exist. Paths may contain spaces when quoted.

The snapshot includes imported events, projections, reviews, native-source registrations and file checkpoints. It **does not include pending outbox delivery, original transcript files or application configuration**. Every JSON report lists these exclusions. This is a main-database snapshot, not a full installation backup.

For a migration that must retain every queued observation:

1. Finish wrappers, pause native sources, and let the server drain delivery.
2. Inspect `harnesscope outbox status`; `harnesscope outbox flush` exits nonzero if queued batches for the configured destination remain. Other configured destinations have separate queues and must be drained too.
3. Stop the server, create and verify the snapshot. Preserve configuration and source files separately. Do not delete the original database/outbox until the restored installation has been checked.

The server may stay running for an ordinary point-in-time database snapshot. New records committed after the snapshot point will naturally be absent. Copying retries busy/locked steps for up to 60 seconds; a continuously changing source can make copying time out. Integrity verification is separate from this copy timeout and may take longer for very large files.

Before publication, create and restore run SQLite `integrity_check`, `foreign_key_check`, check the supported schema version and required tables, and report record counts. Verify is read-only: it never creates a missing input or migrates it. A temporary sibling file is used for copying; the destination is published with no-overwrite semantics only after validation and file synchronization. Existing destinations, symlinks and existing SQLite sidecars are refused. A terminated process may leave an unpublished `.harnesscope-backup-*` temporary file; the original database remains untouched.

Restore always writes a **new** database path. It applies supported schema migrations to that copy and pauses all registered native sources while retaining their checkpoints. It does not switch the running server or overwrite the configured database. To use the result, stop the old server and launch with the restored path:

```powershell
$env:HARNESSCOPE_DB_PATH = 'C:\data\restored.db'
harnesscope serve
```

On Linux/macOS: `HARNESSCOPE_DB_PATH=/data/restored.db harnesscope serve`. Check reviews and counts, inspect source paths, then explicitly resume the desired sources. Process observations remain historical evidence; restoration does not restart wrapped agents. Restoring under a new path starts a new outbox; any pending batches beside the original path remain there.

Snapshot files contain the same local telemetry as the database. No encryption or retention policy is added by these commands. Keep snapshots in storage appropriate for that data.
