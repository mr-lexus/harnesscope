use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "harnesscope",
    version = env!("CARGO_PKG_VERSION"),
    about = "Local cross-platform telemetry utility for AI coding agents",
    long_about = "Harnesscope transparently wraps AI coding agent CLIs (Codex, Copilot, OpenCode),\n\
                  collects deterministic local telemetry, and provides a lightweight Web UI."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Run Codex CLI transparently through Harnesscope wrapper
    Codex {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Run GitHub Copilot CLI transparently through Harnesscope wrapper
    Copilot {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Run OpenCode CLI/TUI transparently through Harnesscope wrapper
    Opencode {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Run any agent CLI, TUI, or GUI application under Harnesscope telemetry
    Run {
        /// Name of the runner/agent (e.g. codex, opencode, copilot, cursor, aider, code)
        #[arg(short, long)]
        runner: Option<String>,

        /// Mark surface as GUI instead of CLI
        #[arg(long)]
        gui: bool,

        /// Command or executable to execute
        command: String,

        /// Arguments passed to the target command
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Start Harnesscope server (ingestion, SQLite, API, Web UI)
    Serve {
        #[arg(long, env = "HARNESSCOPE_SERVER_HOST", default_value = "127.0.0.1")]
        host: String,

        #[arg(long, env = "HARNESSCOPE_SERVER_PORT", default_value_t = 4242)]
        port: u16,
    },
    /// Server management commands
    Server {
        #[command(subcommand)]
        action: ServerAction,
    },
    /// Open Harnesscope Web UI in default browser
    Ui {
        #[arg(long, env = "HARNESSCOPE_SERVER_PORT", default_value_t = 4242)]
        port: u16,
    },
    /// Diagnose toolchain, runner discovery, database, and system status
    Doctor,
    /// Import normalized JSON / JSONL events from any agent adapter (max 2 MiB, 500 events)
    Ingest {
        /// JSON object, JSON array or JSONL file; omit to read stdin
        #[arg(long)]
        file: Option<std::path::PathBuf>,
    },
    /// Register, inspect or scan native telemetry sources stored in this database
    Sources {
        #[command(subcommand)]
        action: SourceAction,
    },
    /// Inspect or retry durable wrapper telemetry for the configured database and server URL
    Outbox {
        #[command(subcommand)]
        action: OutboxAction,
    },
    /// Consistent main-database snapshots (pending outbox and source files are separate)
    Backup {
        #[command(subcommand)]
        action: BackupAction,
    },
    /// Deterministic demo data commands
    Demo {
        #[command(subcommand)]
        action: DemoAction,
    },
}

#[derive(Subcommand, Debug)]
pub enum ServerAction {
    /// Check whether Harnesscope server is currently running
    Status,
    /// Start Harnesscope server as a background daemon
    Start {
        #[arg(long, env = "HARNESSCOPE_SERVER_PORT", default_value_t = 4242)]
        port: u16,
    },
    /// Stop the running Harnesscope server
    Stop {
        #[arg(long, env = "HARNESSCOPE_SERVER_PORT", default_value_t = 4242)]
        port: u16,
    },
}

#[derive(Subcommand, Debug)]
pub enum DemoAction {
    /// Populate database with deterministic demo sessions, executions, and components
    Seed,
}

#[derive(Subcommand, Debug)]
pub enum SourceAction {
    /// Register a Codex rollout file or directory (rollout-*.jsonl); serve polls it every 5 seconds
    AddCodex {
        #[arg(long)]
        path: std::path::PathBuf,
        /// Store redacted prompt summaries and error messages; default is metadata only
        #[arg(long)]
        include_content: bool,
    },
    /// List sources and file checkpoints as JSON
    List,
    /// Run one bounded import pass without an HTTP server; repeat while files show BACKLOG
    Scan,
    /// Pause collection without deleting any data
    Pause { id: String },
    /// Resume collection from saved checkpoints
    Resume { id: String },
    /// Remove a registration and its checkpoints; imported telemetry is retained
    Remove { id: String },
}

#[derive(Subcommand, Debug)]
pub enum OutboxAction {
    /// List pending delivery batches as JSON
    Status,
    /// Clear retry delays and validation blocks; retains all queued events
    Retry,
    /// Try up to 100 queued batches; nonzero exit if any remain
    Flush,
}

#[derive(Subcommand, Debug)]
pub enum BackupAction {
    /// Snapshot the configured database, including committed WAL changes
    Create {
        #[arg(long)]
        output: std::path::PathBuf,
    },
    /// Check SQLite integrity, foreign keys and supported Harnesscope schema
    Verify {
        #[arg(long)]
        file: std::path::PathBuf,
    },
    /// Restore to a NEW path, verify it and pause native collectors
    Restore {
        #[arg(long)]
        file: std::path::PathBuf,
        #[arg(long)]
        output: std::path::PathBuf,
    },
}
