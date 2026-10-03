use std::{
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

struct Server(Child);
impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn server_start_reports_failure_when_port_is_occupied() {
    let temp = tempfile::tempdir().unwrap();
    // Keep the listener open without serving HTTP. A detached child cannot bind
    // it, and successful process creation alone must not mean startup succeeded.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let output = Command::new(env!("CARGO_BIN_EXE_harnesscope"))
        .args(["server", "start", "--port", &port.to_string()])
        .env("HARNESSCOPE_DATA_DIR", temp.path())
        .env("HARNESSCOPE_DB_PATH", temp.path().join("test.db"))
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("did not become ready"));
}

#[test]
fn native_collection_resumes_when_the_server_restarts() {
    let temp = tempfile::tempdir().unwrap();
    let socket = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = socket.local_addr().unwrap().port();
    drop(socket);
    let exe = env!("CARGO_BIN_EXE_harnesscope");
    let url = format!("http://127.0.0.1:{port}");
    let path = temp.path().join("rollout-restart.jsonl");
    let fixture = include_str!("fixtures/rollout-codex.jsonl");
    let head = fixture.split_inclusive('\n').take(9).collect::<String>();
    std::fs::write(&path, &head).unwrap();
    let registered = Command::new(exe)
        .args(["sources", "add-codex", "--path"])
        .arg(&path)
        .env("HARNESSCOPE_DATA_DIR", temp.path())
        .env("HARNESSCOPE_DB_PATH", temp.path().join("test.db"))
        .output()
        .unwrap();
    assert!(
        registered.status.success(),
        "{}",
        String::from_utf8_lossy(&registered.stderr)
    );
    let start = || {
        Server(
            Command::new(exe)
                .args(["serve", "--port", &port.to_string()])
                .env("HARNESSCOPE_DATA_DIR", temp.path())
                .env("HARNESSCOPE_DB_PATH", temp.path().join("test.db"))
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap(),
        )
    };
    let client = reqwest::blocking::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(2))
        .build()
        .unwrap();
    let wait_for = |expected: u64| {
        let deadline = Instant::now() + Duration::from_secs(12);
        loop {
            if let Ok(response) = client.get(format!("{url}/api/v1/health")).send() {
                if let Ok(health) = response.json::<serde_json::Value>() {
                    if health["total_executions"] == expected {
                        break;
                    }
                }
            }
            assert!(
                Instant::now() < deadline,
                "collector did not reach {expected} executions"
            );
            std::thread::sleep(Duration::from_millis(50));
        }
    };
    let mut server = start();
    wait_for(1);
    client
        .post(format!("{url}/api/v1/shutdown"))
        .send()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while server.0.try_wait().unwrap().is_none() {
        assert!(
            Instant::now() < deadline,
            "collector prevented graceful shutdown"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    drop(server);
    // The collector must continue with its saved turn and token baseline.
    std::fs::write(&path, fixture).unwrap();
    let _server = start();
    wait_for(2);
    let id = harnesscope::adapters::codex::execution_id("fixture-thread", "turn-2");
    let detail: serde_json::Value = client
        .get(format!("{url}/api/v1/executions/{id}"))
        .send()
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(detail["execution"]["status"], "CANCELLED");
    assert_eq!(detail["usage"]["total_tokens"], 100);
    let health: serde_json::Value = client
        .get(format!("{url}/api/v1/health"))
        .send()
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(health["active_runtimes"], 0);
    assert_eq!(health["active_executions"], 0);
    client
        .post(format!("{url}/api/v1/shutdown"))
        .send()
        .unwrap();
}

#[test]
fn wrapper_auto_starts_server_at_configured_port() {
    let temp = tempfile::tempdir().unwrap();
    let socket = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = socket.local_addr().unwrap().port();
    drop(socket);
    let url = format!("http://127.0.0.1:{port}");
    let client = reqwest::blocking::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(1))
        .build()
        .unwrap();
    struct Shutdown(reqwest::blocking::Client, String);
    impl Drop for Shutdown {
        fn drop(&mut self) {
            let _ = self.0.post(format!("{}/api/v1/shutdown", self.1)).send();
        }
    }
    let _shutdown = Shutdown(client.clone(), url.clone());
    // Bound a regression where a detached server inherits the CLI output pipe.
    let (done_tx, done_rx) = std::sync::mpsc::channel();
    let watchdog_client = client.clone();
    let watchdog_url = url.clone();
    let watchdog = std::thread::spawn(move || {
        if done_rx.recv_timeout(Duration::from_secs(15)).is_err() {
            let _ = watchdog_client
                .post(format!("{watchdog_url}/api/v1/shutdown"))
                .send();
        }
    });
    let mut command = Command::new(env!("CARGO_BIN_EXE_harnesscope"));
    command.args(["run", "--runner", "generic"]);
    #[cfg(windows)]
    command.args(["cmd.exe", "/C", "exit 0"]);
    #[cfg(not(windows))]
    command.args(["sh", "-c", "exit 0"]);
    let output = command
        .env("HARNESSCOPE_DATA_DIR", temp.path())
        .env("HARNESSCOPE_DB_PATH", temp.path().join("test.db"))
        .env("HARNESSCOPE_SERVER_URL", &url)
        .current_dir(temp.path())
        .output()
        .unwrap();
    let _ = done_tx.send(());
    watchdog.join().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let health: serde_json::Value = client
        .get(format!("{url}/api/v1/health"))
        .send()
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(health["total_executions"], 1);
    assert_eq!(health["active_executions"], 0);
}

#[test]
fn actual_cli_preserves_exit_code_and_imports_into_custom_port() {
    let temp = tempfile::tempdir().unwrap();
    let socket = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = socket.local_addr().unwrap().port();
    drop(socket);
    let exe = env!("CARGO_BIN_EXE_harnesscope");
    let url = format!("http://127.0.0.1:{port}");
    let _server = Server(
        Command::new(exe)
            .args(["serve", "--port", &port.to_string()])
            .env("HARNESSCOPE_DATA_DIR", temp.path())
            .env("HARNESSCOPE_DB_PATH", temp.path().join("test.db"))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let client = reqwest::blocking::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(2))
        .build()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if client
            .get(format!("{url}/api/v1/health"))
            .send()
            .is_ok_and(|r| r.status().is_success())
        {
            break;
        }
        assert!(Instant::now() < deadline, "server did not start");
        std::thread::sleep(Duration::from_millis(50));
    }
    let mut command = Command::new(exe);
    command.args(["run", "--runner", "smoke"]);
    #[cfg(windows)]
    command.args(["cmd.exe", "/C", "exit 7"]);
    #[cfg(not(windows))]
    command.args(["sh", "-c", "exit 7"]);
    let output = command
        .env("HARNESSCOPE_SERVER_URL", &url)
        .env("HARNESSCOPE_DATA_DIR", temp.path())
        .env("HARNESSCOPE_DB_PATH", temp.path().join("test.db"))
        .current_dir(temp.path())
        .output()
        .unwrap();
    assert_eq!(
        output.status.code(),
        Some(7),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let records: serde_json::Value = client
        .get(format!("{url}/api/v1/executions"))
        .send()
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(records["total"], 1);
    assert_eq!(records["items"][0]["status"], "FAILED");
    assert_eq!(records["items"][0]["capture_scope"], "PROCESS");
    let path = temp.path().join("events.jsonl");
    std::fs::write(
        &path,
        r#"{"event_type":"custom.adapter.event","source":"smoke","payload":{"token":"secret"}}"#,
    )
    .unwrap();
    for _ in 0..2 {
        let output = Command::new(exe)
            .args(["ingest", "--file"])
            .arg(&path)
            .env("HARNESSCOPE_SERVER_URL", &url)
            .env("HARNESSCOPE_DATA_DIR", temp.path())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(result["processed"], 1);
    }
    client
        .post(format!("{url}/api/v1/shutdown"))
        .send()
        .unwrap();
}

#[test]
fn concurrent_offline_wrappers_are_recovered_without_duplicate_executions() {
    let temp = tempfile::tempdir().unwrap();
    let socket = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = socket.local_addr().unwrap().port();
    drop(socket);
    let url = format!("http://127.0.0.1:{port}");
    let db = temp.path().join("test.db");
    std::thread::scope(|scope| {
        for _ in 0..8 {
            let temp = &temp;
            let db = &db;
            let url = &url;
            scope.spawn(move || {
                let mut command = Command::new(env!("CARGO_BIN_EXE_harnesscope"));
                command.args(["run", "--runner", "offline"]);
                #[cfg(windows)]
                command.args(["cmd.exe", "/C", "exit 7"]);
                #[cfg(not(windows))]
                command.args(["sh", "-c", "exit 7"]);
                let output = command
                    .env("HARNESSCOPE_AUTOSTART", "0")
                    .env("HARNESSCOPE_DB_PATH", db)
                    .env("HARNESSCOPE_DATA_DIR", temp.path())
                    .env("HARNESSCOPE_SERVER_URL", url)
                    .current_dir(temp.path())
                    .output()
                    .unwrap();
                assert_eq!(output.status.code(), Some(7));
                let diagnostics = String::from_utf8_lossy(&output.stderr);
                assert!(
                    !diagnostics.contains("durable telemetry unavailable")
                        && !diagnostics.contains("telemetry could not be queued"),
                    "Offline wrapper lost telemetry: {diagnostics}"
                );
            });
        }
    });
    let q = harnesscope::outbox::Outbox::open(&db, &url).unwrap();
    assert!(!q.pending().unwrap().is_empty());
    let _server = Server(
        Command::new(env!("CARGO_BIN_EXE_harnesscope"))
            .args(["serve", "--port", &port.to_string()])
            .env("HARNESSCOPE_DB_PATH", &db)
            .env("HARNESSCOPE_DATA_DIR", temp.path())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let deadline = Instant::now() + Duration::from_secs(15);
    while !q.pending().unwrap().is_empty() {
        assert!(Instant::now() < deadline, "outbox recovery timed out");
        std::thread::sleep(Duration::from_millis(50));
    }
    let client = reqwest::blocking::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(2))
        .build()
        .unwrap();
    let data: serde_json::Value = client
        .get(format!("{url}/api/v1/executions"))
        .send()
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(data["total"], 8);
    assert!(data["items"]
        .as_array()
        .unwrap()
        .iter()
        .all(|item| item["exit_code"] == 7 && item["status"] == "FAILED"));
    assert_eq!(data["items"][0]["exit_code"], 7);
    assert_eq!(data["items"][0]["status"], "FAILED");
    let monitor: serde_json::Value = client
        .get(format!("{url}/api/v1/monitoring"))
        .send()
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(monitor["runtimes"].as_array().unwrap().len(), 8);
    assert!(monitor["runtimes"]
        .as_array()
        .unwrap()
        .iter()
        .all(|runtime| runtime["freshness"] == "ENDED"));
    assert_eq!(monitor["pending_events"], 0);
}

#[test]
fn wrapper_emits_periodic_child_heartbeat_and_stops_it_on_exit() {
    let temp = tempfile::tempdir().unwrap();
    let socket = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = socket.local_addr().unwrap().port();
    drop(socket);
    let url = format!("http://127.0.0.1:{port}");
    let db = temp.path().join("heartbeat.db");
    let _server = Server(
        Command::new(env!("CARGO_BIN_EXE_harnesscope"))
            .args(["serve", "--port", &port.to_string()])
            .env("HARNESSCOPE_DB_PATH", &db)
            .env("HARNESSCOPE_DATA_DIR", temp.path())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let mut command = Command::new(env!("CARGO_BIN_EXE_harnesscope"));
    command.args(["run", "--runner", "heartbeat"]);
    #[cfg(windows)]
    command.args([
        "powershell.exe",
        "-NoProfile",
        "-Command",
        "Start-Sleep -Seconds 18; exit 0",
    ]);
    #[cfg(not(windows))]
    command.args(["sh", "-c", "sleep 18; exit 0"]);
    let mut wrapper = Server(
        command
            .env("HARNESSCOPE_AUTOSTART", "0")
            .env("HARNESSCOPE_DB_PATH", &db)
            .env("HARNESSCOPE_DATA_DIR", temp.path())
            .env("HARNESSCOPE_SERVER_URL", &url)
            .current_dir(temp.path())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let client = reqwest::blocking::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(1))
        .build()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(25);
    let mut first = None;
    let mut refreshed = false;
    loop {
        if let Ok(response) = client.get(format!("{url}/api/v1/monitoring")).send() {
            if let Ok(data) = response.json::<serde_json::Value>() {
                let row = &data["runtimes"][0];
                if let Some(observed) = row["observed_at"].as_str() {
                    let observed = observed.to_string();
                    if let Some(initial) = &first {
                        if initial != &observed {
                            refreshed = true;
                        }
                    } else {
                        first = Some(observed);
                    }
                    if row["freshness"] == "ENDED" {
                        break;
                    }
                    assert_eq!(row["freshness"], "FRESH");
                    assert!(row["pid"].as_u64().unwrap() > 0);
                }
            }
        }
        assert!(
            Instant::now() < deadline,
            "heartbeat never reached completion"
        );
        std::thread::sleep(Duration::from_millis(200));
    }
    assert!(refreshed, "only initial heartbeat was recorded");
    assert!(wrapper.0.wait().unwrap().success());
}
