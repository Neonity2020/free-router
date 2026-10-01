mod updates;
use axum::{
    body::Body,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
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
    key: String,
    model: &'static str,
}
struct App {
    client: reqwest::Client,
    updater: Arc<updates::Updater>,
    providers: RwLock<Vec<Provider>>,
    settings_file: PathBuf,
    token: String,
    gateway_key: RwLock<String>,
    gateway_key_file: PathBuf,
    requests: AtomicU64,
    fallbacks: AtomicU64,
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
        json!({"service":"Free Router","auth_required":!app.token.is_empty() || !app.gateway_key.read().await.is_empty(),"management_auth_required":!app.token.is_empty(),"requests":app.requests.load(Ordering::Relaxed),"fallbacks":app.fallbacks.load(Ordering::Relaxed),"default_provider":providers[0].id,"providers":providers.iter().map(|p|json!({"id":p.id,"model":p.model,"configured":!p.key.is_empty()})).collect::<Vec<_>>()}),
    )
}
// A custom header blocks cross-origin form submissions; gateway auth still applies.
async fn save_settings(
    State(app): State<Shared>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    if !authorized(&app, &headers) {
        return error(StatusCode::UNAUTHORIZED, "Invalid gateway API key");
    }
    if headers
        .get("x-gateway-settings")
        .and_then(|v| v.to_str().ok())
        != Some("1")
    {
        return error(StatusCode::FORBIDDEN, "Missing settings request header");
    }
    let Some(fields) = body.as_object() else {
        return error(StatusCode::BAD_REQUEST, "Settings must be an object");
    };
    for (name, value) in fields {
        match name.as_str() {
            "openrouter" | "opencode" => {
                if !value.is_null()
                    && !value.as_str().is_some_and(|s| {
                        !s.trim().is_empty()
                            && s.len() <= 4096
                            && s.bytes().all(|b| (33..=126).contains(&b))
                    })
                {
                    return error(
                        StatusCode::BAD_REQUEST,
                        "API keys must be non-empty ASCII strings; use null to clear",
                    );
                }
            }
            "default_provider" if value == "openrouter" || value == "opencode" => {}
            _ => return error(StatusCode::BAD_REQUEST, "Unknown or invalid settings field"),
        }
    }
    let mut providers = app.providers.write().await;
    let mut next = providers.clone();
    for p in &mut next {
        if let Some(value) = body.get(p.id) {
            p.key = value.as_str().unwrap_or("").to_owned();
        }
    }
    if let Some(preferred) = body.get("default_provider").and_then(Value::as_str) {
        next.sort_by_key(|p| p.id != preferred);
    }
    let mut saved = json!({"default_provider":next[0].id});
    for p in &next {
        saved[p.id] = json!(p.key);
    }
    let tmp = app.settings_file.with_extension("json.tmp");
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
        file.write_all(saved.to_string().as_bytes()).await?;
        file.sync_all().await?;
        tokio::fs::rename(&tmp, &app.settings_file).await
    }
    .await;
    if persist.is_err() {
        return error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Unable to save settings to local file",
        );
    }
    *providers = next;
    Json(json!({"saved":true})).into_response()
}
async fn models(State(app): State<Shared>, headers: HeaderMap) -> Response {
    if !api_authorized(&app, &headers).await {
        return error(StatusCode::UNAUTHORIZED, "Invalid gateway API key");
    }
    Json(json!({"object":"list","data":(["space-bunny","openrouter/space-bunny","opencode/space-bunny"].iter().map(|id|json!({"id":id,"object":"model","created":0,"owned_by":"free-router"})).collect::<Vec<_>>())})).into_response()
}
fn route(model: &str) -> Option<Option<&'static str>> {
    match model {
        "space-bunny" => Some(None),
        "openrouter/space-bunny" | "stealth/space-bunny-alpha" => Some(Some("openrouter")),
        "opencode/space-bunny" | "space-bunny-free" => Some(Some("opencode")),
        _ => None,
    }
}
async fn chat(
    State(app): State<Shared>,
    headers: HeaderMap,
    Json(mut body): Json<Value>,
) -> Response {
    if !api_authorized(&app, &headers).await {
        return error(StatusCode::UNAUTHORIZED, "Invalid gateway API key");
    }
    let Some(selected) = body.get("model").and_then(Value::as_str).and_then(route) else {
        return error(
            StatusCode::BAD_REQUEST,
            "Unknown model. Use space-bunny, openrouter/space-bunny or opencode/space-bunny",
        );
    };
    if !body
        .get("messages")
        .is_some_and(|v| v.is_array() && !v.as_array().unwrap().is_empty())
    {
        return error(
            StatusCode::BAD_REQUEST,
            "messages must be a non-empty array",
        );
    }
    let providers = app.providers.read().await.clone();
    let candidates: Vec<_> = providers
        .iter()
        .filter(|p| !p.key.is_empty() && selected.is_none_or(|id| id == p.id))
        .collect();
    if candidates.is_empty() {
        return error(
            StatusCode::SERVICE_UNAVAILABLE,
            "No API key configured for this route. Save upstream keys in Settings.",
        );
    }
    app.requests.fetch_add(1, Ordering::Relaxed);
    let mut last = error(StatusCode::BAD_GATEWAY, "Upstream connection failed");
    for (i, p) in candidates.iter().enumerate() {
        if i > 0 {
            app.fallbacks.fetch_add(1, Ordering::Relaxed);
        }
        body["model"] = json!(p.model);
        let result = app
            .client
            .post(format!("{}/chat/completions", p.base.trim_end_matches('/')))
            .bearer_auth(&p.key)
            .json(&body)
            .send()
            .await;
        match result {
            Ok(upstream) => {
                let code = upstream.status();
                let retry = code.as_u16() == 429 || code.is_server_error();
                if retry && i + 1 < candidates.len() {
                    continue;
                }
                let content_type = upstream.headers().get("content-type").cloned();
                let mut response = Response::new(Body::from_stream(upstream.bytes_stream()));
                *response.status_mut() = code;
                if let Some(ct) = content_type {
                    response.headers_mut().insert("content-type", ct);
                }
                response
                    .headers_mut()
                    .insert("x-gateway-provider", p.id.parse().unwrap());
                response
                    .headers_mut()
                    .insert("cache-control", "no-cache".parse().unwrap());
                return response;
            }
            Err(e) => {
                last = error(
                    if e.is_timeout() {
                        StatusCode::GATEWAY_TIMEOUT
                    } else {
                        StatusCode::BAD_GATEWAY
                    },
                    "Unable to reach upstream provider",
                );
            }
        }
    }
    last
}
#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    dotenvy::from_filename("../.env").ok();
    let var = |name: &str, default: &str| env::var(name).unwrap_or_else(|_| default.to_owned());
    let mut providers = vec![
        Provider {
            id: "opencode",
            base: var("OPENCODE_BASE_URL", "https://opencode.ai/zen/v1"),
            key: var("OPENCODE_API_KEY", ""),
            model: "space-bunny-free",
        },
        Provider {
            id: "openrouter",
            base: var("OPENROUTER_BASE_URL", "https://openrouter.ai/api/v1"),
            key: var("OPENROUTER_API_KEY", ""),
            model: "stealth/space-bunny-alpha",
        },
    ];
    if var("DEFAULT_PROVIDER", "opencode") == "openrouter" {
        providers.reverse();
    }
    let executable_root = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()));
    let root = executable_root
        .filter(|p| p.join("frontend/dist").exists())
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".."));
    let settings_file = env::var("SETTINGS_FILE")
        .map(PathBuf::from)
        .unwrap_or_else(|_| root.join("settings.local.json"));
    if settings_file.exists() {
        let saved: Value = serde_json::from_slice(
            &std::fs::read(&settings_file).expect("Cannot read local settings"),
        )
        .expect("Invalid local settings JSON");
        for p in &mut providers {
            if let Some(key) = saved.get(p.id).and_then(Value::as_str) {
                p.key = key.to_owned();
            }
        }
        if let Some(preferred) = saved.get("default_provider").and_then(Value::as_str) {
            providers.sort_by_key(|p| p.id != preferred);
        }
    }
    let data_root = settings_file.parent().unwrap_or(&root);
    let gateway_key_file = data_root.join("gateway-key.local.txt");
    let saved_gateway_key = if gateway_key_file.exists() {
        std::fs::read_to_string(&gateway_key_file).expect("Cannot read gateway API key")
    } else {
        String::new()
    };
    let updater = updates::Updater::new(
        data_root.join("update-settings.local.json"),
        data_root.join(".updates"),
    );
    updater.start();
    let app = Arc::new(App {
        updater,
        gateway_key: RwLock::new(saved_gateway_key),
        gateway_key_file,
        client: reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(15))
            .timeout(Duration::from_secs(300))
            .build()
            .unwrap(),
        providers: RwLock::new(providers),
        settings_file,
        token: var("GATEWAY_API_KEY", ""),
        requests: AtomicU64::new(0),
        fallbacks: AtomicU64::new(0),
    });
    let dist = root.join("frontend/dist");
    let router = Router::new()
        .route("/api/status", get(status))
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
        .with_state(app);
    let address = format!("{}:{}", var("HOST", "127.0.0.1"), var("PORT", "8787"));
    let listener = tokio::net::TcpListener::bind(&address)
        .await
        .expect("Cannot bind gateway address");
    println!("Free Router listening at http://{address} — API baseURL: http://{address}/v1");
    axum::serve(listener, router)
        .with_graceful_shutdown(async {
            tokio::signal::ctrl_c().await.ok();
        })
        .await
        .unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn model_routes() {
        assert_eq!(route("space-bunny"), Some(None));
        assert_eq!(route("stealth/space-bunny-alpha"), Some(Some("openrouter")));
        assert_eq!(route("space-bunny-free"), Some(Some("opencode")));
        assert_eq!(route("unknown"), None);
    }
}
