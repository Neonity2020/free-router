use crate::*;

pub(crate) async fn models(State(app): State<Shared>, headers: HeaderMap) -> Response {
    if !api_authorized(&app, &headers).await {
        return error(StatusCode::UNAUTHORIZED, "Invalid gateway API key");
    }
    let data: Vec<_> = [
        "space-bunny",
        "openrouter/space-bunny",
        "openrouter/apodex/apodex-1.1-mini:free",
        "opencode/space-bunny",
        "commandcode/space-bunny",
    ]
    .into_iter()
    .map(|id| {
        let mut model = json!({"id":id,"object":"model","created":0,"owned_by":"free-router"});
        if matches!(id, "openrouter/space-bunny" | "opencode/space-bunny") {
            model["reasoning"] =
                json!({"mandatory":true,"supported_efforts":["low","medium","high","xhigh","max"]});
            if id == "openrouter/space-bunny" {
                model["reasoning"]["default_effort"] = json!("max");
            }
        }
        model
    })
    .collect();
    Json(json!({"object":"list","data":data})).into_response()
}
pub(crate) fn route(model: &str) -> Option<Option<&'static str>> {
    match model {
        "space-bunny" => Some(None),
        "openrouter/space-bunny" | "openrouter/apodex/apodex-1.1-mini:free" | "stealth/space-bunny-alpha" => Some(Some("openrouter")),
        "opencode/space-bunny" | "space-bunny-free" => Some(Some("opencode")),
        id if id.strip_prefix("openrouter/").is_some_and(|model| {
            !model.is_empty()
                && model.len() <= 512
                && model
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-._/:".contains(&b))
        }) =>
        {
            Some(Some("openrouter"))
        }
        id if id.strip_prefix("commandcode/").is_some_and(|model| {
            !model.is_empty()
                && model.len() <= 200
                && model
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-._/:".contains(&b))
        }) =>
        {
            Some(Some("commandcode"))
        }
        _ => None,
    }
}

pub(crate) async fn chat(
    State(app): State<Shared>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    let log = diagnostics::RequestLog::new();
    let deadline = tokio::time::Instant::now() + app.request_timeout;
    let response = tokio::time::timeout_at(
        deadline,
        chat_with_deadline(app, headers, body, deadline, &log),
    )
    .await
    .unwrap_or_else(|_| {
        error(
            StatusCode::GATEWAY_TIMEOUT,
            "Gateway request timeout exceeded",
        )
    });
    log.observe(response)
}

async fn chat_with_deadline(
    app: Shared,
    headers: HeaderMap,
    mut body: Value,
    deadline: tokio::time::Instant,
    log: &diagnostics::RequestLog,
) -> Response {
    if !api_authorized(&app, &headers).await {
        return error(StatusCode::UNAUTHORIZED, "Invalid gateway API key");
    }
    let requested_model = body
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();
    let Some(selected) = route(&requested_model) else {
        return error(
            StatusCode::BAD_REQUEST,
            "Unknown model. Use space-bunny, openrouter/space-bunny, opencode/space-bunny or commandcode/<model ID>",
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
        .filter(|p| !p.keys.is_empty() && selected.is_none_or(|id| id == p.id))
        .collect();
    if candidates.is_empty() {
        return error(
            StatusCode::SERVICE_UNAVAILABLE,
            "No API key configured for this route. Save upstream keys in Settings.",
        );
    }
    app.requests.fetch_add(1, Ordering::Relaxed);
    let mut last = error(StatusCode::BAD_GATEWAY, "Upstream connection failed");
    let mut attempts = 0;
    let mut earliest_retry: Option<u64> = None;
    for (i, p) in candidates.iter().enumerate() {
        if !reasoning::compatible(p.id, &body) {
            last = error(StatusCode::BAD_REQUEST, "This upstream requires reasoning. Use low, medium, high, xhigh or max; disabling reasoning is unsupported.");
            continue;
        }
        if i > 0 {
            app.fallbacks.fetch_add(1, Ordering::Relaxed);
        }
        let mut provider_attempts = 0;
        let start = p.cursor.fetch_add(1, Ordering::Relaxed) as usize % p.keys.len();
        for offset in 0..p.keys.len() {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                return error(
                    StatusCode::GATEWAY_TIMEOUT,
                    "Gateway request timeout exceeded",
                );
            }
            let key = &p.keys[(start + offset) % p.keys.len()];
            if let Some(seconds) = key.retry_after() {
                earliest_retry =
                    Some(earliest_retry.map_or(seconds, |previous| previous.min(seconds)));
                if attempts == 0 {
                    last = error(
                        StatusCode::SERVICE_UNAVAILABLE,
                        "Upstream keys are cooling down; retry later.",
                    );
                    last.headers_mut().insert(
                        "retry-after",
                        earliest_retry.unwrap().to_string().parse().unwrap(),
                    );
                }
                continue;
            }
            if provider_attempts > 0 {
                app.key_retries.fetch_add(1, Ordering::Relaxed);
            }
            provider_attempts += 1;
            attempts += 1;
            let started = std::time::Instant::now();
            body["model"] = json!(if p.id == "commandcode"
                && requested_model != "commandcode/space-bunny"
            {
                requested_model
                    .strip_prefix("commandcode/")
                    .unwrap_or(p.model)
            } else if p.id == "openrouter"
                && requested_model != "openrouter/space-bunny"
            {
                requested_model
                    .strip_prefix("openrouter/")
                    .unwrap_or(p.model)
            } else {
                p.model
            });
            let result = app
                .client
                .post(format!("{}/chat/completions", p.base.trim_end_matches('/')))
                .bearer_auth(&key.secret)
                // The same budget covers retries and the eventual response body/SSE.
                .timeout(remaining)
                .json(&body)
                .send()
                .await;
            match result {
                Ok(upstream) => {
                    let code = upstream.status();
                    log.attempt(p.id, attempts, code.as_u16(), started);
                    let retry = matches!(code.as_u16(), 401 | 403 | 429) || code.is_server_error();
                    if retry {
                        let seconds = upstream
                            .headers()
                            .get("retry-after")
                            .and_then(|v| v.to_str().ok())
                            .and_then(|v| v.parse::<u64>().ok())
                            .unwrap_or(if matches!(code.as_u16(), 401 | 403) {
                                300
                            } else {
                                30
                            });
                        key.cool_down(seconds);
                    } else if code.is_success() {
                        key.recover();
                    }
                    if retry && (offset + 1 < p.keys.len() || i + 1 < candidates.len()) {
                        last = error(
                            code,
                            "Upstream rejected the request; eligible keys exhausted.",
                        );
                        if let Some(value) = upstream.headers().get("retry-after") {
                            last.headers_mut().insert("retry-after", value.clone());
                        }
                        continue;
                    }
                    let retry_after = upstream.headers().get("retry-after").cloned();
                    let content_type = upstream.headers().get("content-type").cloned();
                    let mut response = Response::new(Body::from_stream(upstream.bytes_stream()));
                    *response.status_mut() = code;
                    if let Some(value) = retry_after {
                        response.headers_mut().insert("retry-after", value);
                    }
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
                    log.attempt(
                        p.id,
                        attempts,
                        if e.is_timeout() { 504 } else { 502 },
                        started,
                    );
                    key.cool_down(5);
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
    }
    last
}
