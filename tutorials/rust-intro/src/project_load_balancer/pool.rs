//! Key 轮询池：每个上游有若干 key，轮流使用；失败的 key 进入冷却。
//!
//! 这一层演示三件事：
//! - `HashMap` 存「每个上游各自的状态」（第 12 课）
//! - `Option` 表达「没有可用 key」而不是返回一个假 key（第 7 课）
//! - 用「逻辑时刻」而不是真实时间来算冷却，测试才能稳定复现（第 16 课）

use std::collections::HashMap;

/// 一个 key 的状态。枚举比 `bool + 额外字段` 更能表达「有几种情况」。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyState {
    /// 可以立刻使用
    Ready,
    /// 冷却中，到这个逻辑时刻之后才能再用
    Cooling { until: u64 },
}

/// 轮询池。key 本身用编号表示 —— 真实项目里也绝不该把 key 明文写进日志。
#[derive(Debug, Default)]
pub struct KeyPool {
    /// 上游名 -> 它的 key 编号列表
    keys: HashMap<&'static str, Vec<u32>>,
    /// 上游名 -> 下一次从哪个下标开始找（实现 round robin）
    cursor: HashMap<&'static str, usize>,
    /// (上游名, key 编号) -> 状态
    state: HashMap<(&'static str, u32), KeyState>,
}

impl KeyPool {
    pub fn new() -> Self {
        Self::default()
    }

    /// 给某个上游登记 count 个 key，编号从 0 开始。
    pub fn add_keys(&mut self, provider: &'static str, count: u32) {
        let entry = self.keys.entry(provider).or_default();
        entry.extend(0..count);
    }

    /// 取出下一个可用的 key：从上次的位置继续往后找，跳过冷却中的。
    ///
    /// 全部冷却时返回 `None` —— 调用方据此返回 503，而不是随便挑一个必然失败的 key。
    pub fn next_key(&mut self, provider: &'static str, now: u64) -> Option<u32> {
        let ids = self.keys.get(provider)?;
        if ids.is_empty() {
            return None;
        }

        let start = self.cursor.get(provider).copied().unwrap_or(0);
        for offset in 0..ids.len() {
            let index = (start + offset) % ids.len();
            let id = ids[index];
            if self.is_ready(provider, id, now) {
                // 下次从这一个的后面接着找
                self.cursor.insert(provider, (index + 1) % ids.len());
                return Some(id);
            }
        }
        None
    }

    /// 这个 key 现在能不能用
    pub fn is_ready(&self, provider: &'static str, id: u32, now: u64) -> bool {
        match self.state.get(&(provider, id)) {
            Some(KeyState::Cooling { until }) => *until <= now,
            _ => true,
        }
    }

    /// 记录一次失败，按状态码决定冷却多久。
    /// 401/403 是密钥问题，冷却久一点（300）；429/5xx 是临时问题（30）。
    pub fn mark_failed(&mut self, provider: &'static str, id: u32, status: u16, now: u64) {
        let seconds = match status {
            401 | 403 => 300,
            429 | 500..=599 => 30,
            _ => return, // 其他状态不冷却
        };
        self.state
            .insert((provider, id), KeyState::Cooling { until: now + seconds });
    }

    /// 全部冷却时，最早能重试的时刻（用来算 `Retry-After`）
    pub fn earliest_retry(&self, provider: &'static str) -> Option<u64> {
        self.keys
            .get(provider)?
            .iter()
            .filter_map(|id| match self.state.get(&(provider, *id)) {
                Some(KeyState::Cooling { until }) => Some(*until),
                _ => None,
            })
            .min()
    }

    /// 现在有几个 key 可用（只用来打印，不参与挑选逻辑）
    pub fn ready_count(&self, provider: &'static str, now: u64) -> usize {
        self.keys
            .get(provider)
            .map(|ids| {
                ids.iter()
                    .filter(|id| self.is_ready(provider, **id, now))
                    .count()
            })
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 轮询按顺序循环() {
        let mut pool = KeyPool::new();
        pool.add_keys("openrouter", 3);
        assert_eq!(pool.next_key("openrouter", 0), Some(0));
        assert_eq!(pool.next_key("openrouter", 0), Some(1));
        assert_eq!(pool.next_key("openrouter", 0), Some(2));
        assert_eq!(pool.next_key("openrouter", 0), Some(0)); // 回到开头
    }

    #[test]
    fn 没有_key_的上游返回_none() {
        let mut pool = KeyPool::new();
        assert_eq!(pool.next_key("openrouter", 0), None);
        pool.add_keys("commandcode", 0);
        assert_eq!(pool.next_key("commandcode", 0), None);
    }

    #[test]
    fn 冷却中的_key_会被跳过() {
        let mut pool = KeyPool::new();
        pool.add_keys("openrouter", 2);
        assert_eq!(pool.next_key("openrouter", 0), Some(0));

        pool.mark_failed("openrouter", 0, 429, 0); // 冷却到 30
        assert_eq!(pool.next_key("openrouter", 1), Some(1), "key0 冷却中，应该换 key1");
        assert_eq!(pool.ready_count("openrouter", 1), 1);

        // 时刻推进到 30 之后，key0 恢复
        assert_eq!(pool.next_key("openrouter", 30), Some(0));
        assert_eq!(pool.ready_count("openrouter", 30), 2);
    }

    #[test]
    fn 全部冷却时给出最早可重试时刻() {
        let mut pool = KeyPool::new();
        pool.add_keys("openrouter", 2);
        pool.mark_failed("openrouter", 0, 429, 0); // until 30
        pool.mark_failed("openrouter", 1, 403, 0); // until 300

        assert_eq!(pool.next_key("openrouter", 0), None);
        assert_eq!(pool.earliest_retry("openrouter"), Some(30));
        assert_eq!(pool.next_key("openrouter", 30), Some(0));
    }

    #[test]
    fn 不该重试的状态不产生冷却() {
        let mut pool = KeyPool::new();
        pool.add_keys("openrouter", 1);
        pool.mark_failed("openrouter", 0, 400, 0);
        assert!(pool.is_ready("openrouter", 0, 0));
    }
}
