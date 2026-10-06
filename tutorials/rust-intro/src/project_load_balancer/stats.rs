//! 统计：每个上游成功/失败了几次。
//!
//! 用 `BTreeMap` 而不是 `HashMap`：打印顺序稳定，测试和日志都好读。
//! （`HashMap` 的迭代顺序是随机的，第 12 课讲过。）

use std::collections::BTreeMap;

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Stats {
    ok: BTreeMap<&'static str, u32>,
    failed: BTreeMap<&'static str, u32>,
}

impl Stats {
    pub fn record_ok(&mut self, provider: &'static str) {
        *self.ok.entry(provider).or_insert(0) += 1;
    }

    pub fn record_failed(&mut self, provider: &'static str) {
        *self.failed.entry(provider).or_insert(0) += 1;
    }

    pub fn ok(&self, provider: &str) -> u32 {
        self.ok.get(provider).copied().unwrap_or(0)
    }

    pub fn failed(&self, provider: &str) -> u32 {
        self.failed.get(provider).copied().unwrap_or(0)
    }

    /// 成功率。一次都没请求过时返回 0.0，避免 0/0。
    pub fn success_rate(&self, provider: &str) -> f64 {
        let total = self.ok(provider) + self.failed(provider);
        if total == 0 {
            return 0.0;
        }
        f64::from(self.ok(provider)) / f64::from(total)
    }

    /// 每个上游一行摘要，按名字排序（用 Vec 排序去重，比 HashMap 更好读）。
    pub fn summary(&self) -> Vec<String> {
        let mut names: Vec<&'static str> = self
            .ok
            .keys()
            .chain(self.failed.keys())
            .copied()
            .collect();
        names.sort_unstable();
        names.dedup();

        names
            .into_iter()
            .map(|name| {
                format!(
                    "{name:<12} 成功 {} 失败 {} 成功率 {:.0}%",
                    self.ok(name),
                    self.failed(name),
                    self.success_rate(name) * 100.0
                )
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 分别计数() {
        let mut s = Stats::default();
        s.record_ok("openrouter");
        s.record_ok("openrouter");
        s.record_failed("openrouter");
        s.record_ok("opencode");

        assert_eq!(s.ok("openrouter"), 2);
        assert_eq!(s.failed("openrouter"), 1);
        assert_eq!(s.ok("opencode"), 1);
        assert_eq!(s.failed("opencode"), 0);
        assert_eq!(s.ok("没见过的上游"), 0);
    }

    #[test]
    fn 成功率和零请求都安全() {
        let mut s = Stats::default();
        assert_eq!(s.success_rate("openrouter"), 0.0); // 不能 0/0 出 NaN

        s.record_ok("openrouter");
        s.record_failed("openrouter");
        assert!((s.success_rate("openrouter") - 0.5).abs() < 1e-9);
    }

    #[test]
    fn 摘要按名字排序且格式稳定() {
        let mut s = Stats::default();
        s.record_ok("openrouter");
        s.record_failed("opencode");

        let lines = s.summary();
        assert_eq!(lines.len(), 2, "只有两个上游出现过：{lines:?}");
        assert!(lines[0].starts_with("opencode"), "opencode 应该排在前面：{}", lines[0]);
        assert!(lines[0].contains("失败 1"));
        assert!(lines[1].starts_with("openrouter"));
        assert!(lines[1].contains("成功率 100%"));
    }
}
