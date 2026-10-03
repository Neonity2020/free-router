use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

#[derive(Clone)]
pub(crate) struct ApiKey {
    pub(crate) id: String,
    pub(crate) secret: String,
    cooldown: Arc<Mutex<Option<Instant>>>,
}
pub(crate) fn parse_keys(value: &Value) -> Result<Vec<ApiKey>, String> {
    let values: Vec<&Value> = match value {
        Value::Null => vec![],
        Value::String(s) if s.is_empty() => vec![],
        Value::String(_) => vec![value],
        Value::Array(values) if values.len() <= 16 => values.iter().collect(),
        _ => return Err("每家上游最多支持 16 个 API Key".into()),
    };
    let mut keys = Vec::<ApiKey>::new();
    for (index, value) in values.into_iter().enumerate() {
        let fail = |reason: &str| format!("第 {} 个 API Key：{reason}", index + 1);
        let raw = value.as_str().ok_or_else(|| fail("必须是字符串"))?;
        // Copying an Authorization header should store only its credential.
        // Strip surrounding copy artifacts, never whitespace inside a credential.
        let trim = |s: &str| {
            s.trim_matches(|c: char| c.is_whitespace() || matches!(c, '\u{feff}' | '\u{200b}'))
                .to_owned()
        };
        let mut secret = trim(raw);
        if secret
            .get(..7)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("bearer "))
        {
            secret = trim(&secret[7..]);
        }
        if secret.is_empty() {
            return Err(fail("不能为空"));
        }
        if secret.len() > 4096 {
            return Err(fail("长度超过 4096 字节，请只粘贴密钥本身"));
        }
        if !secret.bytes().all(|b| (33..=126).contains(&b)) {
            return Err(fail("含有空格、换行或非 ASCII 字符，请只粘贴密钥本身"));
        }
        if !keys.iter().any(|k| k.secret == secret) {
            keys.push(ApiKey {
                id: format!("key_{:x}", Sha256::digest(secret.as_bytes())),
                secret: secret.to_owned(),
                cooldown: Arc::new(Mutex::new(None)),
            });
        }
    }
    Ok(keys)
}

impl ApiKey {
    pub(crate) fn retry_after(&self) -> Option<u64> {
        self.cooldown
            .lock()
            .unwrap()
            .and_then(|until| until.checked_duration_since(Instant::now()))
            .map(|remaining| remaining.as_secs().saturating_add(1))
    }
    pub(crate) fn cool_down(&self, seconds: u64) {
        let seconds = std::env::var("GATEWAY_KEY_COOLDOWN_SECS")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(seconds)
            .min(86400);
        if seconds == 0 {
            return;
        }
        let until = Instant::now() + Duration::from_secs(seconds);
        let mut state = self.cooldown.lock().unwrap();
        *state = Some(state.map_or(until, |previous| previous.max(until)));
    }
    pub(crate) fn recover(&self) {
        *self.cooldown.lock().unwrap() = None;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn health_shared_across_request_snapshots() {
        let key = parse_keys(&serde_json::json!("test")).unwrap().remove(0);
        let snapshot = key.clone();
        *key.cooldown.lock().unwrap() = Some(Instant::now() + Duration::from_secs(30));
        assert!(snapshot.retry_after().is_some());
        snapshot.recover();
        assert!(key.retry_after().is_none());
        *key.cooldown.lock().unwrap() = Some(Instant::now() - Duration::from_secs(1));
        assert!(snapshot.retry_after().is_none());
    }
}
