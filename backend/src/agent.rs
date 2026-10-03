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
    folder_picker: Mutex<()>,
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
            folder_picker: Mutex::new(()),
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
        if std::env::var("PI_NODE_RUN_AS_NODE").as_deref() == Ok("1") {
            command.env("ELECTRON_RUN_AS_NODE", "1");
        }
        let mut child = command
            .env("PI_BRIDGE_TOKEN", &self.token)
            .env("PI_GATEWAY_URL", &self.gateway_url)
            .env("PI_DEFAULT_CWD", cwd)
            .env(
                "PI_EXA_BASE_URL",
                std::env::var("EXA_BASE_URL").unwrap_or_else(|_| "https://api.exa.ai".into()),
            )
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
        let port = startup_port(&ready).ok_or("Invalid Pi Agent port")?;
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
    if path == "pick-directory" {
        if method != Method::POST {
            return error(
                StatusCode::METHOD_NOT_ALLOWED,
                "Use POST to choose a folder",
            );
        }
        if !peer.ip().is_loopback() {
            return error(StatusCode::FORBIDDEN, "请在本机打开网关以选择系统文件夹");
        }
        let Ok(_guard) = app.agent.folder_picker.try_lock() else {
            return error(StatusCode::CONFLICT, "文件夹选择窗口已经打开");
        };
        return choose_directory(&body, &app.agent.root).await;
    }
    let parts: Vec<_> = path.split('/').collect();
    if !allowed_route(path, &parts) {
        return error(StatusCode::NOT_FOUND, "Unknown Agent route");
    }
    let port = match app.agent.port().await {
        Ok(port) => port,
        Err(message) => return error(StatusCode::SERVICE_UNAVAILABLE, &message),
    };
    let gateway_key = app.gateway_key.read().await.clone();
    let key = agent_gateway_key(&app.token, &gateway_key);
    let response = match app
        .agent
        .client
        .request(method, format!("http://127.0.0.1:{port}/{path}"))
        .bearer_auth(&app.agent.token)
        .header("x-pi-gateway-key", key)
        .header("x-pi-exa-key", app.exa_key.read().await.as_str())
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

// The Agent forwards the key on every call, so rotating the gateway key takes effect immediately.
// Management key wins because Agent routes already require it.
fn agent_gateway_key<'a>(token: &'a str, gateway_key: &'a str) -> &'a str {
    if !token.is_empty() {
        token
    } else if !gateway_key.is_empty() {
        gateway_key
    } else {
        "local"
    }
}
// The Agent service only accepts these routes; anything else never reaches Node.
fn allowed_route(path: &str, parts: &[&str]) -> bool {
    if matches!(path, "status" | "sessions") {
        return true;
    }
    parts.first() == Some(&"sessions")
        && (2..=3).contains(&parts.len())
        && !parts[1].is_empty()
        && parts[1]
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        && (parts.len() == 2 || matches!(parts[2], "prompt" | "abort"))
}
// The bridge token authenticates Rust against a loopback-only Agent process.
fn startup_port(ready: &Value) -> Option<u16> {
    ready["port"]
        .as_u64()
        .and_then(|p| u16::try_from(p).ok())
        .filter(|p| *p != 0)
}
// osascript receives the directory as argv, so the fallback must be a real path.
fn initial_directory(body: &[u8], root: &std::path::Path) -> PathBuf {
    serde_json::from_slice::<Value>(body)
        .ok()
        .and_then(|value| value["cwd"].as_str().map(PathBuf::from))
        .filter(|path| path.is_absolute() && path.is_dir())
        .unwrap_or_else(|| root.to_path_buf())
}
async fn choose_directory(body: &[u8], root: &std::path::Path) -> Response {
    if !cfg!(target_os = "macos") {
        return error(
            StatusCode::NOT_IMPLEMENTED,
            "系统 Finder 选择目录仅支持 macOS，请手动输入路径",
        );
    }
    let initial = initial_directory(body, root);
    // Pass the directory as argv, never interpolate user paths into AppleScript.
    let script = r#"on run argv
      try
        tell application "Finder"
          activate
          set chosen to choose folder with prompt "选择 Pi Agent 工作目录" default location (POSIX file (item 1 of argv))
        end tell
        return POSIX path of chosen
      on error number -128
        return ""
      end try
    end run"#;
    let result = tokio::time::timeout(
        Duration::from_secs(300),
        Command::new("/usr/bin/osascript")
            .arg("-e")
            .arg(script)
            .arg(initial)
            .kill_on_drop(true)
            .output(),
    )
    .await;
    match result {
        Ok(Ok(output)) if output.status.success() => {
            let path = String::from_utf8_lossy(&output.stdout);
            let path = path.strip_suffix('\n').unwrap_or(&path);
            if path.is_empty() {
                return axum::Json(serde_json::json!({ "cwd": null, "cancelled": true }))
                    .into_response();
            }
            match tokio::fs::canonicalize(path).await {
                Ok(path) if path.is_dir() => {
                    axum::Json(serde_json::json!({ "cwd": path, "cancelled": false }))
                        .into_response()
                }
                _ => error(StatusCode::BAD_REQUEST, "所选文件夹不存在或无法访问"),
            }
        }
        Err(_) => error(StatusCode::REQUEST_TIMEOUT, "文件夹选择已超时，请重试"),
        _ => error(
            StatusCode::SERVICE_UNAVAILABLE,
            "无法打开 Finder 文件夹选择窗口，请检查系统自动化权限或手动输入路径",
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{updates, App, RwLock};
    use axum::{body::to_bytes, http::Uri};
    use std::sync::atomic::AtomicU64;

    fn app(root: PathBuf, token: &str, gateway_key: &str) -> Shared {
        let settings_file = root.join("settings.local.json");
        Shared::new(App {
            agent: Bridge::new(root.clone(), "http://127.0.0.1:1/v1".into()),
            client: reqwest::Client::new(),
            request_timeout: Duration::from_secs(1),
            updater: updates::Updater::new(
                root.join("update-settings.local.json"),
                root.join(".updates"),
            ),
            providers: RwLock::new(vec![]),
            exa_key: RwLock::new(String::new()),
            settings_file: settings_file.clone(),
            token: token.into(),
            gateway_key: RwLock::new(gateway_key.into()),
            gateway_key_file: root.join("gateway-key.local.txt"),
            requests: AtomicU64::new(0),
            fallbacks: AtomicU64::new(0),
            key_retries: AtomicU64::new(0),
        })
    }

    async fn call(
        app: Shared,
        peer: &str,
        path: &str,
        method: Method,
        headers: &[(&str, &str)],
        body: &[u8],
    ) -> (StatusCode, serde_json::Value) {
        let mut headers = headers
            .iter()
            .map(|(name, value)| (name.parse().unwrap(), value.parse().unwrap()))
            .collect::<HeaderMap>();
        headers
            .entry("x-gateway-settings")
            .or_insert("1".parse().unwrap());
        let response = proxy(
            State(app),
            ConnectInfo(peer.parse().unwrap()),
            OriginalUri(path.parse::<Uri>().unwrap()),
            method,
            headers,
            Bytes::copy_from_slice(body),
        )
        .await;
        let status = response.status();
        let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        )
    }

    #[test]
    fn allowed_routes_match_the_node_service_surface() {
        for path in [
            "status",
            "sessions",
            "sessions/abc",
            "sessions/abc/prompt",
            "sessions/abc/abort",
        ] {
            let parts: Vec<_> = path.split('/').collect();
            assert!(allowed_route(path, &parts), "{path} should be allowed");
        }
        for path in [
            "",
            "session",
            "sessions/",
            "sessions/abc/",
            "sessions/abc/unknown",
            "sessions/abc/prompt/extra",
            "sessions/../etc",
            "sessions/abc%2f",
            "pick-directory/extra",
            "internal",
        ] {
            let parts: Vec<_> = path.split('/').collect();
            assert!(!allowed_route(path, &parts), "{path} should be rejected");
        }
    }

    #[test]
    fn session_ids_reject_path_traversal() {
        let path = "sessions/../../etc/passwd";
        let parts: Vec<_> = path.split('/').collect();
        assert!(!allowed_route(path, &parts));
    }

    #[test]
    fn startup_port_accepts_only_real_ports() {
        assert_eq!(
            startup_port(&serde_json::json!({"port": 41234})),
            Some(41234)
        );
        assert_eq!(
            startup_port(&serde_json::json!({"port": 65535})),
            Some(65535)
        );
        assert_eq!(startup_port(&serde_json::json!({"port": 0})), None);
        assert_eq!(startup_port(&serde_json::json!({"port": 65536})), None);
        assert_eq!(startup_port(&serde_json::json!({"port": -1})), None);
        assert_eq!(startup_port(&serde_json::json!({"port": "41234"})), None);
        assert_eq!(startup_port(&serde_json::json!({"port": 41234.5})), None);
        assert_eq!(startup_port(&serde_json::json!({"port": null})), None);
        assert_eq!(startup_port(&serde_json::json!({})), None);
    }

    #[test]
    fn bridge_token_is_random_hex_per_bridge() {
        let first = Bridge::new(PathBuf::from("/tmp"), "http://127.0.0.1:1/v1".into());
        let second = Bridge::new(PathBuf::from("/tmp"), "http://127.0.0.1:1/v1".into());
        assert_eq!(first.token.len(), 64);
        assert!(first.token.bytes().all(|b| b.is_ascii_hexdigit()));
        assert_ne!(first.token, second.token);
    }

    #[test]
    fn agent_prefers_management_key_then_gateway_key() {
        assert_eq!(agent_gateway_key("management", "gateway"), "management");
        assert_eq!(agent_gateway_key("", "gateway"), "gateway");
        assert_eq!(agent_gateway_key("", ""), "local");
    }

    #[test]
    fn initial_directory_falls_back_to_root_for_unusable_input() {
        let root = PathBuf::from("/tmp");
        assert_eq!(initial_directory(b"{}", &root), root);
        assert_eq!(initial_directory(b"not json", &root), root);
        assert_eq!(
            initial_directory(br#"{"cwd":"relative/path"}"#, &root),
            root
        );
        assert_eq!(
            initial_directory(br#"{"cwd":"/does/not/exist"}"#, &root),
            root
        );
        assert_eq!(initial_directory(br#"{"cwd":123}"#, &root), root);
        assert_eq!(
            initial_directory(br#"{"cwd":"/tmp"}"#, &root),
            PathBuf::from("/tmp")
        );
    }

    #[tokio::test]
    async fn remote_access_without_management_key_is_refused() {
        let app = app(PathBuf::from("/tmp"), "", "");
        let (status, body) = call(
            app,
            "203.0.113.7:1234",
            "/api/agent/sessions",
            Method::GET,
            &[],
            b"",
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert!(body["error"]["message"]
            .as_str()
            .unwrap()
            .contains("GATEWAY_API_KEY"));
    }

    #[tokio::test]
    async fn wrong_management_key_is_unauthorized() {
        let app = app(PathBuf::from("/tmp"), "management-token", "");
        let (status, body) = call(
            app,
            "127.0.0.1:1234",
            "/api/agent/sessions",
            Method::GET,
            &[("authorization", "Bearer wrong")],
            b"",
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert!(body["error"]["message"]
            .as_str()
            .unwrap()
            .contains("management key"));
    }

    #[tokio::test]
    async fn settings_header_is_required() {
        let app = app(PathBuf::from("/tmp"), "management-token", "");
        let response = proxy(
            State(app.clone()),
            ConnectInfo("127.0.0.1:1234".parse().unwrap()),
            OriginalUri("/api/agent/sessions".parse::<Uri>().unwrap()),
            Method::GET,
            HeaderMap::from_iter([(
                "authorization".parse().unwrap(),
                "Bearer management-token".parse().unwrap(),
            )]),
            Bytes::new(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let body: Value = serde_json::from_slice(&bytes).unwrap();
        assert!(body["error"]["message"]
            .as_str()
            .unwrap()
            .contains("X-Gateway-Settings"));
    }

    #[tokio::test]
    async fn unknown_routes_are_rejected_before_reaching_the_agent() {
        let app = app(PathBuf::from("/tmp"), "", "");
        for path in ["/api/agent/internal", "/api/agent/sessions/../secrets"] {
            let (status, _) =
                call(app.clone(), "127.0.0.1:1234", path, Method::GET, &[], b"").await;
            assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
        }
    }

    #[tokio::test]
    async fn pick_directory_requires_post_and_loopback() {
        let app = app(PathBuf::from("/tmp"), "", "");
        let (status, body) = call(
            app.clone(),
            "127.0.0.1:1234",
            "/api/agent/pick-directory",
            Method::GET,
            &[],
            b"",
        )
        .await;
        assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
        assert!(body["error"]["message"].as_str().unwrap().contains("POST"));
    }

    #[tokio::test]
    async fn pick_directory_is_refused_for_remote_peers() {
        // A configured management key clears the first gate, so the folder
        // picker loopback check is the one that has to reject this request.
        let app = app(PathBuf::from("/tmp"), "management-token", "");
        let (status, body) = call(
            app.clone(),
            "203.0.113.7:1234",
            "/api/agent/pick-directory",
            Method::POST,
            &[("authorization", "Bearer management-token")],
            b"{}",
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert!(body["error"]["message"].as_str().unwrap().contains("本机"));
    }

    #[tokio::test]
    async fn missing_agent_dependencies_report_a_setup_hint() {
        let app = app(PathBuf::from("/nonexistent-root"), "", "");
        let (status, body) = call(
            app,
            "127.0.0.1:1234",
            "/api/agent/status",
            Method::GET,
            &[],
            b"",
        )
        .await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert!(body["error"]["message"]
            .as_str()
            .unwrap()
            .contains("npm ci --prefix agent"));
    }
}
