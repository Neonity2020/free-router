//! 第 9 课：泛型、trait、trait 对象（dyn）
//!
//! 运行：cargo run -- 9

use std::fmt::{self, Display};

/// trait = 一组「行为契约」。类似 Java 的 interface，但可以有默认实现。
pub trait Upstream {
    /// 必须实现的方法
    fn name(&self) -> &str;

    /// 带默认实现的方法，实现者可以不写
    fn health_check(&self) -> Result<(), String> {
        if self.name().is_empty() {
            Err("上游名为空".to_string())
        } else {
            Ok(())
        }
    }

    /// 返回所有候选上游（含自己）
    fn fallbacks(&self) -> Vec<String> {
        vec![self.name().to_string()]
    }
}

/// 泛型函数 + trait bound：T 必须实现 Summable
pub fn sum_all<T: Summable + std::ops::Add<Output = T>>(items: &[T]) -> T {
    items.iter().fold(T::zero(), |acc, x| acc + x.clone())
}

pub trait Summable: Clone {
    fn zero() -> Self;
    fn value(&self) -> f64;
}

impl Summable for u32 {
    fn zero() -> Self { 0 }
    fn value(&self) -> f64 { *self as f64 }
}
impl Summable for f64 {
    fn zero() -> Self { 0.0 }
    fn value(&self) -> f64 { *self }
}

/// 泛型结构体
#[derive(Debug, Clone, PartialEq)]
pub struct Response<T> {
    pub status: u16,
    pub body: T,
}

impl<T: Display> Response<T> {
    pub fn describe(&self) -> String {
        format!("HTTP {} -> {}", self.status, self.body)
    }
}

/// where 子句：当 trait bound 很长时更好读
pub fn pick<T>(items: &[T], pred: impl Fn(&T) -> bool) -> Option<&T>
where
    T: Clone + Display,
{
    items.iter().find(|x| pred(x))
}

/// trait 对象：运行时多态（类似 Java 的接口引用），需要 `dyn`。
pub fn describe_any(u: &dyn Upstream) -> String {
    match u.health_check() {
        Ok(()) => format!("{} 可用", u.name()),
        Err(e) => format!("{} 不可用：{e}", u.name()),
    }
}

#[derive(Debug)]
pub struct OpenRouter;
#[derive(Debug)]
pub struct OpenCode {
    pub keys: usize,
}

impl Upstream for OpenRouter {
    fn name(&self) -> &str { "openrouter" }
}
impl Upstream for OpenCode {
    fn name(&self) -> &str { "opencode" }
    fn health_check(&self) -> Result<(), String> {
        if self.keys == 0 {
            Err("没配置 key".to_string())
        } else {
            Ok(())
        }
    }
    fn fallbacks(&self) -> Vec<String> {
        vec!["opencode".into(), "commandcode".into()]
    }
}

/// 实现别人的 trait：给本地类型接上外部能力（孤儿规则只限制外部类型+外部 trait）
impl Display for OpenRouter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "OpenRouter")
    }
}

/// 为 Vec 提供扩展方法：trait 的常见用法（和迭代器风格一致）
pub trait CountBytes {
    fn byte_len(&self) -> usize;
}

impl CountBytes for str {
    fn byte_len(&self) -> usize {
        self.len()
    }
}

pub fn run() {
    println!("== 第 9 课：泛型 / trait ==\n");

    let items = vec![1u32, 2, 3];
    println!("sum_all(u32) = {}", sum_all(&items));
    let fs = vec![1.5f64, 2.5];
    println!("sum_all(f64) = {}", sum_all(&fs));
    // 不是所有类型都能进来：String 既没实现 Summable，也没有 String + String，
    // 所以下面这行根本编译不过 —— 这正是 trait bound 的作用：
    // 把错误从运行时提前到编译期，并直接告诉你缺哪个 impl。
    // let words = vec![String::from("a"), String::from("bb")];
    // println!("sum_all(String) = {}", sum_all(&words)); // ❌ the trait bound `String: Summable` is not satisfied
    println!();

    // 泛型结构体，实例化后才知道具体类型
    let r: Response<String> = Response { status: 200, body: "ok".into() };
    let j: Response<Vec<u8>> = Response { status: 500, body: vec![] };
    println!("{}", r.describe());
    println!("{:?}", j);
    println!();

    // trait 默认方法
    let or = OpenRouter;
    let oc = OpenCode { keys: 0 };
    println!("{}", describe_any(&or));
    println!("{}", describe_any(&oc));
    println!("OpenCode 的备选：{:?}", oc.fallbacks());
    println!();

    // trait 对象可以装在不同类型的值后面（需要 Box / &）
    let dyns: Vec<Box<dyn Upstream>> = vec![Box::new(OpenRouter), Box::new(OpenCode { keys: 2 })];
    for d in &dyns {
        println!("  {}", d.name());
    }
    println!();

    // impl Trait 作为参数类型：其实等价于匿名泛型参数
    let picked = pick(&["alpha", "beta", "gamma"], |s| s.starts_with('b'));
    println!("pick 结果：{picked:?}");
    println!();

    // 为 &str 实现自定义 trait
    println!("\"你好\".byte_len() = {}", "你好".byte_len());
    println!("OpenRouter 实现了 Display：{or}");

    println!("\n动手练习：");
    println!("  1. 定义 trait `Retry`，给 OpenCode 实现它。");
    println!("  2. 写 `fn max_by<T: PartialOrd>(v: &[T]) -> Option<&T>`，用泛型实现。");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 泛型求和() {
        assert_eq!(sum_all(&[1u32, 2, 3]), 6);
        assert!((sum_all(&[1.5f64, 2.5]) - 4.0).abs() < 1e-9);
        assert_eq!(sum_all::<u32>(&[]), 0);
    }

    #[test]
    fn trait_默认方法() {
        assert!(OpenRouter.health_check().is_ok());
        assert!(OpenCode { keys: 1 }.health_check().is_ok());
        assert_eq!(OpenCode { keys: 0 }.health_check(), Err("没配置 key".into()));
    }

    #[test]
    fn dyn_多态() {
        let v: Vec<&dyn Upstream> = vec![&OpenRouter, &OpenCode { keys: 0 }];
        let names: Vec<String> = v.iter().map(|u| u.name().to_string()).collect();
        assert_eq!(names, vec!["openrouter", "opencode"]);
    }

    #[test]
    fn where_子句() {
        assert_eq!(pick(&[1, 2, 3], |x| *x > 2), Some(&3));
        assert_eq!(pick(&["a"], |s| *s == "b"), None);
    }
}
