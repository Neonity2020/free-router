use crate::*;

// A custom header blocks cross-origin form submissions; gateway auth still applies.
pub(crate) async fn save_settings(
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
    let mut providers = app.providers.write().await;
    let mut next = providers.clone();
    let mut exa_key = app.exa_key.write().await;
    let mut next_exa = exa_key.clone();
    for (name, value) in fields {
        match name.as_str() {
            "exa" => {
                if value.is_null() {
                    next_exa.clear();
                } else {
                    match parse_keys(value) {
                        Ok(keys) if keys.len() == 1 && value.is_string() => {
                            next_exa = keys[0].secret.clone()
                        }
                        _ => {
                            return error(
                                StatusCode::BAD_REQUEST,
                                "Exa API key must be a non-empty ASCII string, or null to clear",
                            )
                        }
                    }
                }
            }
            "openrouter" | "opencode" | "commandcode" => {
                let p = next.iter_mut().find(|p| p.id == name).unwrap();
                let keys = if let Some(patch) = value.as_object() {
                    if patch.keys().any(|k| k != "add" && k != "remove") {
                        return error(StatusCode::BAD_REQUEST, "Unknown key pool field");
                    }
                    let mut keys = p.keys.clone();
                    if let Some(remove) = patch.get("remove") {
                        let Some(ids) = remove.as_array() else {
                            return error(
                                StatusCode::BAD_REQUEST,
                                "remove must be an array of key IDs",
                            );
                        };
                        for id in ids {
                            let Some(id) = id.as_str() else {
                                return error(StatusCode::BAD_REQUEST, "Invalid key ID");
                            };
                            if !p.keys.iter().any(|k| k.id == id) {
                                return error(
                                    StatusCode::CONFLICT,
                                    "Key pool changed. Refresh and retry.",
                                );
                            }
                            keys.retain(|k| k.id != id);
                        }
                    }
                    if let Some(add) = patch.get("add") {
                        if !add.is_array() {
                            return error(
                                StatusCode::BAD_REQUEST,
                                "add must be an array of API keys",
                            );
                        }
                        let add = match parse_keys(add) {
                            Ok(keys) => keys,
                            Err(message) => {
                                return error(
                                    StatusCode::BAD_REQUEST,
                                    &format!("{name}：{message}"),
                                )
                            }
                        };
                        for key in add {
                            if !keys.iter().any(|k| k.id == key.id) {
                                keys.push(key);
                            }
                        }
                    }
                    if keys.len() > 16 {
                        return error(
                            StatusCode::BAD_REQUEST,
                            "Each provider supports at most 16 API keys",
                        );
                    }
                    keys
                } else {
                    if value == "" {
                        return error(StatusCode::BAD_REQUEST, "Use null or [] to clear API keys");
                    }
                    match parse_keys(value) {
                        Ok(keys) => keys,
                        Err(message) => {
                            return error(StatusCode::BAD_REQUEST, &format!("{name}：{message}"))
                        }
                    }
                };
                let mut keys = keys;
                for key in &mut keys {
                    if let Some(previous) = p.keys.iter().find(|old| old.id == key.id) {
                        *key = previous.clone();
                    }
                }
                p.keys = keys;
                p.cursor = Arc::new(AtomicU64::new(0));
            }
            "default_provider"
                if value == "openrouter" || value == "opencode" || value == "commandcode" => {}
            _ => return error(StatusCode::BAD_REQUEST, "Unknown or invalid settings field"),
        }
    }
    if let Some(preferred) = body.get("default_provider").and_then(Value::as_str) {
        prioritize(&mut next, preferred);
    }
    let mut saved = json!({"default_provider":next[0].id});
    saved["exa"] = json!(next_exa);
    for p in &next {
        saved[p.id] = json!(p.keys.iter().map(|k| &k.secret).collect::<Vec<_>>());
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
    *exa_key = next_exa;
    Json(json!({"saved":true})).into_response()
}
