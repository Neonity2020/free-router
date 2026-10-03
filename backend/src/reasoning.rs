use serde_json::{json, Value};

// Only OpenRouter and OpenCode Space Bunny have verified mandatory reasoning.
// Unknown providers retain the caller's parameters unchanged.
pub fn compatible(provider: &str, body: &Value) -> bool {
    if !matches!(provider, "openrouter" | "opencode") {
        return true;
    }
    !(body.get("reasoning_effort") == Some(&json!("none"))
        || body.pointer("/reasoning/effort") == Some(&json!("none"))
        || body.pointer("/reasoning/enabled") == Some(&json!(false))
        || body.pointer("/thinking/type") == Some(&json!("disabled"))
        || body.get("enable_thinking") == Some(&json!(false)))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn provider_specific_policy() {
        for effort in ["low", "medium", "high", "xhigh", "max", "minimal"] {
            let body = json!({"reasoning_effort":effort});
            assert!(compatible("openrouter", &body));
            assert!(compatible("opencode", &body));
        }
        for body in [
            json!({"reasoning_effort":"none"}),
            json!({"reasoning":{"effort":"none","exclude":true}}),
            json!({"thinking":{"type":"disabled"}}),
            json!({"enable_thinking":false}),
            json!({"reasoning":{"enabled":false}}),
        ] {
            assert!(!compatible("openrouter", &body));
            assert!(!compatible("opencode", &body));
            assert!(compatible("commandcode", &body));
        }
    }
}
