# Capture capabilities and acceptance status

This is the canonical support matrix for retrospective evidence (schema 7,
observation/package format 1). A platform CI pass is **not** proof of complete
capture from a real Codex GUI. No retrospective analyzer skill is shipped here.

## Platform matrix

| Agent / platform | Evidence implemented | Validation and recommendation |
| --- | --- | --- |
| Codex GUI, macOS | Local JSONL/gzip history, selected SQLite history, supported hooks, OTLP/HTTP JSON, user LaunchAgent | Primary target. Real GUI acceptance **pending**; do not advertise complete coverage yet. |
| Codex CLI, macOS/Linux/Windows | Same local observation channels plus existing CLI wrapper | Synthetic contract/integration tests; real end-to-end deep capture acceptance **pending**. |
| Codex GUI, Windows | Native history and the same local capture endpoints | Previous metadata collector exercised against real local history. New content pipeline tested with synthetic fixtures, not a week of production use. |
| Codex GUI engine 0.153.4; separately installed CLI 0.144.1 | Shapes inspected during planning | Inspection is not acceptance. GUI engine version comes from source metadata; it is not inferred from the CLI on PATH. Neither executable migrates Codex storage. |
| Other Codex versions | Unknown records can be retained after sanitization | Unsupported schemas/boundaries produce diagnostics; no blanket compatibility promise. |
| Copilot, OpenCode, other agents | Existing wrappers and `/api/v1/events` | Process/config/Git telemetry as documented. No equivalent deep native-history claim. |
| Remote executions | Collection on the execution host; portable package transfer | Desktop history is not assumed to contain everything from a remote host. No automatic machine synchronization. |

## What is collected

- Sanitized original-shaped JSON records: available user/developer/base instructions,
  messages, tool calls/results, context events, compaction/replacement history,
  `world_state`, native usage records, unknown record types. Original source files
  are never rewritten. Source content collection is explicit and can be enabled
  with replay from the Sources panel.
- Provenance includes channel, source, position, engine version when exposed,
  native session/turn/item identifiers, source timestamp and receipt time,
  sanitization disposition/reason and format versions. Missing identifiers stay
  missing; a capture timestamp is not a model-request timestamp.
- Legacy `response_item` and paginated `item_completed` tool representations use
  native call IDs for the existing statistics. Supplementary SQLite/hooks/OTel
  observations do not increment these counters. Their archive entries remain
  separate evidence, not independently counted turns.
- SQLite: explicit `.sqlite` source registration reads only known
  `thread_items` columns (`rowid`, `thread_id`, `turn_id`, `item_id`,
  `updated_at_ordinal`, `item_json`). No copying of Codex databases, auth tables,
  enrollment tables or migrations. Periodic bounded cycles discover updates.
- Hook stdin is sanitized before writing the offline queue. The command emits
  no stdout instructions and exits successfully even on capture failure, with a
  generic diagnostic on stderr. The server drains the queue independently.
- OTel: `/v1/logs` and `/v1/traces` accept **OTLP JSON**, up to 2 MiB / 500 records
  per HTTP batch. Configure `protocol = "json"`. Protobuf, gRPC, compressed HTTP
  bodies and metrics ingestion are not supported by these endpoints.
- Workflow: only explicitly registered roots and a narrow allowlist of workflow
  files. Includes AGENTS/CLAUDE/GEMINI/WORKFLOW/SKILL markdown, skill/instruction
  directories, selected TOML config, package scripts. Changes create versions;
  repeated identical content does not. State is `discovered`; this does not prove
  loading or invocation. Native observations can separately supply that evidence.
- Task trackers: opt-in local mappings select an exact MCP tool and task ID in
  arguments or decoded results using JSON Pointers. A matching native call ID and
  session link a response version to a namespaced task. There are no built-in
  providers, credentials or background polling. Unrecognized shapes remain in the
  archive; no task freshness is invented. See [task adapters](TASK_ADAPTERS.md).
- Human outcomes: accepted, rework, repeated mistake, misunderstanding, successful
  approach. Multiple sessions/turns can be linked to one task. Arbitrary task-ID
  mentions are not automatically confirmed. Git/test/CI/review information is
  available when observed in tool evidence or existing normalized events, not
  fetched from GitHub independently.

## Privacy boundary

Known secret fields, assignments, authorization headers and common credential
formats are scrubbed **before persistence**. Secret-file references (`.env`, auth
stores, SSH/private keys), environment maps, encrypted and opaque binary content
cause exclusion of that content. Exclusion metadata remains. A known secret-file
tool call also excludes its associated JSONL result. Unpaired SQLite results and
OTel tool-result snippets are excluded because their inputs cannot be verified.

This is conservative, best-effort detection, **not a proof that arbitrary text
contains no secrets**. Unknown encodings, user-defined credential formats,
indirect/delayed tool output and upstream transformations can defeat a detector.
Keep secrets out of prompts and agent outputs. Existing Codex source files are
outside Harnesscope's control. Some safe content may be excluded as a result of
the conservative policy. No unrestricted raw-content mode is offered.

## Storage and reliability

- Migration from v5/v6 first makes a verified backup. Existing events/reviews remain;
  v5 native checkpoints reset to backfill the archive from available originals.
  v6 → v7 adds local task mappings without resetting collectors or removing tasks.
- SQLite indexes small objects inline. Objects above 64 KiB use SHA-256 filenames
  in `<database>.objects`; durable publication precedes database references and
  checkpoint commit. Rollbacks may leave unreferenced sanitized files. Reads and
  backups verify object hashes. No automatic history deletion.
- Storage follows `HARNESSCOPE_DB_PATH`; choose its volume before collection.
  There is currently no independent UI archive-directory relocation operation.
- JSONL is streamed without a 256 MiB whole-file cap; 32 MiB per record remains.
  Batches are bounded by 200 records / roughly 8 MiB (one larger record allowed).
  gzip JSONL is supported; other compression formats are not. Prefix validation
  and gzip resume can reread earlier bytes, so very large active files cost more
  I/O. This is not a constant-time seek guarantee.
- Low space pauses object/queue writes with a 128 MiB reserve; database failures
  retain native checkpoints or queued hook observations. This is not a global
  disk quota or a promise that every in-flight hook can be captured on a full disk.
- Backups inline referenced external objects into a portable SQLite snapshot.
  Restore verifies them, targets a new path, and pauses native sources. Pending
  wrapper outbox and hook queue are separate and are not included in backups.
- Export reads a separate SQLite snapshot, writes JSONL incrementally, and fixes
  selection/cutoff/format versions. Workflow/task context travels with the package.
  Import sanitizes again, is retryable, and never enables imported workflow roots.
  A malformed package can leave its already accepted prefix imported; retry is
  idempotent. Existing task identities/links are not overwritten by import.

## Explicit gaps and release gate

Not exposed: hidden system prompts, private internal reasoning, complete exact
model requests, reliable per-role token counts, and data already truncated by
Codex. Cumulative usage is never labeled as current active context. Unknown and
estimated values must not be converted to zero. History that no longer exists
cannot be recovered by backfilling.

Still requires follow-up before declaring the entire design accepted:

1. Real macOS GUI and CLI scenario: clarification, error/fix, compaction, resume,
   fork, subagents; collect engine/OS version and a sanitized fixture per scenario.
2. Three real legacy Windows fork files now reach READY (200, 241 and 238 lines).
   Verify the equivalent macOS formats: explicit fork-parent metadata
   plus native started_at separates the inherited prefix from the child.
   Missing or ambiguous boundary markers must not be treated as complete coverage.
3. Validate each connected task tracker's MCP shapes and task/comment revisions.
   Synthetic adapter tests do not certify a private provider's real responses.
4. Workflow snapshots are polling based; missing/disallowed files create exclusion versions; changes shorter than
   the polling interval can still be missed. Exact instruction
   loading and invocation must be established from native evidence.
5. Richer code attribution and approval/request views remain future work.
   Context pages expose observed window/usage and unavailable active occupancy;
   native parent relations are retained. Broader SQLite tables are not read.
6. Stress/fault injection for multi-gigabyte active rollouts, process termination
   during object publication, low-disk behavior, and all new platform installers.
7. Reconstruct several actual tasks across a week and account for each gap.

The future analyzer can use the documented API/MCP now for development, but the
**production skill-readiness gate remains open** until these checks pass.

Official contracts: [Codex hooks](https://learn.chatgpt.com/docs/hooks),
[telemetry](https://learn.chatgpt.com/docs/agent-approvals-security#monitoring-and-telemetry),
[configuration](https://learn.chatgpt.com/docs/config-file/config-reference).
