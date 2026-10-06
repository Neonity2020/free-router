use crate::*;
use axum::body::Bytes;
use axum::http::HeaderValue;
use futures_util::StreamExt;

pub(crate) async fn models(State(app): State<Shared>, headers: HeaderMap) -> Response {
    if !api_authorized(&app, &headers).await {
        return error(StatusCode::UNAUTHORIZED, "Invalid gateway API key");
    }
    let data: Vec<_> = [
        "space-bunny",
        "openrouter/nvidia/nemotron-3-ultra-550b-a55b:free",
        "openrouter/apodex/apodex-1.1-mini:free",
        "opencode/space-bunny",
        "commandcode/space-bunny",
    ]
    .into_iter()
    .map(|id| {
        let mut model = json!({"id":id,"object":"model","created":0,"owned_by":"free-router"});
        if id == "openrouter/nvidia/nemotron-3-ultra-550b-a55b:free" {
            // Mirrors the upstream declaration instead of Space Bunny's five
            // mandatory levels: reasoning is optional, high and medium only.
            model["reasoning"] = json!({
                "mandatory":false,
                "supported_efforts":["high","medium"],
                "default_effort":"high"
            });
        } else if id == "opencode/space-bunny" {
            model["reasoning"] =
                json!({"mandatory":true,"supported_efforts":["low","medium","high","xhigh","max"]});
        }
        model
    })
    .collect();
    Json(json!({"object":"list","data":data})).into_response()
}
pub(crate) fn route(model: &str) -> Option<Option<&'static str>> {
    match model {
        "space-bunny" => Some(None),
        "openrouter/nvidia/nemotron-3-ultra-550b-a55b:free"
        | "openrouter/apodex/apodex-1.1-mini:free" => Some(Some("openrouter")),
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
            "Unknown model. Use space-bunny, opencode/space-bunny, openrouter/<model ID> or commandcode/<model ID>",
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
            } else if p.id == "openrouter" {
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
                    let more = offset + 1 < p.keys.len() || i + 1 < candidates.len();
                    if retry && more {
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
                    // A success status can still carry a provider failure, so
                    // inspect such payloads before any byte reaches the client.
                    if code.is_success()
                        && content_type_is(content_type.as_ref(), "application/json")
                    {
                        match read_capped(Box::pin(upstream.bytes_stream())).await {
                            Some(bytes) => {
                                if embedded_failure_of(&bytes) && more {
                                    key.cool_down(5);
                                    last = error(
                                        StatusCode::BAD_GATEWAY,
                                        "Upstream reported a failure inside a successful response.",
                                    );
                                    continue;
                                }
                                return relay(
                                    code,
                                    Body::from(bytes),
                                    retry_after,
                                    content_type,
                                    p.id,
                                );
                            }
                            None => {
                                key.cool_down(5);
                                last = error(
                                    StatusCode::BAD_GATEWAY,
                                    "Unable to read the upstream response",
                                );
                                if more {
                                    continue;
                                }
                                return last;
                            }
                        }
                    }
                    if code.is_success()
                        && content_type_is(content_type.as_ref(), "text/event-stream")
                    {
                        let mut stream = Box::pin(upstream.bytes_stream());
                        let mut buffered: Vec<u8> = Vec::new();
                        let peek = async {
                            while buffered.len() <= INSPECT_LIMIT {
                                let Some(chunk) = stream.next().await else {
                                    break;
                                };
                                let Ok(bytes) = chunk else { break };
                                buffered.extend_from_slice(&bytes);
                                if let Some(data) = first_sse_data(&buffered) {
                                    return serde_json::from_str::<Value>(data)
                                        .is_ok_and(|value| embedded_failure(&value));
                                }
                            }
                            false
                        };
                        // The peek shares the request budget, so an upstream
                        // that never delivers an event still gets relayed.
                        let failed = tokio::time::timeout_at(deadline, peek)
                            .await
                            .unwrap_or(false);
                        if failed && more {
                            key.cool_down(5);
                            last = error(
                                StatusCode::BAD_GATEWAY,
                                "Upstream reported a failure inside a successful response.",
                            );
                            continue;
                        }
                        // The peeked prefix is replayed ahead of the live stream.
                        let replay = futures_util::stream::iter([Ok::<Bytes, reqwest::Error>(
                            Bytes::from(buffered),
                        )]);
                        return relay(
                            code,
                            Body::from_stream(replay.chain(stream)),
                            retry_after,
                            content_type,
                            p.id,
                        );
                    }
                    return relay(
                        code,
                        Body::from_stream(upstream.bytes_stream()),
                        retry_after,
                        content_type,
                        p.id,
                    );
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

/// Largest successful payload inspected for an embedded provider failure.
const INSPECT_LIMIT: usize = 8 * 1024 * 1024;

/// Read a whole body, refusing to buffer more than the inspection limit.
/// `None` means the body is too large to inspect or was interrupted.
async fn read_capped<S>(mut stream: S) -> Option<Bytes>
where
    S: futures_util::Stream<Item = Result<Bytes, reqwest::Error>> + Unpin,
{
    let mut buffer: Vec<u8> = Vec::new();
    while let Some(chunk) = stream.next().await {
        let bytes = chunk.ok()?;
        if buffer.len() + bytes.len() > INSPECT_LIMIT {
            return None;
        }
        buffer.extend_from_slice(&bytes);
    }
    Some(Bytes::from(buffer))
}

fn content_type_is(content_type: Option<&HeaderValue>, prefix: &str) -> bool {
    content_type
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.trim_start().to_ascii_lowercase().starts_with(prefix))
}

/// OpenRouter reports some provider failures as a successful response whose
/// payload only carries `error`. Such a body hides a retryable failure and must
/// not be relayed while another key or provider is still available.
fn embedded_failure(value: &Value) -> bool {
    value.get("error").is_some_and(|error| !error.is_null())
        && value
            .get("choices")
            .and_then(Value::as_array)
            .is_none_or(|choices| choices.is_empty())
}

fn embedded_failure_of(bytes: &[u8]) -> bool {
    serde_json::from_slice::<Value>(bytes).is_ok_and(|value| embedded_failure(&value))
}

/// Payload of the first complete SSE event that carries data, skipping
/// keep-alive comments and an event that is still incomplete.
fn first_sse_data(buffered: &[u8]) -> Option<&str> {
    let mut rest = std::str::from_utf8(buffered).ok()?;
    while let Some((event, tail)) = rest.split_once("\n\n") {
        if let Some(data) = event.split('\n').find_map(|line| {
            let data = line.strip_prefix("data:")?.trim();
            (!data.is_empty() && data != "[DONE]").then_some(data)
        }) {
            return Some(data);
        }
        rest = tail;
    }
    None
}

fn relay(
    code: StatusCode,
    body: Body,
    retry_after: Option<HeaderValue>,
    content_type: Option<HeaderValue>,
    provider: &str,
) -> Response {
    let mut response = Response::new(body);
    *response.status_mut() = code;
    if let Some(value) = retry_after {
        response.headers_mut().insert("retry-after", value);
    }
    if let Some(value) = content_type {
        response.headers_mut().insert("content-type", value);
    }
    response
        .headers_mut()
        .insert("x-gateway-provider", provider.parse().unwrap());
    response
        .headers_mut()
        .insert("cache-control", "no-cache".parse().unwrap());
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn embedded_provider_failures() {
        assert!(embedded_failure_of(
            br#"{"error":{"code":503,"message":"overloaded"}}"#
        ));
        assert!(embedded_failure_of(
            br#"{"choices":[],"error":{"code":503}}"#
        ));
        assert!(!embedded_failure_of(
            br#"{"choices":[{"message":{"content":"hello"}}]}"#
        ));
        assert!(!embedded_failure_of(br#"{"choices":[]}"#));
        assert!(!embedded_failure_of(br#"{"error":null}"#));
        assert!(!embedded_failure_of(b"not json"));
    }
    #[test]
    fn first_event_skips_comments_and_partial_frames() {
        assert_eq!(first_sse_data(b": keep-alive\n\n"), None);
        assert_eq!(first_sse_data(b"data: [DONE]\n\n"), None);
        assert_eq!(first_sse_data(b"data: {\"choices\":[\""), None);
        assert_eq!(
            first_sse_data(b": OPENROUTER PROCESSING\n\ndata: hello\n\n"),
            Some("hello")
        );
        assert_eq!(
            first_sse_data(b"data: {\"a\":1}\n\ndata: {\"b"),
            Some("{\"a\":1}")
        );
        let failure = first_sse_data(b"data: {\"choices\":[],\"error\":{\"code\":503}}\n\n")
            .and_then(|data| serde_json::from_str::<Value>(data).ok());
        assert!(failure.is_some_and(|value| embedded_failure(&value)));
        let healthy = first_sse_data(b"data: {\"choices\":[{\"delta\":{\"content\":\"hi\"}}]}\n\n")
            .and_then(|data| serde_json::from_str::<Value>(data).ok());
        assert!(healthy.is_some_and(|value| !embedded_failure(&value)));
    }
}
