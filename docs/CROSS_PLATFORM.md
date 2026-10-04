# Cross-Platform Architecture & Runner Support

[Documentation](../README.md#documentation)

> Current observation boundaries: wrappers track the launched process, not every internal turn or detached GUI session. Telemetry delivery has bounded waits, and failures do not stop the child. See [EVENTS.md](EVENTS.md) and [audit limitations](AUDIT.md).

Harnesscope is engineered from the ground up for native performance across **macOS**, **Linux**, and **Windows**.

---

## 1. Cross-Platform Runtime Matrix

| Feature | macOS (Intel / Apple Silicon) | Linux (x86_64 / arm64) | Windows (x64) |
|---|---|---|---|
| **Data Directory** | `~/Library/Application Support/com.harnesscope.harnesscope/` | `~/.local/share/harnesscope/` | `%LOCALAPPDATA%\harnesscope\harnesscope\data\` |
| **Database** | SQLite WAL mode (bundled C compiler, zero external runtime dependencies) | Same | Same |
| **Path Normalization** | Forward slashes (`/`), Unicode safe | Forward slashes (`/`), Unicode safe | Converts `\` to `/` in Git contexts |
| **Executable Lookup** | PATH + `/opt/homebrew/bin` + `/Applications/*.app/Contents/MacOS/*` | PATH + `/usr/local/bin` + `~/.local/bin` + `/snap/bin` | PATH + `.exe`, `.cmd`, `.bat`, `.ps1` |
| **Daemon Spawning** | Background POSIX child process (`Stdio::null`) | Background POSIX child process (`Stdio::null`) | `DETACHED_PROCESS` + `CREATE_NO_WINDOW` flags |
| **Process Stdio** | Inherited TTY (`Stdio::inherit`) | Inherited TTY (`Stdio::inherit`) | Inherited Console / Windows Terminal |

---

## 2. Launching CLI vs GUI Environments

Rather than forcing developers to construct artificial one-shot prompt strings, Harnesscope wraps the **entire development environment**:

### Dedicated Transparent Wrappers
- `harnesscope codex [args...]`: Transparently wraps Codex CLI or interactive TUI.
- `harnesscope copilot [args...]`: Transparently wraps GitHub Copilot CLI.
- `harnesscope opencode [args...]`: Transparently wraps OpenCode interactive CLI/TUI.

### Generic CLI & GUI Launcher (`harnesscope run`)
For any other agent CLI, TUI, or GUI editor:

```bash
# Launch Cursor Editor under Harnesscope telemetry
harnesscope run cursor .

# Launch VS Code with project folder
harnesscope run code .

# Launch Aider or custom agent CLI
harnesscope run --runner aider aider --model anthropic/claude-3-5-sonnet

# Explicit GUI flag
harnesscope run --gui my-agent-studio
```

### Automatic Telemetry Server Lifeline
Whenever any `harnesscope <runner>` or `harnesscope run ...` command is executed:
1. It verifies if the local Harnesscope server is already running on `127.0.0.1:4242`.
2. If offline, it **automatically and silently spawns the background server daemon**.
3. It captures pre-execution Git context and runtime metadata.
4. It launches the target command with transparent stdio.
5. On process exit, it captures post-execution Git diffs and completion metrics.
6. **Telemetry failures do not stop the agent process. Delivery has bounded waits.** If the server ever crashes, the agent command continues unimpeded.

---

## 3. Environment Variables

- `HARNESSCOPE_DATA_DIR`: Override data and database directory.
- `HARNESSCOPE_DB_PATH`: Override SQLite database file path.
- `HARNESSCOPE_SERVER_HOST`: Server bind address (default `127.0.0.1`).
- `HARNESSCOPE_SERVER_PORT`: Server port (default `4242`).
- `HARNESSCOPE_SERVER_URL`: URL used by wrappers to post events (default `http://127.0.0.1:4242`).
- `HARNESSCOPE_CODEX_BIN`: Custom path to Codex executable.
- `HARNESSCOPE_COPILOT_BIN`: Custom path to Copilot executable.
- `HARNESSCOPE_OPENCODE_BIN`: Custom path to OpenCode executable.
- `HARNESSCOPE_CURSOR_BIN`: Custom path to Cursor executable.
- `HARNESSCOPE_CODE_BIN`: Custom path to VS Code executable.
