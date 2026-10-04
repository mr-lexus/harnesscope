# Harnesscope

[Русская версия](README.ru.md)

> **A local retrospective workspace for AI coding workflows.**

**Retrospective evidence:** the Evidence panel adds sanitized observations, task
timelines, human outcomes, workflow versions/diffs, portable exports and read-only
MCP. Codex hooks and local OTLP/HTTP JSON complement native history. See the
[setup/API guide](docs/EVIDENCE.md) and [capability matrix](docs/CAPABILITIES.md).
Codex GUI on macOS is the primary target; real GUI acceptance is still pending.

Harnesscope observes agent process runs, stores evidence locally in SQLite, and helps you review outcomes and compare workflow experiments. Use the built-in wrappers for Codex, Copilot and OpenCode, `run` for any executable, or the normalized event API/JSONL importer for custom adapters. No LLM calls are made by Harnesscope.

**Observation is explicit:** a wrapper records a process lifetime (`PROCESS`), not each turn inside it. `TURN` records require an adapter with actual turn boundaries. A successful exit is separate from a human-accepted result. GUI launchers may detach before their editor session ends.

---

## Installation

### Homebrew (macOS & Linux)
```bash
# macOS: prebuilt binary, no Xcode upgrade or local compilation
brew install --cask mr-lexus/tap/harnesscope
harnesscope server start

# Linux (or the formula on macOS with compatible developer tools)
brew install --formula mr-lexus/tap/harnesscope
brew services start mr-lexus/tap/harnesscope
```

Choose one method. See [migration, updates and published versions](docs/HOMEBREW.md).

### Direct Script Install
```bash
# macOS & Linux:
curl -fsSL https://raw.githubusercontent.com/mr-lexus/harnesscope/main/install.sh | bash

# Windows (PowerShell):
irm https://raw.githubusercontent.com/mr-lexus/harnesscope/main/install.ps1 | iex
```

### From Source
```bash
cd web && npm ci && npm run build && cd ..
cargo build --release
```

---

## Quick Usage

### 1. Launch Agent Environments
Run your agents directly through Harnesscope. Stdin, stdout, stderr, interactive TTY, and exit codes pass through transparently:

```bash
# Interactive CLI / TUI environments
harnesscope codex
harnesscope copilot
harnesscope opencode

# Any CLI or GUI agent studio / editor
harnesscope run cursor .
harnesscope run code .
harnesscope run --runner aider aider
```

*Note: Whenever you run an agent, Harnesscope automatically spawns the local telemetry server in the background if it is not already running.*

### 2. View Telemetry & Web UI
```bash
# Open Web UI in your default browser (http://127.0.0.1:4242)
harnesscope ui

# Or manage server lifecycle
harnesscope server start
harnesscope server status
harnesscope server stop

# Run health diagnostics
harnesscope doctor
```

### 3. Review and improve

Open **Overview**, choose a period/project/runner, then open an execution and record its outcome, notes and experiment label. Compare similar runs and export the selected report as JSON. Unknown evidence stays visible. Demo records are excluded from Overview by default.

```bash
# Import normalized adapter events (JSON object, array or JSONL)
harnesscope ingest --file events.jsonl
```

See the [event contract](docs/EVENTS.md) and [example](examples/events.jsonl).

### 4. Seed Demo Data (Optional)
```bash
harnesscope demo seed
```

---

## Key Principles

- **Zero LLM Calls**: Harnesscope never invokes an LLM to guess models, sessions, skills, or tools. Everything is extracted strictly from the runtime environment, process arguments, native logs, Git state, and configuration files. If unknown, it is marked as `UNKNOWN`.
- **True Conversation Identity**: Distinguishes **Runtime** (OS process), **Session** (native conversation), and **Execution** (process observation or adapter-reported turn). When a session is resumed across a process restart (`process stopped != session stopped`), Harnesscope preserves conversation continuity.
- **Git Context & Conflict Detection**: Automatically captures Git snapshots before and after execution (commit SHA, branch, dirty status, modified files, diff statistics). If multiple executions run concurrently in the same worktree, their Git attribution is formally flagged as `AMBIGUOUS`.
- **Local & Private**: All data is stored in a local SQLite database (WAL mode). Known credential patterns and structured secret fields are redacted before persistence. This is best effort; avoid putting secrets in telemetry. Full config snapshots require `HARNESSCOPE_CAPTURE_CONFIG=1`. The HTTP server only accepts loopback binding and same-origin browser access.

---

## Task tracker MCP adapters

Task trackers are connected through your agent's MCP configuration. Harnesscope
uses a local, declarative adapter to identify task reads in captured calls and
retain response versions. No tracker provider or credentials are built into this
repository. Start with `examples/task-adapter.json`, then run
`harnesscope task-adapters add /path/to/local-mapping.json`.

## Documentation

- **Current candidate:** [v0.3.0-rc.2 release notes and installation](docs/RELEASE.md)
- **Install and run:** [Homebrew](docs/HOMEBREW.md) · [Daily use](docs/RUNNING.md) · [Platforms and GUI](docs/CROSS_PLATFORM.md)
- **Collection:** [Setup, API and MCP](docs/EVIDENCE.md) · [Codex history](docs/NATIVE_SOURCES.md) · [Capabilities and limits](docs/CAPABILITIES.md)
- **Integrations:** [Agent events](docs/EVENTS.md) · [Task tracker adapters](docs/TASK_ADAPTERS.md)
- **Reliability:** [Delivery and monitoring](docs/DELIVERY.md) · [Backups](docs/BACKUPS.md) · [Performance](docs/PERFORMANCE.md) · [Acceptance](docs/ACCEPTANCE.md)
- **Development:** [Architecture](docs/ARCHITECTURE.md) · [Developer guide](docs/DEVELOPMENT.md) · [Product direction](docs/PRODUCT.md) · [Audit history](docs/AUDIT.md)

## Native Codex collection

Connect a chosen rollout file or directory from **Sources**. The collector inside `serve` imports real turns, tool calls and observed token counters, with transactional checkpoints and restart recovery. Metadata only by default; pause, file diagnostics and disconnect are available in the UI.

```sh
harnesscope sources add-codex --path /absolute/path/to/codex/sessions
harnesscope serve
```

See [setup, lifecycle and format support](docs/NATIVE_SOURCES.md). Select `TURN` in Overview to compare native turns separately from wrapper process observations.


### Durable delivery and monitoring

Wrappers persist redacted telemetry before delivery and report child-process observations every 15 seconds. The server recovers pending batches after restart. The compact responsive panel includes Monitor, URL filters, mobile record cards and display-density settings. See [delivery, heartbeat semantics, recovery commands and limits](docs/DELIVERY.md).


### Backups and large histories

Use `harnesscope backup create --output snapshot.db`, `backup verify --file snapshot.db` and `backup restore --file snapshot.db --output restored.db`. Snapshots include committed WAL records; restore never overwrites an existing path and pauses native collectors. Pending outbox batches, source files and configuration are separate. See [backup and restore](docs/BACKUPS.md).

The event journal loads on demand with cursor pagination and search across the history. Reproducible 10k/100k-event results and remaining scale limits are in [the performance report](docs/PERFORMANCE.md).


For local Windows installation, background collection, login startup and operating limits, see [the daily-use guide](docs/RUNNING.md) and `scripts/install-local.ps1 -AutoStart`.
