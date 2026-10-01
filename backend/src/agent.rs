use crate::{authorized, error, Shared};
use axum::{
    body::Bytes,
    extract::{ConnectInfo, OriginalUri, State},
    http::{HeaderMap, Method, StatusCode},
    response::{IntoResponse, Response},
};
use serde_json::Value;
use std::{path::PathBuf, process::Stdio, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::{Child, ChildStdin, Command},
    sync::Mutex,
};

struct Running {
    child: Child,
    _stdin: Option<ChildStdin>,
    port: u16,
}
pub struct Bridge {
    root: PathBuf,
    gateway_url: String,
    token: String,
    running: Mutex<Option<Running>>,
    client: reqwest::Client,
}
impl Bridge {
    pub async fn shutdown(&self) {
        if let Some(mut running) = self.running.lock().await.take() {
            running._stdin.take();
            if tokio::time::timeout(Duration::from_secs(5), running.child.wait())
                .await
                .is_err()
            {
                running.child.kill().await.ok();
            }
        }
    }
    pub fn new(root: PathBuf, gateway_url: String) -> Self {
        let mut bytes = [0u8; 32];
        getrandom::getrandom(&mut bytes).expect("Cannot generate Agent service token");
        Self {
            root,
            gateway_url,
            token: bytes.iter().map(|b| format!("{b:02x}")).collect(),
            running: Mutex::new(None),
            client: reqwest::Client::builder()
                .no_proxy()
                .timeout(Duration::from_secs(45))
                .build()
                .unwrap(),
        }
    }
    async fn port(&self) -> Result<u16, String> {
        let mut running = self.running.lock().await;
        if let Some(state) = running.as_mut() {
            if state.child.try_wait().map_err(|e| e.to_string())?.is_none() {
                return Ok(state.port);
            }
        }
        *running = None;
        let script = self.root.join("agent/server.mjs");
        if !script.exists()
            || !self
                .root
                .join("agent/node_modules/@earendil-works/pi-coding-agent")
                .exists()
        {
            return Err(
                "Pi Agent dependencies are missing. Run npm ci --prefix agent with Node.js 22.19+"
                    .into(),
            );
        }
        let node = std::env::var("PI_NODE_BIN").unwrap_or_else(|_| "node".into());
        let cwd = std::env::var("PI_AGENT_WORKSPACE")
            .map(PathBuf::from)
            .unwrap_or_else(|_| self.root.clone());
        let mut command = Command::new(node);
        command.arg(&script).current_dir(&self.root).env_clear();
        for name in ["PATH", "HOME", "TMPDIR", "SHELL", "LANG", "SystemRoot"] {
            if let Some(value) = std::env::var_os(name) {
                command.env(name, value);
            }
        }
        let mut child = command
            .env("PI_BRIDGE_TOKEN", &self.token)
            .env("PI_GATEWAY_URL", &self.gateway_url)
            .env("PI_DEFAULT_CWD", cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| format!("Cannot start Pi Agent (Node.js 22.19+ required): {e}"))?;
        let stdin = child.stdin.take();
        let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
        let line = tokio::time::timeout(Duration::from_secs(20), lines.next_line())
            .await
            .map_err(|_| "Pi Agent startup timed out")?
            .map_err(|e| e.to_string())?
            .ok_or("Pi Agent exited during startup")?;
        let ready: Value =
            serde_json::from_str(&line).map_err(|_| "Invalid Pi Agent startup response")?;
        let port = ready["port"]
            .as_u64()
            .and_then(|p| u16::try_from(p).ok())
            .filter(|p| *p != 0)
            .ok_or("Invalid Pi Agent port")?;
        *running = Some(Running {
            child,
            _stdin: stdin,
            port,
        });
        Ok(port)
    }
}

pub async fn proxy(
    State(app): State<Shared>,
    ConnectInfo(peer): ConnectInfo<std::net::SocketAddr>,
    OriginalUri(uri): OriginalUri,
    method: Method,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if !peer.ip().is_loopback() && app.token.is_empty() {
        return error(
            StatusCode::FORBIDDEN,
            "Remote Pi Agent access requires GATEWAY_API_KEY management authentication",
        );
    }
    if !authorized(&app, &headers) {
        return error(
            StatusCode::UNAUTHORIZED,
            "Gateway management key required for Pi Agent",
        );
    }
    if headers
        .get("x-gateway-settings")
        .and_then(|h| h.to_str().ok())
        != Some("1")
    {
        return error(StatusCode::FORBIDDEN, "X-Gateway-Settings header required");
    }
    let Some(path) = uri.path().strip_prefix("/api/agent/") else {
        return error(StatusCode::NOT_FOUND, "Unknown Agent route");
    };
    let parts: Vec<_> = path.split('/').collect();
    let valid = matches!(path, "status" | "sessions")
        || (parts.first() == Some(&"sessions")
            && (2..=3).contains(&parts.len())
            && !parts[1].is_empty()
            && parts[1]
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-')
            && (parts.len() == 2 || matches!(parts[2], "prompt" | "abort")));
    if !valid {
        return error(StatusCode::NOT_FOUND, "Unknown Agent route");
    }
    let port = match app.agent.port().await {
        Ok(port) => port,
        Err(message) => return error(StatusCode::SERVICE_UNAVAILABLE, &message),
    };
    let gateway_key = app.gateway_key.read().await.clone();
    let key = if !app.token.is_empty() {
        &app.token
    } else if !gateway_key.is_empty() {
        &gateway_key
    } else {
        "local"
    };
    let response = match app
        .agent
        .client
        .request(method, format!("http://127.0.0.1:{port}/{path}"))
        .bearer_auth(&app.agent.token)
        .header("x-pi-gateway-key", key)
        .header("content-type", "application/json")
        .body(body)
        .send()
        .await
    {
        Ok(response) => response,
        Err(_) => {
            return error(
                StatusCode::BAD_GATEWAY,
                "Pi Agent service disconnected; retry and reload sessions",
            )
        }
    };
    let status = response.status();
    match response.bytes().await {
        Ok(bytes) => (
            status,
            [
                ("content-type", "application/json"),
                ("cache-control", "no-store"),
            ],
            bytes,
        )
            .into_response(),
        Err(_) => error(StatusCode::BAD_GATEWAY, "Cannot read Pi Agent response"),
    }
}
