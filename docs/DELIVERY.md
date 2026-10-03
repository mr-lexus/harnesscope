# Durable delivery and process observations

Wrappers write redacted events to `<HARNESSCOPE_DB_PATH>.outbox.sqlite3` before attempting HTTP delivery. The separate SQLite queue uses WAL and FULL synchronous writes. Starting the child does not wait for server startup or network delivery; inherited stdin/stdout/stderr and the child's exit code remain intact. The wrapper performs bounded delivery work before it exits; unavailable telemetry does not change the agent's result.

Queue connections allow up to five seconds for SQLite writer contention. Concurrent first-time initialization also retries BUSY/LOCKED failures with a fresh connection, including WAL setup failures that can bypass SQLite's busy handler. Exhausted waits and other storage errors remain explicit on stderr; they do not change the child exit code. Existing queued events remain on disk.

## Recovery

Use the same `HARNESSCOPE_DB_PATH` and loopback server address for wrappers and `serve`. The server drains pending batches every second, including batches left by a wrapper that already exited or crashed. It also continues native source collection independently. Graceful shutdown joins both workers. No machine-wide service, scheduled task or OS autorun is installed.

- Destination is the normalized HTTP origin. `localhost` and `127.0.0.1` share a queue destination; other loopback addresses and ports remain separate.
- The wrapper's background worker can deliver its own stream via `POST /api/v1/delivery`. A server using a different database cannot read the local queue automatically; use the original configuration or `outbox flush` for recovery.
- Ordered batches within a runtime cannot overtake one another. Independent runtime streams can progress while one fails.
- A 30-second claim lease prevents simultaneous consumers from acknowledging each other's claims. After a crash the lease expires. Server commit and outbox acknowledgement are separate: a lost acknowledgement causes replay. Normal events deduplicate by event ID; heartbeat projection is a monotonic upsert.
- `/delivery` commits the entire batch or rolls it back and returns the exact ordered list of acknowledged event IDs. A successful HTTP status alone never deletes a batch. The existing `/events` import API retains its documented per-event results.
- Retry delays increase from 2 seconds to 60 seconds and persist across restarts. HTTP 400/413/422 or invalid event relationships block that stream until an explicit retry; transient failures remain queued. “Retry now” clears delays/blocks, but never cancels an active claim or deletes events.

```sh
harnesscope outbox status
harnesscope outbox retry
harnesscope outbox flush
```

`flush` attempts up to 100 available batches and exits nonzero if any remain (including leased, delayed or blocked batches). `retry` resets delays first when an immediate reattempt is wanted. Commands operate on the configured database path and server URL; `status` is usable while the server is offline. Pending telemetry metadata is also visible in **Monitor**.

For intentionally offline runs set `HARNESSCOPE_AUTOSTART=0`. Events are still queued. The setting suppresses automatic startup, not collection or delivery to an already running server.

## Heartbeat semantics

While `try_wait` confirms that its direct child has not exited, the wrapper records `runtime.heartbeat` every 15 seconds, tied to a new UUID per runtime rather than a reusable PID. The latest observation is stored in schema v5's `runtime_observations`; heartbeats do not fill the raw event timeline. The initial observation is recorded after a successful spawn.

Monitor separates process result from observation freshness:

| State | Meaning |
| --- | --- |
| FRESH | Last observation at most 45 seconds old |
| STALE | No observation within 45 seconds; wrapper, server or machine may have stopped |
| UNMONITORED | No heartbeat evidence, or no running process status |
| CLOCK SKEW | Observation more than 5 seconds in the future; inspect system clocks |
| ENDED | A recorded terminal timestamp exists |

Freshness uses **observation time**, not arrival time. Replaying an old offline heartbeat does not make a process fresh. Older observations do not overwrite newer ones. Late heartbeats do not reopen a stopped runtime. Missing heartbeat does not change an execution into FAILED or COMPLETED. Native transcript observers are excluded; their file collection health is on Sources. A GUI launcher only represents that launcher, not a detached editor process. Monitor shows up to 200 runtimes, unfinished first.

## Bounds and limitations

The queue has a 256 MiB payload budget across destinations, at most 1 MiB per event, and at most 100 events / 1 MiB per batch. Existing events are never evicted to make space. Full disk, unavailable permissions, oversized events or exhausted queue budget produce a visible stderr warning while the child continues; events that could not be committed cannot be recovered. Telemetry collection is not an agent restart supervisor. Abrupt termination before a write may leave incomplete observations.

SQLite can retain allocated pages after delivery; the payload budget is not a hard cap on database/WAL file size. Redaction is best effort, not a guarantee of removing every sensitive value. Do not publish queue files or raw databases. Large, multi-day offline workloads have not been load-tested.

## Panel workflow

- Header: active navigation, local connection state, pause/resume UI polling. Collection continues while polling is paused. Panel settings select compact/comfortable density and load optional demo records.
- Executions: main filters stay visible; advanced filters expand on demand. Filters and page are in the URL. Desktop tables and mobile cards link to the same records. The detail back button restores execution filters.
- Details: primary facts, expandable token details, review, components, Git, agents and searchable event payloads. Payloads start collapsed; draft reviews survive polling and tab switches.
- Sessions: any runner name, URL filters, pagination, mobile cards, accessible links and process/turn scope labels.
- Sources: add-source drawer, path filter, compact status, expandable checkpoints. Monitor: process freshness and durable queue diagnostics.
- Overview: cohort comparisons become cards on narrow screens. Export continues to preserve selected report filters.


Outbox payload size is maintained transactionally by SQLite triggers. Existing queues are backfilled once on opening; inserts, acknowledgements and rollbacks keep the 256 MiB payload budget exact without rereading all payloads on each enqueue. File size can exceed this payload budget because of SQLite/WAL overhead and retained free pages. See [the reproducible load results](PERFORMANCE.ru.md).

Main-database backups do not include queued delivery. Before a full migration, drain every destination and preserve the original database/outbox until the restored history has been verified; see [backup and recovery](BACKUPS.md).
