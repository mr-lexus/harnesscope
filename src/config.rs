use directories::ProjectDirs;
use std::path::PathBuf;

pub struct Config {
    pub data_dir: PathBuf,
    pub db_path: PathBuf,
    pub server_host: String,
    pub server_port: u16,
}

impl Config {
    pub fn load() -> Self {
        let data_dir = if let Ok(val) = std::env::var("HARNESSCOPE_DATA_DIR") {
            PathBuf::from(val)
        } else if let Some(proj_dirs) = ProjectDirs::from("com", "harnesscope", "harnesscope") {
            proj_dirs.data_local_dir().to_path_buf()
        } else {
            let home = std::env::var("USERPROFILE")
                .or_else(|_| std::env::var("HOME"))
                .unwrap_or_else(|_| ".".to_string());
            PathBuf::from(home).join(".harnesscope")
        };

        let db_path = if let Ok(val) = std::env::var("HARNESSCOPE_DB_PATH") {
            PathBuf::from(val)
        } else {
            data_dir.join("harnesscope.db")
        };

        let server_host =
            std::env::var("HARNESSCOPE_SERVER_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());

        let server_port = std::env::var("HARNESSCOPE_SERVER_PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(4242);

        Self {
            data_dir,
            db_path,
            server_host,
            server_port,
        }
    }

    pub fn server_url(&self) -> String {
        if let Ok(url) = std::env::var("HARNESSCOPE_SERVER_URL") {
            return url.trim_end_matches('/').to_string();
        }
        let host = if self.server_host.contains(':') {
            format!("[{}]", self.server_host)
        } else {
            self.server_host.clone()
        };
        format!("http://{}:{}", host, self.server_port)
    }

    pub fn ensure_data_dir(&self) -> std::io::Result<()> {
        if let Some(parent) = self.db_path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)?;
        }
        Ok(())
    }
}

pub fn local_server_url(value: &str) -> Result<reqwest::Url, &'static str> {
    let url = reqwest::Url::parse(value).map_err(|_| "Invalid server URL")?;
    let host = url.host_str().unwrap_or("").trim_matches(['[', ']']);
    if url.scheme() != "http"
        || !(host == "localhost"
            || host
                .parse::<std::net::IpAddr>()
                .is_ok_and(|ip| ip.is_loopback()))
        || !url.username().is_empty()
        || url.password().is_some()
        || url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(
            "Server URL must be a plain HTTP loopback origin, such as http://127.0.0.1:4242",
        );
    }
    Ok(url)
}
