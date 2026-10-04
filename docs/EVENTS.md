# Integrating any agent

[Documentation](../README.md#documentation)

Harnesscope supports process wrappers, normalized adapter events and a [native Codex rollout collector](NATIVE_SOURCES.md). The original two ingestion paths remain:

1. `harnesscope run --runner <name> <command> [args...]` observes a **process** with inherited terminal streams, its exit code and before/after Git state. It does not observe each prompt inside an interactive agent or an editor that detaches from its launcher.
2. An adapter sends normalized events to `POST /api/v1/events`, or writes JSON/JSONL for `harnesscope ingest --file events.jsonl`. This supports real turn boundaries and proven tool invocations without coupling the database to a particular agent vendor.

The importer accepts a single JSON object, an array, or JSONL, including a UTF-8 BOM. It reads stdin when `--file` is omitted. Input is limited to 2 MiB and 500 events per request/import. Native vendor logs must first be converted to this envelope; the importer is not a parser for arbitrary vendor transcripts.

## Try it

```sh
harnesscope ingest --file examples/events.jsonl
harnesscope ui
```

Open an execution, assess its outcome, give related runs the same experiment label, then compare them in Overview. The example identifies itself as `example-agent`; these imported records are real imported test records, not the built-in demo dataset. Use a temporary `HARNESSCOPE_DB_PATH` when trying examples separately from your own history.

## Event contract

```json
{
  "event_id": "my-adapter:conversation-123:turn-2:start",
  "timestamp": "2026-10-03T10:00:00Z",
  "event_type": "execution.started",
  "source": "my-adapter/v1",
  "runtime_id": "my-adapter:process-456",
  "session_id": "my-adapter:conversation-123",
  "execution_id": "my-adapter:conversation-123:turn-2",
  "payload": {
    "capture_scope": "TURN",
    "model": "UNKNOWN",
    "prompt_summary": "Add a regression test"
  }
}
```

- `event_id`, `event_type` and `source`: nonempty, at most 256 bytes. IDs share a database-wide namespace: prefix them with your adapter and account/source namespace when applicable. The first accepted event with an ID wins. Reusing it acknowledges a retry without applying side effects again; it does not update the original event.
- `timestamp`: RFC3339; normalized to UTC when stored. Omission uses ingestion time, which is suitable for live hooks but not historical logs. Historical adapters should always supply the original timestamp.
- `payload`: a JSON object. Unknown event types are retained as timeline events without invented projections. Unrecognized envelope fields are tolerated, but custom durable metadata belongs in `payload`.
- Start events require the relevant entity ID: `runtime_id`, `session_id` or `execution_id`. Completion events require the same ID as the corresponding start. Send parent/start events before dependent/completion events, including across requests.
- `capture_scope`: `PROCESS`, `TURN` or `UNKNOWN`. Do not label a CLI lifetime as a turn. Existing records migrated from older builds remain `UNKNOWN`. `DEMO` is reserved for built-in demo data.
- `model`: supply observed/requested model metadata, or `UNKNOWN`. A requested model does not prove the provider actually used it or that it remained unchanged during a process.
- An absent native conversation ID must remain `UNKNOWN`. Never derive identity from a working directory, branch, time proximity or a display name.

Every event's projection changes and timeline entry commit in **one transaction**. Failed events roll back completely. A batch is validated before processing, then applied **event by event**, not as one transaction. On a processing error the API returns HTTP 422, `event_ids` for successful events and `errors` for failed ones. HTTP 202 means every event was processed or was an already accepted retry. Replaying a batch is safe with stable IDs. Wrappers keep the child process running independently of delivery, persist redacted events in a durable outbox and use the atomic `/delivery` endpoint described below. Direct `/events` callers are responsible for retaining and retrying failed imports.

HTTP callers should always supply stable event IDs. If a file import omits an ID, the importer hashes the normalized JSON record to generate one; two identical records therefore represent one event. HTTP requests that omit IDs get random IDs and cannot be deduplicated across independent retries.

## Supported projections

| Event | Payload highlights | Meaning |
| --- | --- | --- |
| `runtime.started` | `runner_name`, `runner_version`, `surface`, `pid`, `cwd`, `command_line` | OS process observation |
| `session.identified`, `session.started` | `runner_name`, `native_session_id`, `title`, optional `parent_session_id` | Bind or resume a native conversation |
| `execution.started` | `capture_scope`, `model`, `reasoning_effort`, `prompt_summary`, Git context | Start a process observation or actual turn |
| `execution.completed` | `exit_code`, `status`, optional `duration_ms`, `error_message` | End an execution; absent duration is derived from timestamps |
| `runtime.stopped` | `exit_code`, `status` | Stop a process; incomplete executions become `UNKNOWN`, not successful |
| `agent.started`, `subagent.started` | `parent_agent_id`, `agent_role`, `agent_name`, `model` | Agent observed inside an execution |
| `component.discovered`, `mcp.discovered`, `skill.discovered`, `plugin.discovered` | `name`, `component_type`, `state`, `version` | Configuration/discovery evidence |
| `component.invoked`, `mcp.invoked`, `skill.invoked`, `plugin.invoked` | `name`, `invocations_count`, `details` | Proven invocations; count is a nonnegative **delta** |
| `git.snapshot` | `snapshot_type`, `repo_root`, `worktree_path`, `changed_files`, `diff_stat` | Observed Git state; server preserves ambiguous attribution |
| `config.snapshot` | `config_type`, `raw_content`, `parsed_json` | Optional redacted config evidence |

Native session identity is scoped by `runner_name` and `native_session_id`. When a resumed wrapper supplies a fresh local session ID, a persisted alias routes its subsequent events into the original conversation. An unimported parent claim is kept in the raw event without creating a broken foreign key; importing that parent later does not automatically reconstruct the missing lineage.

## Human review and reports

- `GET /api/v1/executions/:id/review` returns the saved review or `null`; missing executions return 404.
- `PUT /api/v1/executions/:id/review` accepts `{"outcome":"ACCEPTED","notes":"...","experiment":"tests-first-v2"}`. Outcomes are `UNREVIEWED`, `ACCEPTED`, `REWORK`, `REJECTED`. Notes allow 10,000 UTF-8 bytes, experiment labels 120 bytes. Saving replaces that execution's current review.
- `GET /api/v1/retrospective?days=30&runner=codex&scope=PROCESS` returns filtered evidence, review coverage, cohorts and a review queue. Optional `project` is the exact repository path; `include_demo` defaults to `false`. `days` is clamped to 1–3650.
- Overview exports the selected report as JSON, with filters, export time and schema version. This is an aggregate report, **not a complete database backup**. Acceptance uses reviewed executions as its denominator; unknown/unreviewed outcomes are never counted as failures or successes.

## Local security and privacy

The server only binds to loopback. Browser calls require the same HTTP origin; arbitrary origins, cross-site requests and non-loopback Host headers are rejected. No wildcard CORS. CLI ingestion bypasses proxies and does not follow redirects. This is a single-user local application, not an authenticated service for untrusted local users or remote teams.

Secret redaction walks JSON and recognizes common credential fields, provider tokens, CLI assignments, authorization tokens and private key blocks. It also runs before fields such as session titles and review notes are stored. It is a best-effort filter, not a guarantee against every arbitrary secret or personal detail. Avoid secrets in prompts, identifiers and adapter payloads. Review exports before sharing them.

Wrappers do not persist full configuration contents unless `HARNESSCOPE_CAPTURE_CONFIG=1` is set. They can still discover declared component names. Generic wrappers only accept portable metadata via `HARNESSCOPE_SESSION_ID`, `HARNESSCOPE_PARENT_SESSION_ID` and `HARNESSCOPE_MODEL`; they do not assign meanings to another CLI's `-s` or `-m` options.


Native collection persists checkpoints atomically with event batches. Wrappers separately use a durable outbox and heartbeat. See [native sources](NATIVE_SOURCES.md) and [delivery/recovery](DELIVERY.md).


## Atomic delivery and process observations

`POST /api/v1/delivery` accepts an array of 1–500 normalized events (2 MiB HTTP limit), commits the entire batch, and returns `{"event_ids":["id-1","id-2"]}` in input order. Validation failures return 400/422; transaction failures return 503 without acknowledging the batch. Consumers retry the same IDs. This differs intentionally from `/events`, which reports individual import results.

`runtime.heartbeat` requires an existing `runtime_id` and an RFC3339 timestamp representing when the direct child was observed. Unlike ordinary timeline events, heartbeats retain only the latest timestamp in `runtime_observations`; they are replay-safe by monotonic timestamp, not a raw-event history. They cannot reopen a stopped runtime or infer a task outcome. Keep heartbeat IDs in a separate unique namespace from other events.

`GET /api/v1/monitoring` reports up to 200 process observations plus aggregate outbox counters and at most 100 pending batch diagnostics. `POST /api/v1/outbox/retry` resets retry delays and validation blocks for the configured destination, without deleting events or revoking leases. These APIs use the same loopback/origin restrictions as the rest of the server.


## Bounded event history

Execution and session detail responses expose `events_total` instead of embedding the complete `events` array. Clients using the earlier development API must request the journal separately:

- `GET /api/v1/executions/:id/events?limit=50&before_id=123&search=tool`
- `GET /api/v1/sessions/:id/events?limit=50&before_id=123&search=tool`

Response: `{"items":[...],"total":1234,"next_cursor":73}`. `total` counts all events matching the search, independent of the current page. `limit` defaults to 50 and is clamped to 1–100. `search` is a literal substring in event type or source (not payload), at most 200 UTF-8 bytes. Missing owners return 404; invalid cursors return 400.

Items use descending database arrival IDs, not event timestamps: late deliveries cannot shift older pages. Omit `before_id` for the latest page; use `next_cursor` for older arrivals; `null` means the end. New arrivals appear when the latest page refreshes. There is no cross-page snapshot guarantee for `total` while writers append data. Page items and their total are read in one transaction.

The panel fetches this history only on the Events tab, keeps at most one page in the DOM, and polls only the latest page. Hidden tabs and historical pages do not poll the journal. Details and history queries run on blocking workers rather than occupying asynchronous HTTP workers.
