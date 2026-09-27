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
        #[arg(long, default_value = "127.0.0.1")]
        host: String,

        #[arg(long, default_value_t = 4242)]
        port: u16,
    },
    /// Server management commands
    Server {
        #[command(subcommand)]
        action: ServerAction,
    },
    /// Open Harnesscope Web UI in default browser
    Ui {
        #[arg(long, default_value_t = 4242)]
        port: u16,
    },
    /// Diagnose toolchain, runner discovery, database, and system status
    Doctor,
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
        #[arg(long, default_value_t = 4242)]
        port: u16,
    },
    /// Stop the running Harnesscope server
    Stop {
        #[arg(long, default_value_t = 4242)]
        port: u16,
    },
}

#[derive(Subcommand, Debug)]
pub enum DemoAction {
    /// Populate database with deterministic demo sessions, executions, and components
    Seed,
}
