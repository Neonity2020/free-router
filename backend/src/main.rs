mod agent;
mod settings;
use settings::save_settings;
mod gateway;
use gateway::{chat, models};
mod cli;
mod config;
mod diagnostics;
mod keys;
mod reasoning;
use keys::{parse_keys, ApiKey};
mod updates;
use axum::{
    body::Body,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{any, get, post},
    Json, Router,
};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::{
    env,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::sync::RwLock;
use tower_http::services::{ServeDir, ServeFile};

#[derive(Clone)]
struct Provider {
    id: &'static str,
    base: String,
    keys: Vec<ApiKey>,
    cursor: Arc<AtomicU64>,
    model: &'static str,
}
fn prioritize(providers: &mut [Provider], preferred: &str) {
    providers.sort_by_key(|p| {
        (
            p.id != preferred,
            match p.id {
                "opencode" => 0,
                "openrouter" => 1,
                _ => 2,
            },
        )
    });
}
struct App {
    agent: agent::Bridge,
    client: reqwest::Client,
    request_timeout: Duration,
    updater: Arc<updates::Updater>,
    providers: RwLock<Vec<Provider>>,
    exa_key: RwLock<String>,
    settings_file: PathBuf,
    token: String,
    gateway_key: RwLock<String>,
    gateway_key_file: PathBuf,
    requests: AtomicU64,
    fallbacks: AtomicU64,
    key_retries: AtomicU64,
}
type Shared = Arc<App>;
fn error(status: StatusCode, message: &str) -> Response {
    (
        status,
        Json(json!({"error":{"message":message,"type":"gateway_error","code":status.as_u16()}})),
    )
        .into_response()
}
fn authorized(app: &App, headers: &HeaderMap) -> bool {
    app.token.is_empty()
        || headers
            .get("authorization")
            .and_then(|h| h.to_str().ok())
            .is_some_and(|h| h == format!("Bearer {}", app.token))
}
async fn api_authorized(app: &App, headers: &HeaderMap) -> bool {
    let key = app.gateway_key.read().await;
    if key.is_empty() {
        return authorized(app, headers);
    }
    headers
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .is_some_and(|h| {
            h == format!("Bearer {}", *key)
                || (!app.token.is_empty() && h == format!("Bearer {}", app.token))
        })
}
async fn gateway_key(State(app): State<Shared>, headers: HeaderMap) -> Response {
    if !authorized(&app, &headers) {
        return error(StatusCode::UNAUTHORIZED, "Invalid gateway management key");
    }
    if headers
        .get("x-gateway-settings")
        .and_then(|h| h.to_str().ok())
        != Some("1")
    {
        return error(StatusCode::FORBIDDEN, "Missing settings request header");
    }
    let mut response = Json(json!({"key": *app.gateway_key.read().await})).into_response();
    response
        .headers_mut()
        .insert("cache-control", "no-store".parse().unwrap());
    response
}
async fn generate_gateway_key(State(app): State<Shared>, headers: HeaderMap) -> Response {
    if !authorized(&app, &headers) {
        return error(StatusCode::UNAUTHORIZED, "Invalid gateway management key");
    }
    if headers
        .get("x-gateway-settings")
        .and_then(|h| h.to_str().ok())
        != Some("1")
    {
        return error(StatusCode::FORBIDDEN, "Missing settings request header");
    }
    let mut current = app.gateway_key.write().await;
    let mut random = [0u8; 32];
    if getrandom::getrandom(&mut random).is_err() {
        return error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Unable to generate API key",
        );
    }
    let key = format!(
        "fr_{}",
        random
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    );
    let tmp = app.gateway_key_file.with_extension("tmp");
    let persist = async {
        use tokio::io::AsyncWriteExt;
        let mut options = tokio::fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        options.mode(0o600);
        let mut file = options.open(&tmp).await?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            file.set_permissions(std::fs::Permissions::from_mode(0o600))
                .await?;
        }
        file.write_all(key.as_bytes()).await?;
        file.sync_all().await?;
        tokio::fs::rename(&tmp, &app.gateway_key_file).await
    }
    .await;
    if persist.is_err() {
        return error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Unable to save gateway API key",
        );
    }
    *current = key.clone();
    let mut response = Json(json!({"key":key})).into_response();
    response
        .headers_mut()
        .insert("cache-control", "no-store".parse().unwrap());
    response
}
async fn status(State(app): State<Shared>) -> Json<Value> {
    let providers = app.providers.read().await;
    Json(
        json!({"service":"Free Router","exa_configured":!app.exa_key.read().await.is_empty(),"auth_required":!app.token.is_empty() || !app.gateway_key.read().await.is_empty(),"management_auth_required":!app.token.is_empty(),"requests":app.requests.load(Ordering::Relaxed),"fallbacks":app.fallbacks.load(Ordering::Relaxed),"key_retries":app.key_retries.load(Ordering::Relaxed),"default_provider":providers[0].id,"providers":providers.iter().map(|p|json!({"id":p.id,"model":p.model,"configured":!p.keys.is_empty(),"key_count":p.keys.len(),"keys":p.keys.iter().enumerate().map(|(i,k)|json!({"id":k.id,"label":format!("Key {}",i+1),"retry_after_seconds":k.retry_after()})).collect::<Vec<_>>()})).collect::<Vec<_>>()}),
    )
}
// A custom header blocks cross-origin form submissions; gateway auth still applies.
#[tokio::main]
async fn main() {
    if env::var("FREE_ROUTER_DESKTOP").is_err() {
        let paths = config::resolve();
        dotenvy::from_path(paths.data_root.join(".env")).ok();
        dotenvy::from_path(paths.root.join(".env")).ok();
    }
    if let Some(code) = cli::run().await {
        std::process::exit(code);
    }
    let var = |name: &str, default: &str| env::var(name).unwrap_or_else(|_| default.to_owned());
    let mut providers = vec![
        Provider {
            id: "opencode",
            base: var("OPENCODE_BASE_URL", "https://opencode.ai/zen/v1"),
            keys: parse_keys(&json!(var("OPENCODE_API_KEY", "")))
                .expect("Invalid OPENCODE_API_KEY"),
            cursor: Arc::new(AtomicU64::new(0)),
            model: "space-bunny-free",
        },
        Provider {
            id: "openrouter",
            base: var("OPENROUTER_BASE_URL", "https://openrouter.ai/api/v1"),
            keys: parse_keys(&json!(var("OPENROUTER_API_KEY", "")))
                .expect("Invalid OPENROUTER_API_KEY"),
            cursor: Arc::new(AtomicU64::new(0)),
            model: "stealth/space-bunny-alpha",
        },
        Provider {
            id: "commandcode",
            base: var(
                "COMMANDCODE_BASE_URL",
                "https://api.commandcode.ai/provider/v1",
            ),
            keys: parse_keys(&json!(var("COMMANDCODE_API_KEY", "")))
                .expect("Invalid COMMANDCODE_API_KEY"),
            cursor: Arc::new(AtomicU64::new(0)),
            model: "stealth/space-bunny-alpha",
        },
    ];
    let preferred = var("DEFAULT_PROVIDER", "opencode");
    prioritize(&mut providers, &preferred);
    let paths = config::resolve();
    let _configuration_lock = paths.lock_for_server().unwrap_or_else(|message| {
        eprintln!("错误：{message}");
        std::process::exit(1);
    });
    let root = paths.root.clone();
    let settings_file = paths.settings_file.clone();
    let mut exa_key = var("EXA_API_KEY", "");
    if settings_file.exists() {
        let saved: Value = serde_json::from_slice(
            &std::fs::read(&settings_file).expect("Cannot read local settings"),
        )
        .expect("Invalid local settings JSON");
        if let Some(value) = saved.get("exa") {
            exa_key = value.as_str().unwrap_or("").to_owned();
        }
        for p in &mut providers {
            if let Some(value) = saved.get(p.id) {
                p.keys = parse_keys(value).expect("Invalid saved API key pool");
            }
        }
        if let Some(preferred) = saved.get("default_provider").and_then(Value::as_str) {
            prioritize(&mut providers, preferred);
        }
    }
    let data_root = &paths.data_root;
    let gateway_key_file = paths.gateway_key_file.clone();
    let saved_gateway_key = if gateway_key_file.exists() {
        std::fs::read_to_string(&gateway_key_file).expect("Cannot read gateway API key")
    } else {
        String::new()
    };
    let updater = updates::Updater::new(
        data_root.join("update-settings.local.json"),
        data_root.join(".updates"),
    );
    if env::var("FREE_ROUTER_DESKTOP").is_err() {
        updater.start();
    }
    let app = Arc::new(App {
        agent: agent::Bridge::new(
            root.clone(),
            format!(
                "http://{}:{}/v1",
                if var("HOST", "127.0.0.1") == "0.0.0.0" {
                    "127.0.0.1".to_owned()
                } else {
                    var("HOST", "127.0.0.1")
                },
                var("PORT", "8787")
            ),
        ),
        updater,
        request_timeout: Duration::from_secs(
            var("GATEWAY_REQUEST_TIMEOUT_SECS", "300")
                .parse::<u64>()
                .ok()
                .filter(|seconds| (1..=86400).contains(seconds))
                .expect("GATEWAY_REQUEST_TIMEOUT_SECS must be an integer between 1 and 86400"),
        ),
        gateway_key: RwLock::new(saved_gateway_key),
        gateway_key_file,
        client: reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(15))
            .timeout(Duration::from_secs(300))
            .build()
            .unwrap(),
        providers: RwLock::new(providers),
        exa_key: RwLock::new(exa_key),
        settings_file,
        token: var("GATEWAY_API_KEY", ""),
        requests: AtomicU64::new(0),
        fallbacks: AtomicU64::new(0),
        key_retries: AtomicU64::new(0),
    });
    let dist = root.join("frontend/dist");
    let router = Router::new()
        .route("/api/status", get(status))
        .route("/api/agent/{*path}", any(agent::proxy))
        .route(
            "/api/gateway-key",
            get(gateway_key).post(generate_gateway_key),
        )
        .route("/api/settings", post(save_settings))
        .route("/api/updates", get(updates::status))
        .route("/api/updates/settings", post(updates::configure))
        .route("/api/updates/check", post(updates::check))
        .route("/api/updates/download", post(updates::download))
        .route("/v1/models", get(models))
        .route("/v1/chat/completions", post(chat))
        .layer(axum::extract::DefaultBodyLimit::max(10 * 1024 * 1024))
        .fallback_service(
            ServeDir::new(&dist).not_found_service(ServeFile::new(dist.join("index.html"))),
        )
        .with_state(app.clone());
    let address = format!("{}:{}", var("HOST", "127.0.0.1"), var("PORT", "8787"));
    let listener = tokio::net::TcpListener::bind(&address)
        .await
        .expect("Cannot bind gateway address");
    println!("Free Router listening at http://{address} — API baseURL: http://{address}/v1");
    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(async {
        tokio::signal::ctrl_c().await.ok();
    })
    .await
    .unwrap();
    app.agent.shutdown().await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gateway::route;
    #[test]
    fn legacy_keys_and_validation() {
        assert_eq!(parse_keys(&json!("legacy-key")).unwrap().len(), 1);
        assert!(parse_keys(&json!("")).unwrap().is_empty());
        assert!(parse_keys(&Value::Null).unwrap().is_empty());
        assert_eq!(parse_keys(&json!(["a", "a", "b"])).unwrap().len(), 2);
        assert!(parse_keys(&json!([""])).is_err());
        assert!(parse_keys(&json!(["line\nkey"])).is_err());
        let normalized =
            parse_keys(&json!([" \u{200b}Bearer test-key\u{feff} ", "test-key"])).unwrap();
        assert_eq!(normalized.len(), 1);
        assert_eq!(normalized[0].secret, "test-key");
        assert!(parse_keys(&json!(["valid", "secret with space"]))
            .err()
            .unwrap()
            .contains("第 2 个"));
        assert!(parse_keys(&json!(["x".repeat(4097)]))
            .err()
            .unwrap()
            .contains("4096"));
    }
    #[test]
    fn model_routes() {
        assert_eq!(route("space-bunny"), Some(None));
        assert_eq!(route("stealth/space-bunny-alpha"), Some(Some("openrouter")));
        assert_eq!(route("space-bunny-free"), Some(Some("opencode")));
        assert_eq!(route("commandcode/space-bunny"), Some(Some("commandcode")));
        assert_eq!(
            route("commandcode/deepseek/deepseek-v4-flash"),
            Some(Some("commandcode"))
        );
        assert_eq!(route("commandcode/"), None);
        assert_eq!(route("commandcode/invalid model"), None);
        assert_eq!(
            route("openrouter/apodex/apodex-1.1-mini:free"),
            Some(Some("openrouter"))
        );
        assert_eq!(route("openrouter/"), None);
        assert_eq!(route("unknown"), None);
    }
}
