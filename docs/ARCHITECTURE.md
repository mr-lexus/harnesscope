# Harnesscope Architecture

Harnesscope is a local, cross-platform telemetry utility for AI coding agents. It non-intrusively wraps agent command-line runners, captures execution and environment metadata deterministically without invoking any external language models, records the telemetry in a local SQLite database, and exposes the data via an embedded Web UI and HTTP API.

---

## 1. Core Data Flow & Runtime Architecture

```
Codex wrapper   ───┐
Copilot wrapper ───┤ HTTP JSON Events
OpenCode wrapper ──┤ (POST /api/v1/events)
Future adapters ───┤
                   ↓
         Harnesscope Server (Axum)
         ├─ Ingestion & Validation
         ├─ Correlation Engine (Session Resume, Attribution)
         ├─ Secret Redaction & SHA-256 Deduplication
         ├─ SQLite (WAL mode, Foreign Keys, Single Writer)
         ├─ HTTP REST API (/api/v1/*)
         └─ Embedded React Web UI (Mantine + TanStack Query)
```

- **Single Writer Principle**: Agent CLI wrappers NEVER write directly to SQLite. They dispatch lightweight HTTP telemetry payloads to the local Harnesscope server.
- **Fail-Safe Design**: If the Harnesscope server is unreachable or fails, the wrapper logs a non-intrusive warning and continues executing the user's agent command. Telemetry failure NEVER breaks the observed agent.
- **Single Server Architecture**: The Axum server listens strictly on `localhost` (default port 4242). It manages transactions, enforces foreign key constraints, and serves both the API and the embedded single-page application (SPA).

---

## 2. Domain Hierarchy

```
RuntimeInstance (Process execution: PID, argv, start/stop)
      │
      │ (RuntimeSessionBinding: START, RESUME)
      ▼
   Session (Native conversation: survives process restart)
      │
      ▼
  Execution (PROCESS observation or adapter-reported TURN)
      ├── AgentInstance (Main agent / Subagents)
      ├── GitSnapshot (BEFORE / AFTER with changed files & diff stat)
      ├── ExecutionComponent (MCP servers, skills, plugins: CONFIGURED vs INVOKED)
      └── Events (Chronological telemetry timeline)
```

### Key Entity Distinctions

1. **RuntimeInstance**:
   Represents a concrete operating system process instance (e.g. an active `codex` command, or an `opencode` TUI process). It tracks process ID, working directory, command line arguments, start time, termination time, and exit code.
2. **Session**:
   Represents the logical conversation or thread between the developer and the agent. **Process stopped != Session stopped**. If an agent process exits and later resumes the conversation with the same native session ID (e.g. `codex resume <SESSION_UUID>` or OpenCode session ID), the existing `Session` entity is reused.
3. **RuntimeSessionBinding**:
   Maintains the explicit association between process instances and logical sessions, annotating each association with reasons (`START`, `RESUME`, `ATTACH`).
4. **Execution**:
   The central unit of analysis: a PROCESS observation or an adapter-reported TURN within a session. A single session contains one or more executions.
5. **AgentInstance**:
   Represents the main agent role (e.g. `coder`, `architect`) or specialized subagents spawned during the execution (e.g. `reviewer`, `test-writer`).
6. **ExecutionComponent**:
   Links an execution to discovered tools, Model Context Protocol (MCP) servers, skills, or plugins, with deterministic state flags: `CONFIGURED`, `DISCOVERED`, `LOADED`, `INVOKED`, or `UNKNOWN`.

---

## 3. Critical Architectural Decisions

### Why Session Survives Process Restart
Developers frequently close an interactive terminal or stop an agent CLI process, only to resume the exact same conversation later. Modeling a session as a 1:1 mapping to an OS process would fragment conversations across multiple disconnected sessions and distort analytics. By binding runtimes to sessions via native IDs, Harnesscope maintains true conversation lifecycle continuity.

### Why Worktree is NOT a Parent Entity
Multiple agents, multiple sessions, and multiple executions can concurrently operate in the same repository worktree, on different branches, or across multiple worktrees. Treating a worktree as the parent entity would falsely merge concurrent runs. In Harnesscope:
- Worktree is purely an **attribute** of the execution context.
- If two executions run concurrently in the same worktree, their Git attribution is formally flagged as `AMBIGUOUS`.
- If an execution runs isolated in a worktree, its Git attribution is flagged as `OBSERVED`.

### Why SQLite is Ideal for Local Telemetry
- **Zero Configuration**: Developers do not need to install or maintain Docker containers, PostgreSQL services, or background daemons.
- **WAL Mode (Write-Ahead Logging)**: Enables concurrent readers and short sequential writes with `PRAGMA synchronous = NORMAL;` and `PRAGMA busy_timeout = 5000;`.
- **Integrity**: Full ACID transactions and relational foreign key constraints prevent orphan records.
- **Portability**: A single database file (`~/.harnesscope/harnesscope.db`) that can be inspected with standard SQLite tools, backed up, or deleted for a clean slate.

### When PostgreSQL Will Be Needed
PostgreSQL is intentionally out of scope for the local MVP. It will become necessary when:
1. Team or centralized deployments aggregate agent telemetry from dozens of remote machines into a shared cloud dashboard.
2. Enterprise role-based access control (RBAC), multi-tenancy, and organization-level compliance auditing are implemented.
3. High-throughput distributed tracing with partitioned event streaming across thousands of agents is required.

The repository layer in `src/storage/` deliberately separates storage access from domain models to enable plugging in alternative storage engines in the future.

---

## 4. Agent Session Identity, Forking, and Conflict Detection

### Native Session Tracking
Harnesscope records native conversation IDs when a supported wrapper can read them from runner arguments or environment variables. It reuses a session only when the runner name and native ID match. If the ID is unavailable, the session remains `UNKNOWN`; Harnesscope does not infer identity from time, branch, or worktree.

### Session Forking & Lineage Tree
Developers frequently branch off existing conversations to explore alternative architectural ideas or test different prompt strategies without destroying earlier turns:
- **Parent-Child Association**: When parent metadata can be linked to a known session, Harnesscope records `parent_session_id`, `fork_reason`, and `forked_at`.
- **Lineage Navigation**: The Web UI links to parent sessions and lists child forks.

### Session-Level Conflict Detection
Git tracking detects uncommitted filesystem dirty states, but **session-level conflict detection** operates at the agent workflow level:
1. **Concurrent Session Access (`CONCURRENT_SESSION_ACCESS`)**:
   Recorded when separate runtimes have active executions in the same logical session.
2. **Fork Divergence (`FORK_DIVERGENCE`)**:
   Recorded when active executions overlap between sessions in the same known parent-child lineage, regardless of which session started first.
3. **Worktree Overlap (`WORKTREE_OVERLAP`)**:
   Recorded when different sessions have active executions in the same worktree. Git attribution for all overlapping executions is marked `AMBIGUOUS`.

---

## 5. Atomic Multi-Wrapper Daemon Lifeline

When multiple wrappers start at the exact same millisecond:
- Each wrapper checks `GET /health` with a fast 200ms timeout.
- If the server is offline, an atomic OS-level file lock (`server-startup-<port>.lock` in the data directory via `create_new(true)`) coordinates startup attempts; the loopback socket bind is the final arbiter.
- Other wrappers wait for the health check with bounded health-check attempts, then attach if the server is ready.
- The lock file is removed after startup coordination so later wrappers can retry if the server exits.


## Retrospective and event integrity

Schema v3 adds persistent session aliases, execution capture scope and human reviews. Correlation deduplication, projection changes and raw event recording share one SQLite transaction. Batch delivery is per-event atomic; see [EVENTS.md](EVENTS.md). Human acceptance is independent from process exit status. Overview groups comparable scopes and excludes demo records by default. Current operational boundaries are listed in [AUDIT.ru.md](AUDIT.ru.md).


### Native source collection (schema v4)

`serve` owns a cancellable collector task. Blocking bounded scans run outside the async worker pool. `telemetry_sources` stores explicit registrations, `source_files` stores metadata-only parser state and byte checkpoints, and `execution_usage` stores per-execution token observations. `CorrelationEngine::process_events_with` commits each native batch and its checkpoint in one transaction. Transcript observers have `surface=transcript` and UNKNOWN process status; they are excluded from live-runtime conflict detection. See [Native sources](NATIVE_SOURCES.md).


### Durable delivery and monitoring

Wrappers persist redacted telemetry before delivery and report child-process observations every 15 seconds. The server recovers pending batches after restart. The compact responsive panel includes Monitor, URL filters, mobile record cards and display-density settings. See [delivery, heartbeat semantics, recovery commands and limits](DELIVERY.md).
