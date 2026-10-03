use std::{env, io::Write, path::Path, time::Duration};

use clap::{Parser, Subcommand};
use serde_json::{json, Map, Value};

use crate::config;

const PROVIDERS: [&str; 3] = ["opencode", "openrouter", "commandcode"];

#[derive(Parser)]
#[command(name = "free-router", version, about = "本地模型网关与 Pi 编码代理")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// 启动网关服务（默认命令）
    Serve,
    /// 查看网关运行状态
    Status {
        /// 以 JSON 输出
        #[arg(long)]
        json: bool,
    },
    /// 管理上游 API Key
    #[command(subcommand)]
    Keys(KeysCommand),
    /// 查看或切换默认上游
    Provider {
        /// opencode、openrouter 或 commandcode；省略则只查看
        id: Option<String>,
    },
    /// 管理本地网关访问密钥
    #[command(subcommand)]
    GatewayKey(GatewayKeyCommand),
    /// 显示解析后的配置路径
    Config,
}

#[derive(Subcommand)]
enum KeysCommand {
    /// 列出 Key（默认列出全部上游）
    List {
        provider: Option<String>,
        /// 以 JSON 输出
        #[arg(long)]
        json: bool,
    },
    /// 追加一个或多个 Key
    Add {
        provider: String,
        #[arg(required = true)]
        keys: Vec<String>,
    },
    /// 按 Key ID 或唯一前缀移除
    Remove {
        provider: String,
        #[arg(required = true)]
        ids: Vec<String>,
    },
    /// 清空某个上游的全部 Key
    Clear { provider: String },
}

#[derive(Subcommand)]
enum GatewayKeyCommand {
    /// 显示当前网关密钥
    Show,
    /// 生成并保存新密钥（旧密钥立即失效）
    Generate,
}

/// Returns the process exit code, or `None` to fall through to `serve`.
pub(crate) async fn run() -> Option<i32> {
    let command = Cli::parse().command?;
    let result = match command {
        Command::Serve => return None,
        Command::Status { json } => status(json).await,
        Command::Keys(command) => keys(command).await,
        Command::Provider { id } => provider(id).await,
        Command::GatewayKey(command) => gateway_key(command).await,
        Command::Config => config_info(),
    };
    Some(match result {
        Ok(()) => 0,
        Err(message) => {
            eprintln!("错误：{message}");
            1
        }
    })
}

struct KeyView {
    id: String,
    label: String,
}

struct ProviderView {
    id: &'static str,
    key_count: usize,
    keys: Vec<KeyView>,
}

fn valid_provider(id: &str) -> Result<&'static str, String> {
    PROVIDERS
        .iter()
        .find(|provider| **provider == id)
        .copied()
        .ok_or_else(|| format!("未知上游 “{id}”，可选：{}", PROVIDERS.join("、")))
}

fn base_url() -> String {
    let host = env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_owned());
    let host = if host == "0.0.0.0" {
        "127.0.0.1".to_owned()
    } else {
        host
    };
    let port = env::var("PORT").unwrap_or_else(|_| "8787".to_owned());
    format!("http://{host}:{port}")
}

fn http() -> reqwest::Client {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_millis(500))
        .timeout(Duration::from_secs(15))
        .build()
        .expect("Cannot build HTTP client")
}

fn management_token() -> String {
    env::var("GATEWAY_API_KEY").unwrap_or_default()
}

enum PostOutcome {
    Ok(Value),
    Failed(String),
}

async fn api_post(client: &reqwest::Client, path: &str, body: Value) -> PostOutcome {
    let mut request = client
        .post(format!("{}{path}", base_url()))
        .header("X-Gateway-Settings", "1")
        .json(&body);
    let token = management_token();
    if !token.is_empty() {
        request = request.bearer_auth(token);
    }
    let response = match request.send().await {
        Ok(response) => response,
        Err(_) => {
            return PostOutcome::Failed("管理请求失败；未修改本地配置，请检查网关连接后重试".into())
        }
    };
    let status = response.status();
    let value = response.json::<Value>().await.unwrap_or(Value::Null);
    if status.is_success() {
        PostOutcome::Ok(value)
    } else if matches!(status.as_u16(), 401 | 403) {
        PostOutcome::Failed("管理鉴权失败；请设置正确的 GATEWAY_API_KEY，未修改本地配置".into())
    } else {
        PostOutcome::Failed(
            value
                .get("error")
                .and_then(|error| error.get("message"))
                .and_then(Value::as_str)
                .map(str::to_owned)
                .unwrap_or_else(|| format!("网关返回 {status}")),
        )
    }
}

async fn gateway_status(client: &reqwest::Client) -> Result<Option<Value>, String> {
    let response = match client
        .get(format!("{}/api/status", base_url()))
        .send()
        .await
    {
        Ok(response) => response,
        Err(error) if error.is_connect() => return Ok(None),
        Err(_) => return Err("无法确定网关状态；请检查连接后重试".into()),
    };
    if !response.status().is_success() {
        return Err(format!(
            "网关状态接口返回 {}，未修改本地配置",
            response.status()
        ));
    }
    let status: Value = response
        .json()
        .await
        .map_err(|_| "网关状态接口返回无效 JSON")?;
    if status.get("service").and_then(Value::as_str) != Some("Free Router")
        || !status.get("providers").is_some_and(Value::is_array)
    {
        return Err("目标端口不是 Free Router 网关，未修改本地配置".into());
    }
    Ok(Some(status))
}

fn read_settings(paths: &config::Paths) -> Result<Value, String> {
    let bytes = match std::fs::read(&paths.settings_file) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => b"{}".to_vec(),
        Err(error) => {
            return Err(format!(
                "无法读取 {}：{error}",
                paths.settings_file.display()
            ))
        }
    };
    let mut value: Value = serde_json::from_slice(&bytes)
        .map_err(|_| format!("{} 不是有效的 JSON", paths.settings_file.display()))?;
    let object = value
        .as_object_mut()
        .ok_or_else(|| format!("{} 不是 JSON 对象", paths.settings_file.display()))?;
    for (field, variable) in [
        ("opencode", "OPENCODE_API_KEY"),
        ("openrouter", "OPENROUTER_API_KEY"),
        ("commandcode", "COMMANDCODE_API_KEY"),
        ("exa", "EXA_API_KEY"),
        ("default_provider", "DEFAULT_PROVIDER"),
    ] {
        if !object.contains_key(field) {
            if let Ok(raw) = env::var(variable) {
                object.insert(field.to_owned(), json!(raw));
            }
        }
    }
    Ok(value)
}

fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut random = [0u8; 16];
    getrandom::getrandom(&mut random).map_err(|error| std::io::Error::other(error.to_string()))?;
    let suffix: String = random.iter().map(|byte| format!("{byte:02x}")).collect();
    let temporary = path.with_file_name(format!(
        ".{}.{}.tmp",
        path.file_name().unwrap_or_default().to_string_lossy(),
        suffix
    ));
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&temporary)?;
    let result = (|| {
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&temporary, path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}

fn write_settings(paths: &config::Paths, value: &Value) -> Result<(), String> {
    write_private(&paths.settings_file, value.to_string().as_bytes())
        .map_err(|error| format!("无法保存 {}：{error}", paths.settings_file.display()))
}

/// Persist a provider/key change, preferring the live gateway so it takes effect immediately.
async fn mutate(
    client: &reqwest::Client,
    paths: &config::Paths,
    body: Value,
    apply: impl FnOnce(&mut Value) -> Result<(), String>,
) -> Result<(), String> {
    let live = gateway_status(client).await?.is_some();
    if live {
        match api_post(client, "/api/settings", body).await {
            PostOutcome::Ok(_) => return Ok(()),
            PostOutcome::Failed(message) => return Err(message),
        }
    }
    let _guard = paths.lock_for_cli().await?;
    let mut settings = read_settings(paths)?;
    apply(&mut settings)?;
    write_settings(paths, &settings)?;
    eprintln!("提示：网关未运行，已写入配置文件，启动网关后生效。");
    Ok(())
}

fn view_from_status(status: &Value) -> Vec<ProviderView> {
    PROVIDERS
        .iter()
        .map(|id| {
            let entry = status
                .get("providers")
                .and_then(Value::as_array)
                .and_then(|providers| {
                    providers
                        .iter()
                        .find(|provider| provider.get("id").and_then(Value::as_str) == Some(*id))
                });
            let keys: Vec<KeyView> = entry
                .and_then(|provider| provider.get("keys"))
                .and_then(Value::as_array)
                .map(|keys| {
                    keys.iter()
                        .filter_map(|key| {
                            Some(KeyView {
                                id: key.get("id")?.as_str()?.to_owned(),
                                label: key
                                    .get("label")
                                    .and_then(Value::as_str)
                                    .unwrap_or("Key")
                                    .to_owned(),
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();
            ProviderView {
                id,
                key_count: keys.len(),
                keys,
            }
        })
        .collect()
}

fn view_from_settings(settings: &Value) -> Result<Vec<ProviderView>, String> {
    PROVIDERS
        .iter()
        .map(|id| {
            let keys = crate::parse_keys(settings.get(*id).unwrap_or(&Value::Null))
                .map_err(|message| format!("{id}：{message}"))?;
            let keys: Vec<KeyView> = keys
                .into_iter()
                .enumerate()
                .map(|(index, key)| KeyView {
                    id: key.id,
                    label: format!("Key {}", index + 1),
                })
                .collect();
            Ok(ProviderView {
                id,
                key_count: keys.len(),
                keys,
            })
        })
        .collect()
}

fn views_json(views: &[ProviderView]) -> Value {
    Value::Array(
        views
            .iter()
            .map(|view| {
                json!({
                    "id": view.id,
                    "configured": view.key_count > 0,
                    "key_count": view.key_count,
                    "keys": view.keys.iter().map(|key| json!({"id": key.id, "label": key.label})).collect::<Vec<_>>(),
                })
            })
            .collect(),
    )
}

fn print_provider_lines(views: &[ProviderView]) {
    for view in views {
        if view.key_count > 0 {
            println!("  {:<12} {} 个 Key", view.id, view.key_count);
        } else {
            println!("  {:<12} 未配置", view.id);
        }
    }
}

fn resolve_ids(keys: &[KeyView], prefixes: &[String]) -> Result<Vec<String>, String> {
    let mut resolved = Vec::new();
    for prefix in prefixes {
        let needle = prefix.trim().to_ascii_lowercase();
        let needle = needle.strip_prefix("key_").unwrap_or(&needle);
        if needle.is_empty() {
            return Err("Key ID 不能为空".into());
        }
        let matches: Vec<&KeyView> = keys
            .iter()
            .filter(|key| {
                key.id
                    .to_ascii_lowercase()
                    .strip_prefix("key_")
                    .is_some_and(|id| id.starts_with(needle))
            })
            .collect();
        match matches.as_slice() {
            [] => return Err(format!("找不到匹配 “{prefix}” 的 Key")),
            [only] => {
                if !resolved.contains(&only.id) {
                    resolved.push(only.id.clone());
                }
            }
            _ => {
                return Err(format!(
                    "“{prefix}” 匹配到 {} 个 Key，请提供更长的前缀",
                    matches.len()
                ))
            }
        }
    }
    Ok(resolved)
}

fn settings_patch(provider: &str, patch: Value) -> Value {
    let mut object = Map::new();
    object.insert(provider.to_owned(), patch);
    Value::Object(object)
}

fn random_gateway_key() -> String {
    let mut random = [0u8; 32];
    getrandom::getrandom(&mut random).expect("Unable to generate API key");
    format!(
        "fr_{}",
        random
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    )
}

async fn status(json_output: bool) -> Result<(), String> {
    let client = http();
    let paths = config::resolve();
    if let Some(status) = gateway_status(&client).await? {
        if json_output {
            println!(
                "{}",
                serde_json::to_string_pretty(&status).unwrap_or_default()
            );
        } else {
            println!("Free Router 运行中（{}）", base_url());
            println!(
                "默认上游：{}",
                status
                    .get("default_provider")
                    .and_then(Value::as_str)
                    .unwrap_or("未知")
            );
            println!(
                "请求 {} · 故障切换 {} · Key 重试 {}",
                status.get("requests").and_then(Value::as_u64).unwrap_or(0),
                status.get("fallbacks").and_then(Value::as_u64).unwrap_or(0),
                status
                    .get("key_retries")
                    .and_then(Value::as_u64)
                    .unwrap_or(0),
            );
            print_provider_lines(&view_from_status(&status));
        }
        return Ok(());
    }
    let settings = read_settings(&paths)?;
    let views = view_from_settings(&settings)?;
    let default_provider = settings
        .get("default_provider")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .unwrap_or_else(|| env::var("DEFAULT_PROVIDER").unwrap_or_else(|_| "opencode".to_owned()));
    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "service": "Free Router",
                "running": false,
                "base_url": base_url(),
                "settings_file": paths.settings_file.display().to_string(),
                "gateway_key_file": paths.gateway_key_file.display().to_string(),
                "default_provider": default_provider,
                "providers": views_json(&views),
            }))
            .unwrap_or_default()
        );
        return Ok(());
    }
    println!("网关未运行（{} 无法连接）", base_url());
    println!("默认上游：{default_provider}");
    print_provider_lines(&views);
    println!("设置文件：{}", paths.settings_file.display());
    Ok(())
}

async fn keys(command: KeysCommand) -> Result<(), String> {
    let client = http();
    let paths = config::resolve();
    match command {
        KeysCommand::List { provider, json } => {
            let filter = provider.as_deref().map(valid_provider).transpose()?;
            let views = match gateway_status(&client).await? {
                Some(status) => view_from_status(&status),
                None => view_from_settings(&read_settings(&paths)?)?,
            };
            let views: Vec<ProviderView> = views
                .into_iter()
                .filter(|view| filter.is_none_or(|id| id == view.id))
                .collect();
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&views_json(&views)).unwrap_or_default()
                );
                return Ok(());
            }
            if views.is_empty() {
                println!("没有匹配的上游");
                return Ok(());
            }
            for view in &views {
                println!("{}（{} 个 Key）", view.id, view.key_count);
                if view.keys.is_empty() {
                    println!("  （未配置）");
                }
                for key in &view.keys {
                    println!("  {}  {}", key.id, key.label);
                }
            }
            Ok(())
        }
        KeysCommand::Add { provider, keys } => {
            let provider = valid_provider(&provider)?;
            let incoming =
                crate::parse_keys(&Value::Array(keys.iter().map(|key| json!(key)).collect()))
                    .map_err(|message| format!("{provider}：{message}"))?;
            if incoming.is_empty() {
                return Err("没有提供有效的 Key".into());
            }
            let secrets: Vec<String> = incoming.into_iter().map(|key| key.secret).collect();
            let added = secrets.len();
            let body = settings_patch(provider, json!({"add": secrets.clone()}));
            mutate(&client, &paths, body, move |settings| {
                let mut current: Vec<String> =
                    crate::parse_keys(settings.get(provider).unwrap_or(&Value::Null))
                        .map_err(|message| format!("{provider}：{message}"))?
                        .into_iter()
                        .map(|key| key.secret)
                        .collect();
                for secret in secrets {
                    if !current.contains(&secret) {
                        current.push(secret);
                    }
                }
                if current.len() > 16 {
                    return Err(format!("{provider}：最多支持 16 个 Key"));
                }
                settings
                    .as_object_mut()
                    .ok_or("设置文件不是 JSON 对象")?
                    .insert(provider.to_owned(), json!(current));
                Ok(())
            })
            .await?;
            println!("已向 {provider} 添加 {added} 个 Key（重复项自动忽略）");
            Ok(())
        }
        KeysCommand::Remove { provider, ids } => {
            let provider = valid_provider(&provider)?;
            let views = match gateway_status(&client).await? {
                Some(status) => view_from_status(&status),
                None => view_from_settings(&read_settings(&paths)?)?,
            };
            let view = views
                .iter()
                .find(|view| view.id == provider)
                .ok_or_else(|| format!("未知上游 “{provider}”"))?;
            let resolved = resolve_ids(&view.keys, &ids)?;
            let body = settings_patch(provider, json!({"remove": resolved.clone()}));
            mutate(&client, &paths, body, move |settings| {
                let current = crate::parse_keys(settings.get(provider).unwrap_or(&Value::Null))
                    .map_err(|message| format!("{provider}：{message}"))?;
                if resolved
                    .iter()
                    .any(|id| !current.iter().any(|key| &key.id == id))
                {
                    return Err("Key 池已发生变化，请重新列出 Key 后重试".into());
                }
                let remaining: Vec<String> = current
                    .into_iter()
                    .filter(|key| !resolved.contains(&key.id))
                    .map(|key| key.secret)
                    .collect();
                settings
                    .as_object_mut()
                    .ok_or("设置文件不是 JSON 对象")?
                    .insert(provider.to_owned(), json!(remaining));
                Ok(())
            })
            .await?;
            println!("已从 {provider} 移除 {} 个 Key", ids.len());
            Ok(())
        }
        KeysCommand::Clear { provider } => {
            let provider = valid_provider(&provider)?;
            let body = settings_patch(provider, Value::Null);
            mutate(&client, &paths, body, move |settings| {
                settings
                    .as_object_mut()
                    .ok_or("设置文件不是 JSON 对象")?
                    .insert(provider.to_owned(), json!([]));
                Ok(())
            })
            .await?;
            println!("已清空 {provider} 的 Key");
            Ok(())
        }
    }
}

async fn provider(id: Option<String>) -> Result<(), String> {
    let client = http();
    let paths = config::resolve();
    let Some(id) = id else {
        let current = match gateway_status(&client).await? {
            Some(status) => status
                .get("default_provider")
                .and_then(Value::as_str)
                .map(str::to_owned),
            None => read_settings(&paths)?
                .get("default_provider")
                .and_then(Value::as_str)
                .map(str::to_owned),
        };
        println!(
            "{}",
            current.unwrap_or_else(
                || env::var("DEFAULT_PROVIDER").unwrap_or_else(|_| "opencode".to_owned())
            )
        );
        return Ok(());
    };
    let id = valid_provider(&id)?;
    mutate(
        &client,
        &paths,
        json!({"default_provider": id}),
        move |settings| {
            settings
                .as_object_mut()
                .ok_or("设置文件不是 JSON 对象")?
                .insert("default_provider".to_owned(), json!(id));
            Ok(())
        },
    )
    .await?;
    println!("默认上游已设为 {id}");
    Ok(())
}

async fn gateway_key(command: GatewayKeyCommand) -> Result<(), String> {
    let client = http();
    let paths = config::resolve();
    match command {
        GatewayKeyCommand::Show => {
            if gateway_status(&client).await?.is_some() {
                let mut request = client
                    .get(format!("{}/api/gateway-key", base_url()))
                    .header("X-Gateway-Settings", "1");
                let token = management_token();
                if !token.is_empty() {
                    request = request.bearer_auth(token);
                }
                let response = request
                    .send()
                    .await
                    .map_err(|_| "无法读取运行中的网关密钥")?;
                if !response.status().is_success() {
                    return Err(format!(
                        "读取网关密钥失败（{}）；请提供正确的 GATEWAY_API_KEY",
                        response.status()
                    ));
                }
                let value: Value = response
                    .json()
                    .await
                    .map_err(|_| "网关密钥接口返回无效 JSON")?;
                let key = value
                    .get("key")
                    .and_then(Value::as_str)
                    .ok_or("网关密钥接口返回无效响应")?;
                if !key.is_empty() {
                    println!("{key}");
                    return Ok(());
                }
            } else {
                match std::fs::read_to_string(&paths.gateway_key_file) {
                    Ok(key) if !key.trim().is_empty() => {
                        println!("{}", key.trim());
                        return Ok(());
                    }
                    Ok(_) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(_) => return Err("无法读取本地网关密钥文件".into()),
                }
            }
            let token = management_token();
            if token.is_empty() {
                println!("（未设置网关密钥，/v1 接口当前免认证）");
            } else {
                println!("{token}");
            }
            Ok(())
        }
        GatewayKeyCommand::Generate => {
            let live = gateway_status(&client).await?.is_some();
            if live {
                match api_post(&client, "/api/gateway-key", Value::Null).await {
                    PostOutcome::Ok(value) => {
                        println!("{}", value.get("key").and_then(Value::as_str).unwrap_or(""));
                        return Ok(());
                    }
                    PostOutcome::Failed(message) => return Err(message),
                }
            }
            let _guard = paths.lock_for_cli().await?;
            let key = random_gateway_key();
            write_private(&paths.gateway_key_file, key.as_bytes()).map_err(|error| {
                format!("无法保存 {}：{error}", paths.gateway_key_file.display())
            })?;
            println!("{key}");
            eprintln!("提示：网关未运行，已写入配置文件，启动网关后生效。");
            Ok(())
        }
    }
}

fn config_info() -> Result<(), String> {
    let paths = config::resolve();
    println!("资源根目录：{}", paths.root.display());
    println!("设置文件：{}", paths.settings_file.display());
    println!("网关密钥文件：{}", paths.gateway_key_file.display());
    println!("网关地址：{}", base_url());
    println!(
        "管理密钥：{}",
        if management_token().is_empty() {
            "未设置（管理接口免鉴权）"
        } else {
            "已通过 GATEWAY_API_KEY 设置"
        }
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(id: &str) -> KeyView {
        KeyView {
            id: id.to_owned(),
            label: "Key".to_owned(),
        }
    }

    #[test]
    fn provider_names_are_validated() {
        assert_eq!(valid_provider("opencode").unwrap(), "opencode");
        assert_eq!(valid_provider("commandcode").unwrap(), "commandcode");
        assert!(valid_provider("unknown").unwrap_err().contains("未知上游"));
    }

    #[test]
    fn key_ids_resolve_by_unique_prefix() {
        let keys = [key("key_ab12cd34ef56"), key("key_ff99aa88bb77")];
        assert_eq!(
            resolve_ids(&keys, &["key_ab12".to_owned()]).unwrap(),
            vec!["key_ab12cd34ef56".to_owned()]
        );
        assert_eq!(
            resolve_ids(&keys, &["AB12".to_owned()]).unwrap(),
            vec!["key_ab12cd34ef56".to_owned()]
        );
        assert_eq!(
            resolve_ids(&keys, &["key_ab12".to_owned(), "key_ff99".to_owned()]).unwrap(),
            vec!["key_ab12cd34ef56".to_owned(), "key_ff99aa88bb77".to_owned()]
        );
        assert!(resolve_ids(&keys, &["key_".to_owned()])
            .unwrap_err()
            .contains("不能为空"));
        assert!(resolve_ids(&keys, &["key_zz".to_owned()])
            .unwrap_err()
            .contains("找不到"));
    }

    #[test]
    fn gateway_key_has_expected_shape() {
        let key = random_gateway_key();
        assert!(key.starts_with("fr_") && key.len() == 67);
        assert!(key[3..].bytes().all(|byte| byte.is_ascii_hexdigit()));
    }

    #[cfg(unix)]
    #[test]
    fn private_writes_use_0600() {
        use std::os::unix::fs::PermissionsExt;
        let directory =
            std::env::temp_dir().join(format!("free-router-cli-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let file = directory.join("settings.local.json");
        write_private(&file, b"{}").unwrap();
        assert_eq!(std::fs::read(&file).unwrap(), b"{}");
        assert_eq!(file.metadata().unwrap().permissions().mode() & 0o777, 0o600);
        assert!(!file.with_extension("tmp").exists());
        std::fs::remove_dir_all(&directory).unwrap();
    }
}
