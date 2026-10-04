# Native Codex sources

[Documentation](../README.md#documentation)

Harnesscope incrementally reads Codex history, adding native turn boundaries, per-turn model metadata, tool invocations, cancellation/failure outcomes and observed token counters. It does not run Codex or call a model API. The main walkthrough below uses rollout JSONL; gzip and selected SQLite history support are documented in the [capability matrix](CAPABILITIES.md).

## Connect a source

Open **Sources** in the UI and enter an absolute path to one rollout or a directory. Directory discovery includes `rollout-*.jsonl` recursively. A usual location is the `sessions` directory under your Codex home; choose the actual path on your computer. `~` is not expanded by the UI.

CLI alternative:

```sh
harnesscope sources add-codex --path /absolute/path/to/codex/sessions
harnesscope serve
```

On Windows, for example:

```powershell
harnesscope sources add-codex --path "C:\Users\YOUR_USER\.codex\sessions"
harnesscope serve
```

The CLI `sources` commands operate on the **local configured database**, not on `HARNESSCOPE_SERVER_URL`. Use the same `HARNESSCOPE_DB_PATH` as the server. The UI always manages the database of its connected server.

Registration imports existing history as well as newly appended records. It does not automatically register or scan any other home directory. Register archived rollouts separately if needed. Overlapping sources and copied rollouts replay stable event IDs; they do not duplicate executions or tool invocation counts.

```sh
harnesscope sources list
harnesscope sources pause SOURCE_ID
harnesscope sources resume SOURCE_ID
harnesscope sources remove SOURCE_ID
```

Disconnect/remove deletes registration and checkpoints; previously imported telemetry and reviews remain. Re-adding the source replays it safely. It does not revise events already accepted under their stable IDs.

For a bounded one-off scan without an HTTP server:

```sh
harnesscope sources scan
```

Repeat while files show `BACKLOG`. A pass handles at most 200 lines per visited file and 10 changed/error files per source. It exits nonzero if enabled sources have errors; other files can still have imported successfully. This is incremental progress, not an all-or-nothing full-directory import.

## Collection lifecycle

- `serve` owns one collector task. It scans immediately, then five seconds after the preceding pass finishes. While checkpoints report backlog, the interval is 250 ms to catch up without an artificial five-second delay per batch. The optional Windows local installer creates a user login shortcut; no machine-wide service is installed.
- File offsets, parser state, normalized events and projections commit in **one SQLite transaction per batch**. SQL failure rolls back the batch and checkpoint together.
- On restart, the collector restores the active turn and cumulative token baseline. It verifies the already-consumed prefix when a file changes, then continues at the saved byte offset.
- A second collector can race safely: the checkpoint uses a compare-and-swap offset inside the transaction. A stale batch rolls back and retries on a later scan.
- Pausing stops before another file batch starts; an in-flight batch can finish. Shutdown signals the collector, cancels its idle wait, and joins its current bounded work before exiting.
- Source files serve as the durable input queue. If Codex deletes/rotates files before Harnesscope reads them, missing records cannot be recovered from nowhere. Keep the original rollouts until import catches up.

Sources shows last scan/success, file checkpoints, backlog, errors and the count of unmapped records. A scan older than 30 seconds is labelled overdue, not automatically failed. **Source freshness is not agent/process liveness.**

| File status | Meaning |
| --- | --- |
| READY | Consumed all complete lines in the observed file |
| BACKLOG | More complete records may remain; another bounded pass is needed |
| WAITING | Final line lacks its newline; leave it untouched until the writer finishes |
| ERROR | Malformed/unsupported input, changed prefix, truncation, access failure or projection failure; checkpoint retained |

An unfinished UTF-8 character in the final line is safe: no decoding or offset advance happens until the line ends. A final record without a newline remains `WAITING`, even if its JSON happens to parse. Malformed complete lines are never silently skipped. Fix the input or disconnect it; other files continue importing.

The collector checks full consumed-prefix SHA-256 when file size/mtime changes. Unchanged size/mtime is an optimization, not protection against malicious filesystem tampering. It refuses truncation or prefix changes; it does not overwrite historical telemetry. Restore the original file or inspect a separate corrected copy. A corrected copy still cannot replace events that already have accepted IDs.

Limits: 32 MiB per JSONL line; no 256 MiB whole-file cap. Batches contain at most 200 records / roughly 8 MiB (one larger record allowed). Tree limits remain 10,000 rollouts / 100,000 entries. Child symlinks and Windows reparse points are skipped. JSONL and gzip JSONL are supported. Verified fork-parent prefixes are preserved without charging inherited turns to the child; ambiguous boundaries remain limited. See [CAPABILITIES.md](CAPABILITIES.md) for the authoritative support matrix.

## Privacy

Default collection is **metadata only**: thread and turn IDs, timestamps, initial cwd, model/effort, tool names/call IDs, outcomes and token counters. Raw user/assistant messages, instructions, reasoning text, tool arguments and tool outputs are not copied.

The **Collect full sanitized evidence** setting (`--include-content`) now archives available messages, instructions, arguments, results and context separately from legacy summaries. Secret-file/opaque content is excluded with a reason. In Sources, changing policy resets the checkpoint to replay available originals; existing events/reviews remain unchanged. Metadata can still be sensitive. Read the privacy limitations in [CAPABILITIES.md](CAPABILITIES.md) before enabling content.

## Evidence and format support

The adapter is `codex-rollout/v1`, with synthetic protocol fixtures in `tests/fixtures/rollout-codex.jsonl`. Its supported shapes were checked against the official Codex source types through Context7 on 2026-10-03: `RolloutItem`, `SessionMeta`, `TurnContextItem`, `TurnStartedEvent`, `TurnCompleteEvent`, `TokenUsage`. The rollout format is an internal evolving format, not a version-stable integration API. A supported event shape matters more than the CLI version string.

- `session_meta.payload.id` identifies this thread. In newer files `session_id` can identify the root thread instead; it is not substituted for `id`.
- Repeated metadata for the same thread is idempotent and preserves usage baselines. A different thread identity within one rollout remains an explicit format error.
- `event_msg/task_started` (also `turn_started`) requires a native `turn_id`; it starts a `TURN` observation. `task_complete` / `turn_complete` ends it; explicit terminal errors mean FAILED and `turn_aborted` means CANCELLED. EOF never proves success, failure or process death. Completion without an observed start is counted as unmapped instead of inventing a task.
- `turn_context` supplies per-turn model and effort; it can enrich a turn after its start. This remains recorded metadata, not proof of which provider model actually served every response.
- `response_item` function/custom/local-shell calls count once per native call ID within the turn. Both a direct item and an `item` envelope are supported. A tool result is not a second invocation. `mcp__`-prefixed tool names are displayed as MCP components.
- `event_msg/token_count.info.total_token_usage` is cumulative. The adapter subtracts the prior session sample and adds nonnegative deltas to the current turn. Repeated samples do not double-count. A fork/history-base sample first establishes a baseline so inherited history is not billed to the child. Counter decreases establish a new baseline and are counted as unmapped; unknown usage is not guessed.
- Token coverage can be partial. Cache tokens are included in input; reasoning tokens are included in output. Values are not prices, subscription consumption or cost estimates. Modern `token_usage_record.turn_token_usage` supplies cumulative per-turn counters and takes precedence for that turn. Legacy counters are then tracked only as a baseline, never added again. A final modern sample may update the last completed turn. Records for an unknown turn or a different thread stay unmapped.
- Unknown record/event types count as unmapped. Known message/output/compaction/world-state records and duplicate `item_completed` presentation records that are deliberately omitted do not. Unsupported required shapes produce a visible file error.
- A transcript uses a synthetic `surface=transcript` observer with UNKNOWN process status and no PID. It is not counted as a live runtime and does not create live-worktree/concurrent-runtime conflicts. Wrapper `PROCESS` observations and native `TURN` observations remain separate; filter by scope when comparing experiments.
- Native identity can bind to an already-known wrapper conversation. A known fork parent is linked; importing the parent later does not reconstruct lineage automatically. Inherited-history coverage depends on the format and verified boundaries; rollback/retraction events are not reconstructed. Review unmapped counts and the capability matrix before drawing conclusions.
- Historical imports do not run Git against today's checkout or attribute current changes to old turns. Overview's project filter falls back to the observer's **initial cwd** when no recorded repository root exists; subsequent cwd changes are only retained in turn context evidence.

## API additions

- `GET /api/v1/collection`: lightweight aggregate collection status, enabled source count, inspected/backlogged/error file counts, converted event count, last scan, database path and application version. A detected Codex folder is only a setup suggestion; this endpoint does not register it. States are `NOT_CONNECTED`, `PAUSED`, `STARTING`, `CATCHING_UP`, `COLLECTING`, `OVERDUE` and `NEEDS_ATTENTION`.
- `GET /api/v1/sources`: registrations and file diagnostics; parser state is excluded.
- `POST /api/v1/sources`: `{ "path": "/absolute/path", "include_content": false }`.
- `PATCH /api/v1/sources/:id`: `{ "enabled": false }` to pause, `true` to resume.
- `DELETE /api/v1/sources/:id`: disconnect; imported telemetry retained.
- Execution details include nullable `usage` with input, cached input, output, reasoning output and total token counters.
- Normalized adapters may emit `execution.context` to enrich model/effort/summary and `usage.observed` with **cumulative per-execution**, nonnegative token counters. These require an existing execution ID. Usage snapshots replace earlier snapshots; older timestamps do not overwrite newer observations. They are not additive deltas.

## Next lifecycle work

Process wrappers now use a separate durable outbox and heartbeat protocol; see [delivery and monitoring](DELIVERY.md). Native files provide restartable history collection, while Monitor reports wrapper observations. The Windows local installer supports explicit `-AutoStart` for user login startup and installs Start-menu open/stop controls. See [daily use](RUNNING.md).
