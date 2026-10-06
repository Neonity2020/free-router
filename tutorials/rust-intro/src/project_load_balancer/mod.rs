//! 实战项目：迷你负载均衡器
//!
//! 运行：cargo run -- project
//!
//! 把前面 16 课拼在一起：
//! - trait + dyn（第 9 课）：上游的统一接口
//! - Vec / HashMap / 迭代器（第 5、12 课）：key 池与统计
//! - Option / Result（第 7、8 课）：没有可用 key 时返回 None
//! - 模块与可见性（第 11 课）：mod.rs 声明子模块，文件即模块
//!
//! 模块结构（和 free-router 这类真实网关的思路一致）：
//! ```text
//! project_load_balancer/
//!   mod.rs          ← 你现在看的这个文件，负责组装与演示
//!   provider.rs     ← 上游抽象
//!   pool.rs         ← Key 轮询池与冷却
//!   stats.rs        ← 统计
//! ```

pub mod pool;
pub mod provider;
pub mod stats;

// 重导出（第 11 课）：外部只需要 use project_load_balancer::KeyPool，
// 不用关心它到底住在哪个文件里。
pub use pool::KeyPool;
pub use provider::Provider;
pub use stats::Stats;

use provider::demo_providers;

/// 按顺序模拟一串请求，返回 (trace, stats, pool, 结束时刻) 方便测试断言。
///
/// 时间用「逻辑时刻」now 表示：每次请求 +1，需要跳过冷却时再手动往前推。
/// 这样测试不依赖真实时钟，永远不会因为机器慢而偶发失败。
pub fn simulate(script: &[u16]) -> (Vec<String>, Stats, KeyPool, u64) {
    let providers = demo_providers();
    let mut pool = KeyPool::new();
    for p in &providers {
        pool.add_keys(p.name(), p.key_count() as u32);
    }

    let mut stats = Stats::default();
    let mut trace = Vec::new();
    let mut now = 0u64;

    for status in script {
        // 找一个「既可用、又有空闲 key」的上游
        let picked = providers
            .iter()
            .find_map(|p| pool.next_key(p.name(), now).map(|key| (p.name(), key)));

        let Some((name, key)) = picked else {
            let earliest = providers
                .iter()
                .filter_map(|p| pool.earliest_retry(p.name()))
                .min();
            trace.push(format!("#{now} 没有可用 key → 503（最早可重试：{earliest:?}）"));
            break;
        };

        if *status == 200 {
            stats.record_ok(name);
            trace.push(format!("#{now} {name:<12} key#{key} → 200"));
        } else {
            stats.record_failed(name);
            pool.mark_failed(name, key, *status, now);
            trace.push(format!("#{now} {name:<12} key#{key} → {status}（该 key 进入冷却）"));
        }
        now += 1;
    }

    (trace, stats, pool, now)
}

pub fn run() {
    println!("== 实战项目：迷你负载均衡器 ==\n");

    // ---- 1. 上游清单 ------------------------------------------------------
    let providers = demo_providers();
    println!("上游清单：");
    for p in &providers {
        println!(
            "  {:<12} keys={} 可用={}",
            p.name(),
            p.key_count(),
            p.is_healthy()
        );
    }
    println!();

    // ---- 2. 模拟一串请求 --------------------------------------------------
    // 状态码写死，保证每次运行输出一致（真实项目里来自上游响应）。
    let script = [200u16, 429, 500, 200, 403, 200, 200, 200];
    println!("模拟请求（状态码预置）：{script:?}\n");
    let (trace, stats, pool, now) = simulate(&script);
    for line in &trace {
        println!("  {line}");
    }
    println!();

    // ---- 3. 统计 ----------------------------------------------------------
    println!("请求统计：");
    for line in stats.summary() {
        println!("  {line}");
    }
    println!();

    // ---- 4. 冷却结束后自动恢复 --------------------------------------------
    println!("当前时刻 {now}，各上游可用 key：");
    for p in &providers {
        println!("  {:<12} {}/{}", p.name(), pool.ready_count(p.name(), now), p.key_count());
    }

    let later = now + 300; // 403 的冷却最长，推 300 秒后应该全部恢复
    println!("\n时间推进到 {later}：");
    for p in &providers {
        println!("  {:<12} {}/{}", p.name(), pool.ready_count(p.name(), later), p.key_count());
    }
    println!();

    println!("要点：");
    println!("  - 可用性判断和「选哪个 key」分开：provider 管前者，pool 管后者");
    println!("  - 全部 key 冷却时返回 None，调用方据此返回 503 和 Retry-After，而不是硬试");
    println!("  - 冷却用逻辑时刻计算，测试才能稳定复现（不依赖 sleep）");
    println!("  - key 编号进日志，key 明文永远不进日志");

    println!("\n动手练习：");
    println!("  1. 给 KeyPool 加一个 `reset(provider, id)`，模拟管理员手动恢复 key。");
    println!("  2. 把 pool 换成「按成功率加权选择」，改完跑 cargo test 看哪些测试需要更新。");
    println!("  3. 给 Stats 加一个 `fn total(&self) -> u32`，并补上测试。");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 正常请求走轮询() {
        let (trace, stats, _, _) = simulate(&[200, 200, 200]);
        assert_eq!(
            trace,
            vec![
                "#0 openrouter   key#0 → 200".to_string(),
                "#1 openrouter   key#1 → 200".to_string(),
                "#2 openrouter   key#0 → 200".to_string(), // 轮询回到 key0
            ]
        );
        assert_eq!(stats.ok("openrouter"), 3);
        assert_eq!(stats.failed("openrouter"), 0);
    }

    #[test]
    fn 限流后换_key_再换上游() {
        // openrouter 两个 key 都被打冷却，opencode 顶上
        let (trace, stats, _, _) = simulate(&[429, 500, 200]);
        assert_eq!(stats.failed("openrouter"), 2);
        assert_eq!(stats.ok("opencode"), 1);
        assert!(trace[2].contains("opencode"), "第三个请求应该换到 opencode：{}", trace[2]);
    }

    #[test]
    fn 全部冷却时返回_503_并给出最早重试时刻() {
        // openrouter 两个 key + opencode 一个 key 全部冷却 → 后面的请求直接 503
        let (trace, stats, pool, now) = simulate(&[429, 500, 403, 200, 200, 200]);
        let last = trace.last().unwrap();
        assert!(last.contains("503"), "应该以 503 结束：{last}");
        assert!(last.contains("Some(30)"), "最早重试应该是 30 秒后：{last}");
        assert_eq!(pool.earliest_retry("openrouter"), Some(30));
        assert_eq!(stats.ok("commandcode"), 0, "没配 key 的上游不该被用到");
        assert!(now > 0);
    }

    #[test]
    fn 冷却结束后恢复可用() {
        let (_, _, pool, now) = simulate(&[429, 500, 403, 200]);
        assert_eq!(pool.ready_count("openrouter", now), 0);
        assert_eq!(pool.ready_count("openrouter", now + 300), 2);
        assert_eq!(pool.ready_count("opencode", now + 300), 1);
    }

    #[test]
    fn 不可用的上游不参与挑选() {
        let providers = demo_providers();
        let names: Vec<&str> = providers
            .iter()
            .filter(|p| p.is_healthy())
            .map(|p| p.name())
            .collect();
        assert!(!names.contains(&"commandcode"));
    }
}
