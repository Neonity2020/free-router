use super::{authorized, error, Shared};
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::{io::AsyncWriteExt, sync::Mutex};

const MAX_PACKAGE: u64 = 150 * 1024 * 1024;
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub repository: String,
    pub auto_download: bool,
}
#[derive(Clone, Serialize, Deserialize)]
struct Release {
    version: String,
    url: String,
    sha256: String,
    size: u64,
    asset: String,
}
#[derive(Clone, Serialize)]
struct Snapshot {
    current_version: &'static str,
    platform: String,
    config: Config,
    phase: String,
    latest_version: Option<String>,
    checked_at: Option<u64>,
    downloaded: u64,
    total: u64,
    package_path: Option<String>,
    error: Option<String>,
}
pub struct Updater {
    client: reqwest::Client,
    state: Mutex<Snapshot>,
    operation: Arc<Mutex<()>>,
    config_file: PathBuf,
    directory: PathBuf,
}
fn platform() -> String {
    match (std::env::consts::ARCH, std::env::consts::OS) {
        ("aarch64", "macos") => "aarch64-apple-darwin",
        ("x86_64", "macos") => "x86_64-apple-darwin",
        ("x86_64", "linux") => "x86_64-unknown-linux-gnu",
        _ => "unsupported",
    }
    .to_owned()
}
fn valid_repo(repo: &str) -> bool {
    let parts: Vec<_> = repo.split('/').collect();
    parts.len() == 2
        && parts.iter().all(|s| {
            !s.is_empty()
                && *s != "."
                && *s != ".."
                && s.len() <= 100
                && s.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
        })
}
fn valid_digest(digest: &str) -> bool {
    digest.len() == 64 && digest.bytes().all(|b| b.is_ascii_hexdigit())
}
fn select_release(data: &Value, repository: &str, target: &str) -> Result<Option<Release>, String> {
    let version = data["tag_name"].as_str().ok_or("Release 缺少版本号")?;
    let version = version.strip_prefix('v').unwrap_or(version);
    let parsed =
        semver::Version::parse(version).map_err(|_| "Release tag 必须是语义版本，例如 v0.2.0")?;
    if data["draft"] == true || data["prerelease"] == true || !parsed.pre.is_empty() {
        return Err("仅支持正式版本".into());
    }
    if parsed <= semver::Version::parse(env!("CARGO_PKG_VERSION")).unwrap() {
        return Ok(None);
    }
    let asset_name = format!("free-router-{target}.tar.gz");
    let asset = data["assets"]
        .as_array()
        .ok_or("Release 缺少更新包")?
        .iter()
        .find(|a| a["name"] == asset_name)
        .ok_or("该版本没有当前平台的更新包")?;
    let sha = asset["digest"]
        .as_str()
        .and_then(|d| d.strip_prefix("sha256:"))
        .filter(|d| valid_digest(d))
        .ok_or("更新包缺少 GitHub SHA-256 digest，无法验证下载")?;
    let url = asset["browser_download_url"]
        .as_str()
        .ok_or("更新包缺少下载地址")?;
    let parsed_url = reqwest::Url::parse(url).map_err(|_| "下载地址无效")?;
    let prefix = format!("/{repository}/releases/download/");
    if parsed_url.scheme() != "https"
        || parsed_url.host_str() != Some("github.com")
        || !parsed_url.path().starts_with(&prefix)
        || !parsed_url.username().is_empty()
        || parsed_url.password().is_some()
        || parsed_url.port().is_some()
    {
        return Err("更新包必须来自配置的 GitHub 仓库".into());
    }
    let size = asset["size"]
        .as_u64()
        .filter(|s| *s > 0 && *s <= MAX_PACKAGE)
        .ok_or("更新包大小无效或超过 150 MiB")?;
    Ok(Some(Release {
        version: parsed.to_string(),
        url: url.to_owned(),
        sha256: sha.to_lowercase(),
        size,
        asset: asset_name,
    }))
}
impl Updater {
    pub fn new(config_file: PathBuf, directory: PathBuf) -> Arc<Self> {
        let config = if config_file.exists() {
            serde_json::from_slice::<Config>(
                &std::fs::read(&config_file).expect("Cannot read update settings"),
            )
            .expect("Invalid update settings")
        } else {
            Config {
                repository: String::new(),
                auto_download: true,
            }
        };
        assert!(
            config.repository.is_empty() || valid_repo(&config.repository),
            "Invalid update repository"
        );
        let client = reqwest::Client::builder()
            .user_agent("free-router-updater")
            .connect_timeout(Duration::from_secs(15))
            .timeout(Duration::from_secs(300))
            .redirect(reqwest::redirect::Policy::custom(|attempt| {
                let url = attempt.url();
                if attempt.previous().len() >= 5 {
                    return attempt.error("Too many redirects");
                }
                if url.scheme() == "https"
                    && matches!(
                        url.host_str(),
                        Some(
                            "github.com"
                                | "release-assets.githubusercontent.com"
                                | "objects.githubusercontent.com"
                        )
                    )
                {
                    attempt.follow()
                } else {
                    attempt.error("Untrusted update redirect")
                }
            }))
            .build()
            .unwrap();
        Arc::new(Self {
            client,
            state: Mutex::new(Snapshot {
                current_version: env!("CARGO_PKG_VERSION"),
                platform: platform(),
                config,
                phase: "idle".into(),
                latest_version: None,
                checked_at: None,
                downloaded: 0,
                total: 0,
                package_path: None,
                error: None,
            }),
            operation: Arc::new(Mutex::new(())),
            config_file,
            directory,
        })
    }
    pub fn start(self: &Arc<Self>) {
        let this = self.clone();
        tokio::spawn(async move {
            let mut timer = tokio::time::interval(Duration::from_secs(6 * 60 * 60));
            loop {
                timer.tick().await;
                this.launch(false).await.ok();
            }
        });
    }
    async fn launch(self: &Arc<Self>, download: bool) -> Result<(), String> {
        let guard = self
            .operation
            .clone()
            .try_lock_owned()
            .map_err(|_| "更新任务正在运行")?;
        if self.state.lock().await.config.repository.is_empty() {
            return Err("请先保存 GitHub 仓库地址".into());
        }
        let this = self.clone();
        tokio::spawn(async move {
            let _guard = guard;
            if let Err(message) = this.check(download).await {
                let mut state = this.state.lock().await;
                state.phase = "error".into();
                state.error = Some(message);
            }
        });
        Ok(())
    }
    async fn check(&self, force_download: bool) -> Result<(), String> {
        let config = {
            let mut state = self.state.lock().await;
            state.phase = "checking".into();
            state.error = None;
            state.checked_at = Some(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
            );
            state.package_path = None;
            state.config.clone()
        };
        let response = self
            .client
            .get(format!(
                "https://api.github.com/repos/{}/releases/latest",
                config.repository
            ))
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .send()
            .await
            .map_err(|_| "无法连接 GitHub，请稍后重试")?;
        let code = response.status();
        if !code.is_success() {
            return Err(match code.as_u16() {
                404 => "仓库不存在、非公开或尚无正式 Release".into(),
                403 | 429 => "GitHub 请求限流，请稍后重试".into(),
                _ => format!("GitHub 返回 HTTP {code}"),
            });
        }
        let mut bytes = Vec::new();
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|_| "读取版本信息失败")?;
            if bytes.len() + chunk.len() > 2 * 1024 * 1024 {
                return Err("版本信息过大".into());
            }
            bytes.extend_from_slice(&chunk);
        }
        let data: Value = serde_json::from_slice(&bytes).map_err(|_| "GitHub 返回无效版本信息")?;
        let target = self.state.lock().await.platform.clone();
        let release = select_release(&data, &config.repository, &target)?;
        {
            let mut state = self.state.lock().await;
            state.checked_at = Some(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
            );
            state.latest_version = release.as_ref().map(|r| r.version.clone());
            state.phase = if release.is_some() {
                "available"
            } else {
                "up_to_date"
            }
            .into();
            state.package_path = None;
            state.downloaded = 0;
            state.total = 0;
        }
        if let Some(release) = release {
            if config.auto_download || force_download {
                self.download(&release).await?;
            }
        }
        Ok(())
    }
    async fn download(&self, release: &Release) -> Result<(), String> {
        tokio::fs::create_dir_all(&self.directory)
            .await
            .map_err(|_| "无法创建更新目录")?;
        let final_path = self
            .directory
            .join(format!("{}-{}", release.version, release.asset));
        // Reuse a verified package after restart, without another network transfer.
        if let Ok(mut existing) = tokio::fs::File::open(&final_path).await {
            use tokio::io::AsyncReadExt;
            let mut hash = Sha256::new();
            let mut buffer = vec![0; 65536];
            let mut size = 0;
            loop {
                let n = existing
                    .read(&mut buffer)
                    .await
                    .map_err(|_| "读取已下载更新包失败")?;
                if n == 0 {
                    break;
                }
                size += n as u64;
                if size > MAX_PACKAGE {
                    break;
                }
                hash.update(&buffer[..n]);
            }
            if size == release.size && format!("{:x}", hash.finalize()) == release.sha256 {
                self.ready(&final_path, release.size).await;
                return Ok(());
            }
        }
        tokio::fs::remove_file(&final_path).await.ok();
        self.state.lock().await.package_path = None;
        let part = final_path.with_extension("gz.part");
        let result = async {
            {
                let mut state = self.state.lock().await;
                state.phase = "downloading".into();
                state.total = release.size;
                state.downloaded = 0;
            }
            let response = self
                .client
                .get(&release.url)
                .send()
                .await
                .map_err(|_| "更新下载连接失败")?
                .error_for_status()
                .map_err(|_| "更新包下载失败")?;
            let mut file = tokio::fs::File::create(&part)
                .await
                .map_err(|_| "无法创建更新临时文件")?;
            let mut hash = Sha256::new();
            let mut size = 0;
            let mut stream = response.bytes_stream();
            while let Some(chunk) = stream.next().await {
                let chunk = chunk.map_err(|_| "下载中断，请重试")?;
                size += chunk.len() as u64;
                if size > release.size || size > MAX_PACKAGE {
                    return Err("更新包大小与发布信息不一致".to_owned());
                }
                file.write_all(&chunk).await.map_err(|_| "写入更新包失败")?;
                hash.update(&chunk);
                self.state.lock().await.downloaded = size;
            }
            if size != release.size || format!("{:x}", hash.finalize()) != release.sha256 {
                return Err("SHA-256 校验失败，更新包未保留".to_owned());
            }
            file.sync_all().await.map_err(|_| "保存更新包失败")?;
            drop(file);
            tokio::fs::rename(&part, &final_path)
                .await
                .map_err(|_| "完成更新包保存失败")?;
            self.ready(&final_path, size).await;
            Ok(())
        }
        .await;
        if result.is_err() {
            tokio::fs::remove_file(&part).await.ok();
        }
        result
    }
    async fn ready(&self, path: &std::path::Path, size: u64) {
        let mut state = self.state.lock().await;
        state.phase = "ready".into();
        state.downloaded = size;
        state.total = size;
        state.package_path = Some(path.to_string_lossy().into());
    }
}
fn mutation_allowed(app: &super::App, headers: &HeaderMap) -> bool {
    authorized(app, headers)
        && headers
            .get("x-gateway-settings")
            .and_then(|v| v.to_str().ok())
            == Some("1")
}
pub async fn status(State(app): State<Shared>, headers: HeaderMap) -> Response {
    if !authorized(&app, &headers) {
        return error(StatusCode::UNAUTHORIZED, "Invalid gateway API key");
    }
    let state = app.updater.state.lock().await.clone();
    let mut response = Json(state).into_response();
    response
        .headers_mut()
        .insert("cache-control", "no-store".parse().unwrap());
    response
}
pub async fn configure(
    State(app): State<Shared>,
    headers: HeaderMap,
    Json(config): Json<Config>,
) -> Response {
    if !mutation_allowed(&app, &headers) {
        return error(StatusCode::FORBIDDEN, "Settings authorization required");
    }
    if !config.repository.is_empty() && !valid_repo(&config.repository) {
        return error(StatusCode::BAD_REQUEST, "仓库格式应为 owner/repo");
    }
    let Ok(_guard) = app.updater.operation.try_lock() else {
        return error(StatusCode::CONFLICT, "更新任务正在运行，请完成后修改配置");
    };
    let tmp = app.updater.config_file.with_extension("json.tmp");
    let persist = async {
        tokio::fs::write(&tmp, serde_json::to_vec(&config).unwrap()).await?;
        tokio::fs::rename(&tmp, &app.updater.config_file).await
    }
    .await;
    if persist.is_err() {
        return error(StatusCode::INTERNAL_SERVER_ERROR, "无法保存更新配置");
    }
    {
        let mut state = app.updater.state.lock().await;
        state.config = config;
        state.phase = "idle".into();
        state.latest_version = None;
        state.package_path = None;
        state.error = None;
        state.checked_at = None;
        state.downloaded = 0;
        state.total = 0;
    }
    drop(_guard);
    app.updater.launch(false).await.ok();
    Json(json!({"saved":true})).into_response()
}
pub async fn check(State(app): State<Shared>, headers: HeaderMap) -> Response {
    trigger(app, headers, false).await
}
pub async fn download(State(app): State<Shared>, headers: HeaderMap) -> Response {
    trigger(app, headers, true).await
}
async fn trigger(app: Shared, headers: HeaderMap, download: bool) -> Response {
    if !mutation_allowed(&app, &headers) {
        return error(StatusCode::FORBIDDEN, "Settings authorization required");
    }
    match app.updater.launch(download).await {
        Ok(()) => (StatusCode::ACCEPTED, Json(json!({"started":true}))).into_response(),
        Err(e) => error(StatusCode::CONFLICT, &e),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn release(version: &str) -> Value {
        json!({"tag_name":version,"draft":false,"prerelease":false,"assets":[{"name":"free-router-aarch64-apple-darwin.tar.gz","size":10,"digest":format!("sha256:{}","a".repeat(64)),"browser_download_url":"https://github.com/example/free-router/releases/download/v0.2.0/free-router-aarch64-apple-darwin.tar.gz"}]})
    }
    #[test]
    fn repository_validation() {
        assert!(valid_repo("example/free-router"));
        for bad in [
            "https://github.com/a/b",
            "a/b/c",
            "../repo",
            "a/..",
            "a/b?x",
        ] {
            assert!(!valid_repo(bad));
        }
    }
    #[test]
    fn semantic_versions_and_assets() {
        assert!(select_release(
            &release("v0.10.0"),
            "example/free-router",
            "aarch64-apple-darwin"
        )
        .unwrap()
        .is_some());
        assert!(select_release(
            &release("v0.1.0"),
            "example/free-router",
            "aarch64-apple-darwin"
        )
        .unwrap()
        .is_none());
        assert!(select_release(
            &release("v0.2.0-beta.1"),
            "example/free-router",
            "aarch64-apple-darwin"
        )
        .is_err());
        assert!(select_release(&release("v0.2.0"), "example/free-router", "unsupported").is_err());
    }
    #[test]
    fn refuses_unverified_or_foreign_assets() {
        let mut r = release("v0.2.0");
        r["assets"][0]["digest"] = Value::Null;
        assert!(select_release(&r, "example/free-router", "aarch64-apple-darwin").is_err());
        let r = release("v0.2.0");
        assert!(select_release(&r, "other/repository", "aarch64-apple-darwin").is_err());
    }
    #[tokio::test]
    async fn corrupted_existing_package_is_not_ready() {
        let root =
            std::env::temp_dir().join(format!("free-router-update-test-{}", std::process::id()));
        tokio::fs::create_dir_all(&root).await.unwrap();
        let updater = Updater::new(root.join("config.json"), root.clone());
        let release = Release {
            version: "0.2.0".into(),
            asset: "test.tar.gz".into(),
            size: 3,
            sha256: format!("{:x}", Sha256::digest(b"abc")),
            url: "http://127.0.0.1:1/unreachable".into(),
        };
        tokio::fs::write(root.join("0.2.0-test.tar.gz"), b"abc")
            .await
            .unwrap();
        updater.download(&release).await.unwrap();
        assert_eq!(updater.state.lock().await.phase, "ready");
        tokio::fs::write(root.join("0.2.0-test.tar.gz"), b"bad")
            .await
            .unwrap();
        assert!(updater.download(&release).await.is_err());
        assert!(!root.join("0.2.0-test.tar.gz.part").exists());
        assert!(updater.state.lock().await.package_path.is_none());
        tokio::fs::remove_dir_all(root).await.unwrap();
    }
    #[tokio::test]
    async fn streams_download_and_rejects_corruption() {
        use axum::{routing::get, Router};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(
                listener,
                Router::new().route("/package", get(|| async { "package-content" })),
            )
            .await
            .unwrap();
        });
        let root =
            std::env::temp_dir().join(format!("free-router-stream-test-{}", std::process::id()));
        tokio::fs::create_dir_all(&root).await.unwrap();
        let updater = Updater::new(root.join("config.json"), root.clone());
        let mut release = Release {
            version: "0.2.0".into(),
            asset: "test.tar.gz".into(),
            size: 15,
            sha256: format!("{:x}", Sha256::digest(b"package-content")),
            url: format!("http://{address}/package"),
        };
        updater.download(&release).await.unwrap();
        assert_eq!(
            tokio::fs::read(root.join("0.2.0-test.tar.gz"))
                .await
                .unwrap(),
            b"package-content"
        );
        assert_eq!(updater.state.lock().await.downloaded, 15);
        release.sha256 = "0".repeat(64);
        assert!(updater
            .download(&release)
            .await
            .unwrap_err()
            .contains("SHA-256"));
        assert!(!root.join("0.2.0-test.tar.gz").exists());
        assert!(!root.join("0.2.0-test.tar.gz.part").exists());
        release.size = 1;
        assert!(updater
            .download(&release)
            .await
            .unwrap_err()
            .contains("大小"));
        assert!(!root.join("0.2.0-test.tar.gz.part").exists());
        server.abort();
        tokio::fs::remove_dir_all(root).await.unwrap();
    }
}
