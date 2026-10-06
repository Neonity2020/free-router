//! 第 11 课：模块系统与可见性（pub / pub(crate) / use）
//!
//! 运行：cargo run -- 11
//!
//! 模块树：
//! ```text
//! project_load_balancer/
//!   mod.rs          ← 声明子模块，文件即模块
//!     provider.rs   ← 上游抽象
//!     pool.rs       ← Key 轮询池
//!     stats.rs      ← 统计
//! ```
//! （`cargo run -- project` 的实战项目就是按这个结构组织的，可以对照看）

/// 子模块：公开一个「重试策略」
pub mod strategy {
    /// 只有本模块可见，外面用不了
    fn secret() -> u8 {
        42
    }

    /// 公开给父模块及其下属
    pub(crate) fn clamp(value: u32, max: u32) -> u32 {
        if value > max {
            secret(); // 同模块内可以直接调用私有函数
            max
        } else {
            value
        }
    }

    /// 完全公开
    pub fn describe(max: u32) -> String {
        format!("最多重试 {} 次", clamp(3, max))
    }
}

/// 子模块嵌套：router::provider
pub mod router {
    /// 私有项：不写 pub，父模块也不能直接访问
    struct Secret;

    pub mod provider {
        pub fn ping() -> &'static str {
            "pong from provider"
        }
    }

    pub fn call() -> &'static str {
        let _ = Secret; // 同模块内部可见
        provider::ping()
    }
}

/// use 的作用：把长路径变短，或做重命名
use crate::lesson11_modules::router::provider as upstream_provider;

pub fn run() {
    println!("== 第 11 课：模块 ==\n");

    println!("{}", strategy::describe(5));
    println!("{}", strategy::describe(2));
    // strategy::secret(); // error[E0603]: function `secret` is private
    println!();

    println!("{}", router::call());
    println!("{}", upstream_provider::ping()); // use ... as ...
    println!("{}", router::provider::ping()); // 直接全路径
    println!();

    // 重导出：让外部只看到一层
    // （写在文件底部，等价于 pub use crate::x::y;）
    println!();

    // 可见性一览：
    // 无 pub        仅当前模块
    // pub(crate)    当前 crate 内
    // pub(super)    父模块
    // pub           任何地方
    println!("记住：模块树靠 pub 控制谁能访问，比 Python 的 _ 下划线更显式。");

    println!("\n动手练习：");
    println!("  1. 在 strategy 模块里加一个 `pub fn priority() -> u8`。");
    println!("  2. 用 pub(super) 限制某函数的可见范围，体验编译错误。");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 裁剪上限() {
        assert_eq!(strategy::clamp(3, 5), 3);
        assert_eq!(strategy::clamp(9, 5), 5);
    }

    #[test]
    fn 跨模块调用() {
        assert_eq!(router::call(), "pong from provider");
        assert_eq!(upstream_provider::ping(), "pong from provider");
    }
}
