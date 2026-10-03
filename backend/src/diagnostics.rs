use axum::{
    body::{Body, BodyDataStream, Bytes},
    response::Response,
};
use futures_util::Stream;
use serde_json::json;
use std::{
    pin::Pin,
    sync::atomic::{AtomicU64, Ordering},
    task::{Context, Poll},
    time::Instant,
};

static NEXT_ID: AtomicU64 = AtomicU64::new(1);
pub(crate) struct RequestLog {
    id: u64,
    started: Instant,
}
impl RequestLog {
    pub(crate) fn new() -> Self {
        Self {
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            started: Instant::now(),
        }
    }
    pub(crate) fn attempt(
        &self,
        provider: &'static str,
        attempt: u64,
        status: u16,
        started: Instant,
    ) {
        if std::env::var("GATEWAY_LOG_REQUESTS").as_deref() == Ok("0") {
            return;
        }
        eprintln!(
            "{}",
            json!({"event":"upstream_attempt", "request_id":self.id, "provider":provider, "attempt":attempt, "status":status, "elapsed_ms":started.elapsed().as_millis()})
        );
    }
    pub(crate) fn observe(self, mut response: Response) -> Response {
        response
            .headers_mut()
            .insert("x-gateway-request-id", self.id.to_string().parse().unwrap());
        let status = response.status().as_u16();
        let provider = response
            .headers()
            .get("x-gateway-provider")
            .and_then(|value| value.to_str().ok())
            .filter(|value| matches!(*value, "openrouter" | "opencode" | "commandcode"))
            .map(str::to_owned);
        let (parts, body) = response.into_parts();
        Response::from_parts(
            parts,
            Body::from_stream(LoggedBody {
                inner: Box::pin(body.into_data_stream()),
                log: self,
                status,
                provider,
                outcome: "cancelled",
            }),
        )
    }
}
struct LoggedBody {
    inner: Pin<Box<BodyDataStream>>,
    log: RequestLog,
    status: u16,
    provider: Option<String>,
    outcome: &'static str,
}
impl Stream for LoggedBody {
    type Item = Result<Bytes, axum::Error>;
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let result = self.inner.as_mut().poll_next(cx);
        match &result {
            Poll::Ready(None) if self.outcome != "stream_error" => self.outcome = "complete",
            Poll::Ready(Some(Err(_))) => self.outcome = "stream_error",
            _ => {}
        }
        result
    }
}
impl Drop for LoggedBody {
    fn drop(&mut self) {
        if std::env::var("GATEWAY_LOG_REQUESTS").as_deref() == Ok("0") {
            return;
        }
        // Fixed metadata only: no URLs, credentials, payloads or upstream error text.
        eprintln!(
            "{}",
            json!({"event":"gateway_request", "request_id":self.log.id, "status":self.status, "provider":self.provider, "elapsed_ms":self.log.started.elapsed().as_millis(), "outcome":self.outcome})
        );
    }
}
