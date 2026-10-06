//! 第 6 课：struct 与 impl
//!
//! 运行：cargo run -- 6

use std::fmt;

/// 三种 struct 写法
/// 1. 具名字段（最常用）
#[derive(Debug, Clone, PartialEq, Default)]
pub struct KeyStat {
    pub id: usize,
    pub label: String,
    pub fail_count: u32,
    pub cooldown_ms: u64, // 没有 `pub`：模块外不可访问（私有性默认）
}

/// 2. 元组结构体：适合「新类型」包装
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KeyId(pub usize);

impl KeyId {
    /// 关联函数（没有 self），通常用来做构造器
    pub fn new(id: usize) -> Self {
        Self(id)
    }
}

/// 3. 单元结构体：没有数据的标记类型
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Healthy;

impl KeyStat {
    // ---- 关联函数：构造函数 ------------------------------------------------
    pub fn new(id: usize, label: &str) -> Self {
        Self {
            id,
            label: label.to_string(),
            fail_count: 0,
            cooldown_ms: 0,
        }
    }

    // ---- 方法：第一个参数是 &self / &mut self / self --------------------
    /// 不可变借用：只读
    pub fn is_cooled_down(&self, now_ms: u64) -> bool {
        now_ms < self.cooldown_ms
    }

    /// 可变借用：改自己（调用方必须先声明 mut）
    pub fn record_failure(&mut self, now_ms: u64, backoff_ms: u64) {
        self.fail_count += 1;
        self.cooldown_ms = now_ms + backoff_ms * u64::from(self.fail_count);
    }

    /// 消耗 self：调用后原对象就没了，适合「收尾后返回新对象」
    pub fn into_label(self) -> String {
        self.label
    }

    /// 关联函数 + 无 self：工具函数
    pub fn recommended_backoff(fails: u32) -> u64 {
        100 * 2u64.pow(fails.saturating_sub(1))
    }
}

/// 为自己的类型实现标准库 trait，这样它就能用 `{}` 打印。
impl fmt::Display for KeyStat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{} {} (失败 {} 次)", self.id, self.label, self.fail_count)
    }
}

pub fn run() {
    println!("== 第 6 课：struct / impl ==\n");

    let mut k = KeyStat::new(1, "sk-aaa");
    println!("{k}"); // Display
    println!("结构体全貌 {k:?}"); // Debug
    println!("字段访问：{} {}", k.id, k.label);
    println!();

    println!("冷却中？{}", k.is_cooled_down(0));
    k.record_failure(0, 100);
    println!("失败一次后：{k}，冷却到 {}ms", k.cooldown_ms);
    println!("现在还在冷却？{}", k.is_cooled_down(50));
    println!("500ms 时还冷却？{}", k.is_cooled_down(500));
    println!();

    println!("退避建议(1..=3)：{:?}",
        (1..=3).map(KeyStat::recommended_backoff).collect::<Vec<_>>());
    println!();

    // Default：derive 出来的
    let d = KeyStat::default();
    println!("default = {d:?}");
    println!();

    // Clone / Copy / PartialEq
    let k2 = k.clone();
    println!("clone 后相等？{}", k == k2);
    let id = KeyId::new(7);
    let id2 = id; // Copy：id 依然可用
    println!("Copy 类型：id={:?} id2={:?} 相等={}", id, id2, id == id2);
    let _marker = Healthy;
    println!();

    // 消耗 self 的方法
    println!("into_label 消耗了对象，返回：{}", k2.into_label());

    println!("\n动手练习：");
    println!("  1. 给 KeyStat 加 `fn is_usable(&self, now_ms: u64, max_fail: u32) -> bool`。");
    println!("  2. 实现 `Display` 时把冷却时间也打印出来。");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 记录失败会变长退避() {
        let mut k = KeyStat::new(1, "sk-1");
        assert_eq!(k.fail_count, 0);
        k.record_failure(0, 100);
        assert_eq!(k.fail_count, 1);
        assert_eq!(k.cooldown_ms, 100);
        k.record_failure(0, 100);
        assert_eq!(k.fail_count, 2);
        assert_eq!(k.cooldown_ms, 200);
    }

    #[test]
    fn 冷却判断() {
        let mut k = KeyStat::new(1, "sk-1");
        k.record_failure(0, 100);
        assert!(k.is_cooled_down(99));
        assert!(!k.is_cooled_down(100));
    }

    #[test]
    fn 退避上限不会溢出() {
        assert_eq!(KeyStat::recommended_backoff(1), 100);
        assert_eq!(KeyStat::recommended_backoff(3), 400);
        assert_eq!(KeyStat::recommended_backoff(0), 100);
    }

    #[test]
    fn default_值全零() {
        let d = KeyStat::default();
        assert_eq!(d.fail_count, 0);
        assert!(d.label.is_empty());
    }
}
