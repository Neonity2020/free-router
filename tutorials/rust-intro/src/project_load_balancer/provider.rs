//! 上游（provider）：能提供模型服务的后端。
//!
//! 这一层只回答三个问题：你叫什么、有几个 key、现在能不能用。
//! 「怎么发请求」是另一层的事，这里不关心 —— 这就是 trait 抽出来的边界。

/// 一个上游。真实项目里这些信息来自配置文件。
pub trait Provider {
    /// 上游名字。用 &'static str 是因为它来自字面量，活到程序结束。
    fn name(&self) -> &'static str;

    /// 配置了几个 key
    fn key_count(&self) -> usize;

    /// 默认实现：一个 key 都没有就不可用。需要别的判断可以覆盖它。
    fn is_healthy(&self) -> bool {
        self.key_count() > 0
    }

    /// 出错时该不该换 key 重试：
    /// - 401/403 是密钥本身的问题，重试同一个上游没意义
    /// - 429 是限流，5xx 是上游抖动，都值得换一个 key 再试
    fn should_retry(&self, status: u16) -> bool {
        matches!(status, 429 | 500..=599)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OpenRouter {
    pub keys: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OpenCode {
    pub keys: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CommandCode {
    pub keys: usize,
}

impl Provider for OpenRouter {
    fn name(&self) -> &'static str {
        "openrouter"
    }
    fn key_count(&self) -> usize {
        self.keys
    }
}

impl Provider for OpenCode {
    fn name(&self) -> &'static str {
        "opencode"
    }
    fn key_count(&self) -> usize {
        self.keys
    }
}

impl Provider for CommandCode {
    fn name(&self) -> &'static str {
        "commandcode"
    }
    fn key_count(&self) -> usize {
        self.keys
    }
}

/// 演示用的三个上游：Command Code 一个 key 都没配，所以它不可用。
pub fn demo_providers() -> Vec<Box<dyn Provider>> {
    vec![
        Box::new(OpenRouter { keys: 2 }),
        Box::new(OpenCode { keys: 1 }),
        Box::new(CommandCode { keys: 0 }),
    ]
}

/// 从一堆上游里挑出可用的，顺便演示 `dyn Trait` 的运行时多态。
pub fn healthy_names(providers: &[Box<dyn Provider>]) -> Vec<&'static str> {
    providers
        .iter()
        .filter(|p| p.is_healthy())
        .map(|p| p.name())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 没有_key_就不可用() {
        assert!(OpenRouter { keys: 1 }.is_healthy());
        assert!(!CommandCode { keys: 0 }.is_healthy());
    }

    #[test]
    fn 只有限流和_5xx_才值得换_key_重试() {
        let p = OpenRouter { keys: 1 };
        assert!(p.should_retry(429));
        assert!(p.should_retry(503));
        assert!(!p.should_retry(401));
        assert!(!p.should_retry(403));
        assert!(!p.should_retry(400));
    }

    #[test]
    fn 过滤出可用的上游() {
        assert_eq!(healthy_names(&demo_providers()), vec!["openrouter", "opencode"]);
    }
}
