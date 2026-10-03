use clap::Parser;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use harnesscope::cli::{Cli, Commands, DemoAction, ServerAction};
use harnesscope::config::Config;
use harnesscope::demo;
use harnesscope::git;
use harnesscope::runners::discovery::discover_runner_binary;
use harnesscope::runners::{execute_wrapper, execute_wrapper_generic, spawn_background_server};
use harnesscope::server::correlation::CorrelationEngine;
use harnesscope::server::handlers::AppState;
use harnesscope::server::routes::build_router;
use harnesscope::storage::{Database, Repository};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let config = Config::load();

    match cli.command {
        Commands::Codex { args } => {
            let code = tokio::task::spawn_blocking(move || {
                execute_wrapper("codex", &args, &config.server_url())
            })
            .await?;
            std::process::exit(code);
        }
        Commands::Copilot { args } => {
            let code = tokio::task::spawn_blocking(move || {
                execute_wrapper("copilot", &args, &config.server_url())
            })
            .await?;
            std::process::exit(code);
        }
        Commands::Opencode { args } => {
            let code = tokio::task::spawn_blocking(move || {
                execute_wrapper("opencode", &args, &config.server_url())
            })
            .await?;
            std::process::exit(code);
        }
        Commands::Run {
            runner,
            gui,
            command,
            args,
        } => {
            let runner_name = runner.unwrap_or_else(|| {
                Path::new(&command)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("agent")
                    .to_lowercase()
            });

            let surface =
                if gui || ["cursor", "code", "windsurf", "zed"].contains(&runner_name.as_str()) {
                    "gui"
                } else {
                    "cli"
                };

            let code = tokio::task::spawn_blocking(move || {
                execute_wrapper_generic(
                    &runner_name,
                    surface,
                    &command,
                    &args,
                    &config.server_url(),
                )
            })
            .await?;
            std::process::exit(code);
        }
        Commands::Serve { host, port } => {
            init_tracing();
            run_server(&host, port, &config).await?;
        }
        Commands::Server { action } => match action {
            ServerAction::Status => {
                check_server_status(&config).await;
            }
            ServerAction::Start { port } => {
                let url = format!("http://127.0.0.1:{}", port);
                let health_url = format!("{}/api/v1/health", url);
                let client = reqwest::Client::builder()
                    .no_proxy()
                    .redirect(reqwest::redirect::Policy::none())
                    .timeout(std::time::Duration::from_millis(500))
                    .build()?;

                if client
                    .get(&health_url)
                    .send()
                    .await
                    .map(|r| r.status().is_success())
                    .unwrap_or(false)
                {
                    println!("Harnesscope Server is already running at {}", url);
                    return Ok(());
                }

                let current_exe = std::env::current_exe()?;
                spawn_background_server(&current_exe, port)?;
                println!(
                    "Starting Harnesscope server in background on port {}...",
                    port
                );

                let mut started = false;
                for _ in 0..10 {
                    tokio::time::sleep(std::time::Duration::from_millis(150)).await;
                    if client
                        .get(&health_url)
                        .send()
                        .await
                        .map(|r| r.status().is_success())
                        .unwrap_or(false)
                    {
                        started = true;
                        break;
                    }
                }

                if started {
                    println!("✓ Harnesscope Server started successfully at {}", url);
                } else {
                    return Err(format!(
                        "Server did not become ready at {url}. Run harnesscope serve --port {port} to inspect the startup error."
                    ).into());
                }
            }
            ServerAction::Stop { port } => {
                let url = format!("http://127.0.0.1:{}", port);
                let client = reqwest::Client::builder()
                    .no_proxy()
                    .redirect(reqwest::redirect::Policy::none())
                    .timeout(std::time::Duration::from_millis(1000))
                    .build()?;

                match client.post(format!("{}/api/v1/shutdown", url)).send().await {
                    Ok(resp) if resp.status().is_success() => {
                        println!("✓ Harnesscope Server stopped.");
                    }
                    _ => {
                        println!(
                            "Harnesscope server was not running or not responding at {}",
                            url
                        );
                    }
                }
            }
        },
        Commands::Ui { port } => {
            let url = format!("http://127.0.0.1:{}", port);
            println!("Opening Harnesscope Web UI: {}", url);
            let client = reqwest::Client::builder()
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .timeout(std::time::Duration::from_millis(500))
                .build()?;

            let is_running = client
                .get(format!("{}/api/v1/health", url))
                .send()
                .await
                .map(|r| r.status().is_success())
                .unwrap_or(false);

            if !is_running {
                let current_exe = std::env::current_exe()?;
                if spawn_background_server(&current_exe, port).is_ok() {
                    println!("Started Harnesscope server in background on port {}.", port);
                    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                }
            }
            let _ = open::that(&url);
        }
        Commands::Doctor => {
            run_doctor(&config).await;
        }
        Commands::Ingest { file } => {
            let events = tokio::task::spawn_blocking(move || {
                harnesscope::ingest::read_events(file.as_deref())
            })
            .await??;
            let url = config.server_url();
            harnesscope::config::local_server_url(&url)?;
            let startup_url = url.clone();
            tokio::task::spawn_blocking(move || {
                harnesscope::runners::wrapper::ensure_server_running(&startup_url)
            })
            .await?;
            let response = reqwest::Client::builder()
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .timeout(std::time::Duration::from_secs(10))
                .build()?
                .post(format!("{url}/api/v1/events"))
                .json(&events)
                .send()
                .await?;
            let status = response.status();
            println!("{}", response.text().await?);
            if !status.is_success() {
                return Err(format!(
                    "Import failed: HTTP {status}; successfully applied event IDs are shown above"
                )
                .into());
            }
        }
        Commands::Backup { action } => {
            tokio::task::spawn_blocking(move || -> Result<(), String> {
                use harnesscope::{cli::BackupAction, storage::backup};
                let report = match action {
                    BackupAction::Create { output } => backup::create(&config.db_path, &output),
                    BackupAction::Verify { file } => backup::verify(&file),
                    BackupAction::Restore { file, output } => backup::restore(&file, &output),
                }?;
                println!(
                    "{}",
                    serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?
                );
                Ok(())
            })
            .await??;
        }
        Commands::Sources { action } => {
            tokio::task::spawn_blocking(move || -> Result<(), String> {
                use harnesscope::cli::SourceAction;
                config.ensure_data_dir().map_err(|e| e.to_string())?;
                let db = Database::open(&config.db_path).map_err(|e| e.to_string())?;
                let repo = Arc::new(Repository::new(db));
                match action {
                    SourceAction::AddCodex {
                        path,
                        include_content,
                    } => {
                        let source = repo.add_source(&path, include_content)?;
                        println!("{}", serde_json::to_string_pretty(&source).unwrap());
                    }
                    SourceAction::Scan => {
                        let engine = CorrelationEngine::new(repo.clone());
                        harnesscope::adapters::scan_sources(
                            &repo,
                            &engine,
                            &std::sync::atomic::AtomicBool::new(false),
                        )?;
                        println!(
                            "{}",
                            serde_json::to_string_pretty(
                                &repo.list_sources().map_err(|e| e.to_string())?
                            )
                            .unwrap()
                        );
                        if repo
                            .list_sources()
                            .map_err(|e| e.to_string())?
                            .iter()
                            .any(|s| s.enabled && s.last_error.is_some())
                        {
                            return Err("Some sources need attention; inspect sources list".into());
                        }
                    }
                    SourceAction::List => {
                        let sources = repo.list_sources().map_err(|e| e.to_string())?;
                        let mut output = Vec::new();
                        for source in sources {
                            let files = repo
                                .list_source_files(&source.id)
                                .map_err(|e| e.to_string())?;
                            output.push(serde_json::json!({"source":source,"files":files}));
                        }
                        println!("{}", serde_json::to_string_pretty(&output).unwrap());
                    }
                    SourceAction::Pause { id } => {
                        if !repo
                            .set_source_enabled(&id, false)
                            .map_err(|e| e.to_string())?
                        {
                            return Err("Source not found".into());
                        }
                    }
                    SourceAction::Resume { id } => {
                        if !repo
                            .set_source_enabled(&id, true)
                            .map_err(|e| e.to_string())?
                        {
                            return Err("Source not found".into());
                        }
                    }
                    SourceAction::Remove { id } => {
                        if !repo.remove_source(&id).map_err(|e| e.to_string())? {
                            return Err("Source not found".into());
                        }
                    }
                }
                Ok(())
            })
            .await?
            .map_err(std::io::Error::other)?;
        }
        Commands::Outbox { action } => {
            tokio::task::spawn_blocking(move || -> Result<(), String> {
                use harnesscope::cli::OutboxAction;
                let queue =
                    harnesscope::outbox::Outbox::open(&config.db_path, &config.server_url())?;
                match action {
                    OutboxAction::Retry => queue.retry()?,
                    OutboxAction::Flush => {
                        for _ in 0..100 {
                            if !queue.deliver_http()? {
                                break;
                            }
                        }
                    }
                    OutboxAction::Status => {}
                }
                let pending = queue.pending()?;
                println!("{}", serde_json::to_string_pretty(&pending).unwrap());
                if matches!(action, OutboxAction::Flush) && !pending.is_empty() {
                    return Err("Events remain queued; inspect outbox status".into());
                }
                Ok(())
            })
            .await?
            .map_err(std::io::Error::other)?;
        }
        Commands::Demo { action } => match action {
            DemoAction::Seed => {
                config.ensure_data_dir()?;
                let db = Database::open(&config.db_path)?;
                let repo = Repository::new(db);
                let msg = demo::seed_demo_data(&repo)?;
                println!("✓ {}", msg);
                println!("Data location: {:?}", config.db_path);
                println!("You can now run 'harnesscope serve' or 'harnesscope ui' to explore the demo data.");
            }
        },
    }

    Ok(())
}

fn init_tracing() {
    let _ = tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "harnesscope=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .try_init();
}

async fn run_server(
    host: &str,
    port: u16,
    config: &Config,
) -> Result<(), Box<dyn std::error::Error>> {
    config.ensure_data_dir()?;
    let ip = if host == "localhost" {
        "127.0.0.1"
    } else {
        host
    }
    .trim_matches(['[', ']'])
    .parse::<std::net::IpAddr>()?;
    let addr = SocketAddr::new(ip, port);
    if !addr.ip().is_loopback() {
        return Err("Harnesscope only supports loopback addresses".into());
    }
    let db = Database::open(&config.db_path)?;
    let repo = Arc::new(Repository::new(db));
    let engine = Arc::new(CorrelationEngine::new(repo.clone()));

    let (shutdown_tx, mut shutdown_rx) = tokio::sync::mpsc::channel::<()>(1);

    let outbox = harnesscope::outbox::Outbox::open(&config.db_path, &format!("http://{addr}"))
        .map_err(std::io::Error::other)?;
    let state = AppState {
        repo: repo.clone(),
        engine: engine.clone(),
        shutdown_tx: Some(shutdown_tx),
        outbox: Some(outbox.clone()),
    };

    let router = build_router(state);

    println!("============================================================");
    println!(
        "  Harnesscope Server v{} (Cross-Platform Telemetry)",
        env!("CARGO_PKG_VERSION")
    );
    println!("============================================================");
    println!("  SQLite Database : {:?}", config.db_path);
    println!("  API Endpoint    : http://{}:{}/api/v1", host, port);
    println!("  Web UI          : http://{}:{}", host, port);
    println!("  Status          : Listening on localhost");
    println!("============================================================");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    // The server owns the collector: no extra daemon or machine-wide scheduled job.
    // The join below waits for the current bounded batch before process shutdown.
    let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let worker_stop = stop.clone();
    let (cancel_tx, mut cancel_rx) = tokio::sync::watch::channel(false);
    let delivery_engine = engine.clone();
    let mut delivery_cancel = cancel_rx.clone();
    let delivery = tokio::spawn(async move {
        loop {
            let queue = outbox.clone();
            let engine = delivery_engine.clone();
            match tokio::task::spawn_blocking(move || -> Result<(), String> {
                for _ in 0..20 {
                    if !queue.deliver_local(&engine)? {
                        break;
                    }
                }
                Ok(())
            })
            .await
            {
                Ok(Ok(())) => {}
                result => tracing::warn!("Outbox pass failed: {result:?}"),
            }
            tokio::select! {
                _ = tokio::time::sleep(std::time::Duration::from_secs(1)) => {},
                _ = delivery_cancel.changed() => break,
            }
        }
    });
    let collector = tokio::spawn(async move {
        loop {
            let scan_repo = repo.clone();
            let scan_engine = engine.clone();
            let scan_stop = worker_stop.clone();
            let backlogged = match tokio::task::spawn_blocking(move || -> Result<bool, String> {
                harnesscope::adapters::scan_sources(&scan_repo, &scan_engine, &scan_stop)?;
                Ok(
                    scan_repo.collection_status().map_err(|e| e.to_string())?["backlog_files"]
                        .as_u64()
                        .unwrap_or(0)
                        > 0,
                )
            })
            .await
            {
                Ok(Ok(value)) => value,
                result => {
                    tracing::warn!("Collector pass failed: {result:?}");
                    false
                }
            };
            if worker_stop.load(std::sync::atomic::Ordering::Relaxed) {
                break;
            }
            tokio::select! {
                _=cancel_rx.changed()=>break,
                _=tokio::time::sleep(std::time::Duration::from_millis(if backlogged {250} else {5000}))=>{},
            }
        }
    });
    let server_result = axum::serve(listener, router)
        .with_graceful_shutdown(async move {
            tokio::select! {
                _ = shutdown_rx.recv() => {},
                _ = shutdown_signal() => {},
            }
        })
        .await;
    stop.store(true, std::sync::atomic::Ordering::Relaxed);
    let _ = cancel_tx.send(true);
    collector.await?;
    delivery.await?;
    server_result?;
    Ok(())
}

async fn check_server_status(config: &Config) {
    let url = config.server_url();
    let health_url = format!("{}/api/v1/health", url);
    let client = match reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_millis(1000))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            println!(
                "Harnesscope Server Status: Error creating HTTP client: {}",
                e
            );
            return;
        }
    };

    match client.get(&health_url).send().await {
        Ok(resp) => {
            if resp.status().is_success() {
                if let Ok(json) = resp.json::<serde_json::Value>().await {
                    println!("✓ Harnesscope Server is RUNNING at {}", url);
                    println!(
                        "  Version           : {}",
                        json.get("version")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown")
                    );
                    println!(
                        "  Active Executions : {}",
                        json.get("active_executions")
                            .and_then(|v| v.as_i64())
                            .unwrap_or(0)
                    );
                    println!(
                        "  Active Runtimes   : {}",
                        json.get("active_runtimes")
                            .and_then(|v| v.as_i64())
                            .unwrap_or(0)
                    );
                    println!(
                        "  Total Executions  : {}",
                        json.get("total_executions")
                            .and_then(|v| v.as_i64())
                            .unwrap_or(0)
                    );
                    println!(
                        "  Total Sessions    : {}",
                        json.get("total_sessions")
                            .and_then(|v| v.as_i64())
                            .unwrap_or(0)
                    );
                } else {
                    println!(
                        "✓ Harnesscope Server is RUNNING at {} (health response received)",
                        url
                    );
                }
            } else {
                println!("! Harnesscope Server returned status: {}", resp.status());
            }
        }
        Err(_) => {
            println!("✗ Harnesscope Server is NOT running at {}", url);
            println!("  Start it with: harnesscope serve");
        }
    }
}

async fn run_doctor(config: &Config) {
    println!("============================================================");
    println!("  Harnesscope Doctor — System & Telemetry Diagnostics");
    println!("============================================================");

    // Platform
    println!("Platform:");
    println!("  OS                : {}", std::env::consts::OS);
    println!("  Arch              : {}", std::env::consts::ARCH);
    println!("  Data directory    : {:?}", config.data_dir);
    println!("  Database path     : {:?}", config.db_path);

    // DB Check
    print!("SQLite Database     : ");
    match Database::open(&config.db_path) {
        Ok(_) => println!("✓ OK (Accessible, migrations applied, WAL enabled)"),
        Err(e) => println!("✗ ERROR ({})", e),
    }

    // Git Check
    print!("Git Executable      : ");
    if git::is_git_available() {
        println!("✓ OK (git installed and available on PATH)");
    } else {
        println!("! WARNING (git not found on PATH; Git telemetry will be UNKNOWN)");
    }

    // Runner discovery
    println!("\nRunner & Environment Discovery:");
    let runners = [
        ("Codex CLI", "codex", "HARNESSCOPE_CODEX_BIN"),
        ("Copilot CLI", "copilot", "HARNESSCOPE_COPILOT_BIN"),
        ("OpenCode CLI/TUI", "opencode", "HARNESSCOPE_OPENCODE_BIN"),
        ("Cursor Editor", "cursor", "HARNESSCOPE_CURSOR_BIN"),
        ("VS Code", "code", "HARNESSCOPE_CODE_BIN"),
    ];

    for (label, name, env_var) in runners {
        let found = discover_runner_binary(name)
            .or_else(|| harnesscope::runners::discovery::find_executable(name));
        match found {
            Some(path) => {
                println!("  {:<18}: ✓ FOUND at {:?}", label, path);
            }
            None => {
                println!("  {:<18}: - NOT FOUND (can specify via {})", label, env_var);
            }
        }
    }

    // Server check
    println!("\nHarnesscope Server:");
    let url = config.server_url();
    let client = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_millis(500))
        .build();

    let server_ok = if let Ok(c) = client {
        c.get(format!("{}/api/v1/health", url))
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    } else {
        false
    };

    if server_ok {
        println!("  Local Server      : ✓ RUNNING at {}", url);
    } else {
        println!("  Local Server      : - NOT RUNNING (run 'harnesscope serve' to start)");
    }

    println!("============================================================");
}

async fn shutdown_signal() {
    #[cfg(unix)]
    {
        if let Ok(mut terminate) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            tokio::select! { _=tokio::signal::ctrl_c()=>{}, _=terminate.recv()=>{} }
            return;
        }
    }
    let _ = tokio::signal::ctrl_c().await;
}
