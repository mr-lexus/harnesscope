# Harnesscope

> **Local cross-platform telemetry utility for AI coding agents.**

Harnesscope transparently wraps AI coding agent CLIs and GUIs (Codex, Copilot, OpenCode, Cursor, VS Code, Aider), deterministically captures execution and environmental telemetry without invoking any external language models, stores data locally in SQLite, and displays insights via an embedded Web UI.

---

## Installation

### Homebrew (macOS & Linux)
```bash
brew install mr-lexus/tap/harnesscope

# Optional: run telemetry server automatically in background via system service
brew services start mr-lexus/tap/harnesscope
```

### Direct Script Install
```bash
# macOS & Linux:
curl -fsSL https://raw.githubusercontent.com/mr-lexus/harnesscope/main/install.sh | bash

# Windows (PowerShell):
irm https://raw.githubusercontent.com/mr-lexus/harnesscope/main/install.ps1 | iex
```

### From Source
```bash
cd web && npm install && npm run build && cd ..
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

### 3. Seed Demo Data (Optional)
```bash
harnesscope demo seed
```

---

## Key Principles

- **Zero LLM Calls**: Harnesscope never invokes an LLM to guess models, sessions, skills, or tools. Everything is extracted strictly from the runtime environment, process arguments, native logs, Git state, and configuration files. If unknown, it is marked as `UNKNOWN`.
- **True Conversation Identity**: Distinguishes **Runtime** (OS process), **Session** (native conversation), and **Execution** (single turn/request). When a session is resumed across a process restart (`process stopped != session stopped`), Harnesscope preserves conversation continuity.
- **Git Context & Conflict Detection**: Automatically captures Git snapshots before and after execution (commit SHA, branch, dirty status, modified files, diff statistics). If multiple executions run concurrently in the same worktree, their Git attribution is formally flagged as `AMBIGUOUS`.
- **Local & Private**: All data is stored in a local SQLite database (WAL mode). API keys, auth headers, tokens, and passwords are automatically redacted prior to persistence.

---

## Documentation

- [System Architecture](docs/ARCHITECTURE.md)
- [Homebrew Tap Guide](docs/HOMEBREW.md)
- [Cross-Platform & GUI Launcher](docs/CROSS_PLATFORM.md)
- [Developer Guide (English)](docs/DEVELOPMENT.md)
- [README (Русский)](README.ru.md)
- [Homebrew Руководство (Русский)](docs/HOMEBREW.ru.md)
- [Кроссплатформенность (Русский)](docs/CROSS_PLATFORM.ru.md)
- [Руководство разработчика (Русский)](docs/DEVELOPMENT.ru.md)

