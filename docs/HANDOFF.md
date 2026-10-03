# Harnesscope — continuation checkpoint

Updated 2026-10-03. Workspace: `C:\server\harnesscope` (Windows/PowerShell).

**Current state:** see the final **Installed collector stage** section below. The production collector is installed and running on port **4242** with the user's authorized Codex metadata collection and login startup. Earlier fixture-only and no-personal-import notes describe previous checkpoints. The current priority is reliable collection and retrospective use; a second native adapter is not requested.

## Product intent and authorization

Local retrospective of AI coding workflows: observe evidence, assess useful outcomes, compare similar tasks and improve the next run. The user authorizes substantial refactoring and usability improvements. Do not equate successful process exit with task quality, discovered tools with invoked tools, or historical transcripts with live processes. Use Context7 for library documentation. Do not spawn subagents without explicit authorization.

## Completed stage

- Broad correctness/security audit, atomic event projection and migrations, redaction, local-only HTTP, normalized event import, explicit PROCESS/TURN/UNKNOWN/DEMO scopes, outcome reviews and cohort overview.
- Native Codex rollout adapter: explicit source registration, metadata-only default, checkpoint/restart, actual turns, tool calls and observed usage. Conservative format support and unmapped diagnostics; no automatic personal-history import.
- Durable wrapper outbox beside the configured database, ordered batches per runtime, claim leases, backoff, exact acknowledgement, replay-safe delivery and server-owned recovery. CLI: `outbox status`, `retry`, `flush`. Atomic `/api/v1/delivery` complements existing per-event `/events`.
- Heartbeat every 15s; freshness expires after 45s using observation time. No automatic failure inference or PID-based identity reuse. Schema v5. Latest heartbeat projection is a monotonic upsert, not a growing raw event history.
- Monitor page for runtime freshness and delivery queue. Queue diagnostics limited to 100 batches with SQL aggregate totals; runtime list capped at 200.
- Compact responsive panel: navigation, persistent density, pause UI polling, URL filters, session pagination, mobile record/cohort cards, source drawer and collapsed diagnostics, searchable collapsed events, review draft preservation across polling/tab switches, lazy page loading.

## Validation completed

64 Rust tests passed across the full suite and targeted reruns after final changes (12 unit, 13 audit, 5 CLI, 6 delivery, 16 integration, 12 native-source). Real CLI tests cover offline recovery with exit 7, periodic heartbeat, auto-start/custom port and collector restart. Clippy with warnings denied, rustfmt, frontend production build and `git diff --check` passed.

Browser QA covered 320/390/768/1024/1440 px, review saving, draft persistence, filter return, event search, drawer accessibility, density persistence, polling pause/resume and Monitor. No console errors observed. Large real archives and sustained heavy load remain unbenchmarked; Linux/macOS CI has not been executed from this session.

## Working state

Changes remain uncommitted. The large dirty tree contains this work and preceding stages; do not reset it. Original user file `copilot-handoff.json` must be preserved. Nothing was pushed, released or published.

Preview: `http://127.0.0.1:4343/`, binary `.audit/harnesscope-preview.exe`, separate fixture DB `.audit/native-preview.db`. It contains synthetic Codex turns, a `panel-qa` test review and a real harmless `preview-check` command observation. No personal agent history was imported. Last serve exec session: 47193 (check health before reuse). To update: build frontend, build Rust, gracefully stop preview via `/api/v1/shutdown`, copy the new binary, serve with the same explicit DB/data-directory environment. Browser viewport override was reset; density is compact and polling resumed.

Disk C: became full from accumulated build artifacts. `cargo clean -p harnesscope` safely removed generated package artifacts; subsequent rebuild/tests passed. After the final verification the package build cache was cleaned again; the copied preview binary remains available. Prefer `cargo ... -j 1` on this machine. Do not delete unrelated caches or kill other projects' build processes. Use explicit UTF-8 for scripted file edits.

## Reliability stage completed after this checkpoint

Completed 2026-10-03:

- Synthetic HTTP benchmark at 10k/100k events and offline queue benchmark at 100k. Scripts: `scripts/benchmark.py`, `examples/queue_benchmark.rs`; raw results `docs/benchmarks/`. Report: `docs/PERFORMANCE.ru.md`.
- Separate cursor-paginated event history APIs for executions/sessions. Detail responses now return `events_total`, not an unbounded `events` array. The Events tab loads 50 rows only when opened, searches the full history by type/source and polls only the latest page. Arrival ID cursors handle delayed timestamps. Compact rows are ~28px on desktop; mobile retains touch targets.
- Transactional outbox payload-byte ledger maintained with insert/delete/update triggers; one-time backfill for old queues. Removed repeated scans of all payloads. Queue budget, ordering, redaction and lease semantics preserved.
- CLI `backup create/verify/restore`: SQLite online backup with bounded copy retry, integrity/foreign-key/schema checks, no-overwrite publication, restore to a new path with collectors paused. Main database only; pending outbox, source files and configuration excluded explicitly. See `docs/BACKUPS.md`.
- 72 tests passed: 13 unit, 13 audit, 4 backup, 5 CLI, 8 delivery, 1 event pages, 16 integration, 12 native sources. Eight real offline wrappers recover without duplicate executions; eight concurrent producers/four consumers preserve stream order; WAL backups and concurrent writes tested. Clippy warnings-denied and frontend build passed.
- 100k-event detail: 5.54s / 43.37MB before, 14ms / 1.42KB after; server peak working set 269MB → 26MB. Queue enqueue 53s → 37.9s. Debug Windows synthetic figures, not production guarantees.
- Browser checked journal pagination, search across 60 matching events, empty results, payload expansion and 320/1440px layouts. Preview contains an additional synthetic `timeline-qa` execution with 122 events. Latest server session: 47193; same explicit `.audit/native-preview.db` and port 4343.

Next product stage: second native adapter and an explicit capability contract per adapter (turns, tools, usage, fork lineage, format support), with small real-format fixtures and conservative diagnostics. Pick the adapter from the intended user's actual workflow. Remaining scale work is separate: many short executions/cohorts, thousands of source files, larger payloads, long-running soak and other OSes. Source-file diagnostics and session execution lists are still unbounded; only event history was paginated here. No retention deletion, scheduled jobs, OS services or automatic personal-history import added.

## Reference documents

- `docs/AUDIT.ru.md`: findings and verification.
- `docs/DELIVERY.md`: queue/heartbeat lifecycle, CLI, bounds and UI workflow.
- `docs/NATIVE_SOURCES.md`: Codex format support and privacy.
- `docs/EVENTS.md`: normalized contract and APIs.
- `docs/PRODUCT.ru.md`: philosophy, product direction and competitors researched previously.

No callable tool for forcing this conversation's compaction was available. This file preserves the checkpoint; it does not itself compact the chat. The app documents `/compact` as the user command.


## Installed collector stage — 2026-10-03

The user rejected expanding to a second adapter now and asked for a ready product that collects data. This authorizes local real Codex metadata collection; the earlier no-personal-history-import checkpoint is superseded for this requested setup. Do not propose another adapter as the default next task.

- Installed optimized Windows executable in `%LOCALAPPDATA%/Programs/harnesscope/bin/harnesscope.exe`, connected `%USERPROFILE%/.codex/sessions` in metadata-only mode, using the pre-existing default database `%LOCALAPPDATA%/harnesscope/harnesscope/data/harnesscope.db` and port **4242**. The old v2 database (27 events) was backed up before migration; no old records were deleted. Backups are in its `backups` sibling directory. Production is distinct from the former `.audit` fixture panel on 4343.
- `scripts/install-local.ps1` installs a locally built release, validates an existing listener's database before stopping it, backs up before migration/update, registers/resumes Codex, creates hidden VBS launchers, Start-menu open/stop shortcuts and optional user Startup shortcut. Invoked with `-AutoStart`. User PATH updated. Installed binary is self-contained; repository/build runtime not needed. Autostart is at sign-in, not a service/watchdog. Startup launcher was invoked after a stop and successfully restarted collection.
- Added lightweight `/api/v1/collection` status with default-path detection and explicit active/paused/backlog/overdue/error states. Global badge links to Sources; first-run panel leads to source setup instead of requiring wrapper knowledge. Sources can prefill the detected Codex directory.
- Added current Codex `token_usage_record.turn_token_usage` support; native per-turn totals take precedence over legacy counts, which differ after compaction. Final samples can update the last completed turn. Checkpoint fields have serde defaults for existing histories. Increased bounded JSONL line size to 32 MiB after observing a real 14 MiB private output line. Ignored private records no longer serialize/hash their complete JSON unnecessarily. Known duplicate item presentation and world-state records are deliberately omitted.
- Initial history catch-up uses 250 ms between bounded passes while backlog exists; idle polling remains 5 s. Graceful SIGTERM added on Unix.
- Full optimized suite: 75 tests passed (15 native-source tests); Clippy release/all-targets warnings-denied passed. Frontend production build passed. Subsequent CLI smoke rerun covers the adaptive collector loop. Real database quick_check was ok. Of 99 completed turns with modern usage in a metadata-only sample of 20 recent files, 98 matched exactly; the remaining sample had no observed task_started and correctly has no invented execution.
- Real archive: 288 files, about 138k source records / 627MB. Three legacy July fork files mix child metadata and copied parent history and remain ERROR; the other files collect normally. This limitation is visible, not silently discarded or repaired. No source transcript was edited. No messages/tool outputs were stored by the collector. Do not commit local rollouts or database contents.
- Operational guide: `docs/RUNNING.ru.md`. No new adapter, external publication or remote upload.

## Review and commit stage — 2026-10-03

The user explicitly requested code review, small reliability/optimization fixes, commit and push of the accumulated project work. The previous no-push statement is superseded by that request. Preserve and exclude the original untracked `copilot-handoff.json`; local telemetry and database files must not be committed.

Final local verification: 77 Rust tests passed, release/all-target Clippy with warnings denied, rustfmt, frontend production build and both dependency audits passed. Fixed repeated session metadata resetting legacy usage, skipped unchanged WAITING files, replaced the second checkpoint load with an SQL error count, returned a failure exit code for unsuccessful server startup, added bounded Windows EXE-lock retries during install, and ignored SQLite outbox files. Regression tests cover restart/duplicate metadata and occupied-port startup. See the final section of `docs/AUDIT.ru.md`.

The reviewed executable has been installed with `scripts/install-local.ps1 -AutoStart`, with another verified pre-update backup. Collection resumed at port 4242 with zero backlog and the same three legacy format errors. No source rollouts were modified and no new database migration was needed. Changes are prepared for the existing `main` branch and `origin`; inspect Git to determine final commit/push/CI state rather than treating earlier checkpoints as current.