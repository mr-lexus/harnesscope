# Developer Guide (Harnesscope)

This document provides developer guidelines for building, testing, extending, and debugging Harnesscope.

---

## 1. Prerequisites

- **Rust toolchain**: 1.80+ (`cargo`, `rustc`)
- **Node.js**: v20.19+ or v22.12+ with `npm` (Node.js 24 LTS recommended)
- **Git**: 2.30+ installed and available on PATH
- **Supported OS**: Windows 10/11, macOS (Intel/Apple Silicon), Linux (Ubuntu, Debian, Fedora, Arch)

---

## 2. Project Structure

```
harnesscope/
├── Cargo.toml               # Rust workspace definition and dependencies
├── src/
│   ├── main.rs              # CLI entry point and server startup
│   ├── lib.rs               # Library export for tests and integrations
│   ├── cli.rs               # Clap argument definitions
│   ├── config.rs            # Config & directory path resolution
│   ├── redact.rs            # Secret redaction & SHA-256 helpers
│   ├── git.rs               # Safe deterministic Git context collection
│   ├── demo.rs              # Deterministic demo seed dataset
│   ├── domain/              # Core domain entities & event models
│   │   ├── models.rs        # RuntimeInstance, Session, Execution, etc.
│   │   ├── events.rs        # IngestEvent and IngestPayload
│   │   └── mod.rs
│   ├── storage/             # Persistence layer
│   │   ├── db.rs            # SQLite connection setup (WAL, busy_timeout)
│   │   ├── migrations.rs    # Schema migrations versioning
│   │   ├── repository.rs    # SQL queries and filtering
│   │   └── mod.rs
│   ├── server/              # Axum HTTP API & embedded UI
│   │   ├── correlation.rs   # Correlation engine: identity & attribution
│   │   ├── handlers.rs      # REST handlers for health, executions, sessions, stats
│   │   ├── routes.rs        # Route table & CORS setup
│   │   ├── static_files.rs  # Embedded SPA asset server
│   │   └── mod.rs
│   └── runners/             # Transparent agent wrappers
│       ├── discovery.rs     # Executable locator (PATH & env vars)
│       ├── wrapper.rs       # Process execution, stdio passthrough, exit code
│       ├── codex.rs         # Codex CLI parser & config discovery
│       ├── copilot.rs       # Copilot CLI parser & config discovery
│       ├── opencode.rs      # OpenCode CLI/TUI & OMO parser
│       └── mod.rs
├── tests/
│   └── harnesscope_tests.rs # Comprehensive integration test suite
├── web/                     # React + Vite + Mantine frontend
│   ├── package.json
│   ├── vite.config.ts
│   ├── tsconfig.json
│   ├── index.html
│   └── src/
│       ├── app/             # Application shell and providers
│       ├── pages/           # Executions, ExecutionDetail, Sessions, SessionDetail, Stats
│       ├── widgets/         # Header and common navigation
│       ├── features/        # ExecutionFilters, DemoSeedButton
│       ├── entities/        # TypeScript interfaces (Execution, Session, Stats)
│       └── shared/          # API client, UI badges, utility helpers
└── docs/                    # Documentation
    ├── ARCHITECTURE.md
    ├── DEVELOPMENT.md
    └── DEVELOPMENT.ru.md
```

---

## 3. Development Workflow

### Frontend Development
To run the Vite dev server with hot module reloading:
```bash
cd web
npm install
npm run dev
```
Vite runs at `http://localhost:5173` and automatically proxies `/api` requests to `http://127.0.0.1:4242`.

To build the production frontend:
```bash
cd web
npm run build
```
This outputs compiled assets into `web/dist/`, which are then automatically embedded into the Rust binary via `rust-embed`.

### Backend Development
Run the backend server in development mode:
```bash
cargo run -- serve --port 4242
```
Or check for compilation issues quickly:
```bash
cargo check
```

To build a standalone production release binary:
```bash
cd web && npm run build && cd ..
cargo build --release
```
The resulting executable is located at `target/release/harnesscope` (or `harnesscope.exe` on Windows).

---

## 4. Running Tests

Run the full integration test suite:
```bash
cargo test
```

The test suite covers:
- SQLite schema migrations
- Concurrent event ingestion (WAL mode lock safety)
- Simultaneous executions in distinct worktrees
- Parallel executions in the same worktree (`AMBIGUOUS` attribution)
- Session resume across runtime restart (`process stopped != session stopped`)
- Isolation of separate native sessions in the same worktree/branch
- Secret redaction (OpenAI, Anthropic, GitHub tokens, passwords, bearer tokens)
- Unknown event field tolerance
- Runner crash and abnormal termination handling
- Transparent exit code passthrough

---

## 5. Database Location and Reset

- **Windows**: `%LOCALAPPDATA%\harnesscope\harnesscope\data\harnesscope.db`
- **Linux**: `~/.local/share/harnesscope/harnesscope.db`
- **macOS**: `~/Library/Application Support/com.harnesscope.harnesscope/harnesscope.db`
- **Custom Path**: Set the environment variable `HARNESSCOPE_DB_PATH=/path/to/custom.db`.

To reset the database, simply delete the `.db` file or set a temporary DB path:
```bash
# Windows PowerShell
Remove-Item "$env:LOCALAPPDATA\harnesscope\harnesscope\data\harnesscope.db*"
# Linux / macOS
rm -f ~/.local/share/harnesscope/harnesscope.db*
```

---

## 6. How Database Migrations Work

Migrations are managed in `src/storage/migrations.rs`.
- The `_schema_migrations` table records applied versions.
- When `Database::open` is called, `run_migrations` checks the current version and applies new migration functions sequentially in short transactions.
- Future migrations should increment the version and execute within standard SQLite transactions.

---

## 7. How to Add a New Runner Adapter

To support a new AI coding agent (e.g. `cursor`, `aider`, `cline`):

1. **Create an argument and config parser** in `src/runners/<runner>.rs`:
   - Implement `parse_<runner>_args_and_env(args: &[String], cwd: &Path) -> RunnerMetadata`.
   - Extract native session IDs, model flags, reasoning effort flags, agent roles.
   - Read local configuration files if present, discover MCP servers, skills, or plugins, and set state to `CONFIGURED`.
   - Never guess: if unknown, set to `"UNKNOWN"`.
2. **Add discovery logic** in `src/runners/discovery.rs`:
   - Add environment variable lookup `HARNESSCOPE_<RUNNER>_BIN`.
   - Add default executable names for PATH search.
3. **Register CLI subcommand** in `src/cli.rs`:
   ```rust
   #[command(subcommand)]
   pub enum Commands {
       ...
       NewRunner {
           #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
           args: Vec<String>,
       },
   }
   ```
4. **Dispatch command** in `src/main.rs`:
   ```rust
   Commands::NewRunner { args } => {
       let code = execute_wrapper("newrunner", &args, &config.server_url());
       std::process::exit(code);
   }
   ```
5. **Add parser unit tests** verifying argument parsing and config discovery.
