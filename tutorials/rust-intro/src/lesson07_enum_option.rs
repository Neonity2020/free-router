//! 第 7 课：enum、Option、if let / while let
//!
//! 运行：cargo run -- 7

use std::fmt;

/// Rust 的 enum 是**带数据的**标签联合体（tagged union），
/// 比「一堆常量」强大得多：每个分支可以携带不同类型的数据。
#[derive(Debug, Clone, PartialEq)]
pub enum Upstream {
    OpenRouter,
    OpenCode,
    CommandCode {
        /// 每个 Command Code Key 只能尝试一次
        keys: Vec<String>,
    },
}

impl fmt::Display for Upstream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OpenRouter => write!(f, "openrouter"),
            Self::OpenCode => write!(f, "opencode"),
            Self::CommandCode { keys } => write!(f, "commandcode({} 个 key)", keys.len()),
        }
    }
}

impl Upstream {
    /// 标准库风格的转换：无效输入不 panic，而是返回 None
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "openrouter" => Some(Self::OpenRouter),
            "opencode" => Some(Self::OpenCode),
            "commandcode" => Some(Self::CommandCode {
                keys: vec!["sk-1".into()],
            }),
            _ => None,
        }
    }
}

/// Option<T>：标准库定义的两态枚举，表示「可能有一个 T，也可能没有」
#[derive(Debug, Clone, PartialEq)]
pub enum MyOption<T> {
    None,
    Some(T),
}

/// 从 Option 里安全取值，给默认值。
pub fn port_or_default(name: &str) -> u16 {
    match name {
        "prod" => 8787,
        _ => 8080,
    }
}

/// 解析端口字符串：数字 → u16，其它 → None
pub fn parse_port(s: &str) -> Option<u16> {
    s.parse::<u16>().ok() // .ok() 把 Result 变成 Option
}

pub fn run() {
    println!("== 第 7 课：enum / Option ==\n");

    // ---- 带数据的 enum ----------------------------------------------------
    let ups = vec![
        Upstream::OpenRouter,
        Upstream::OpenCode,
        Upstream::CommandCode {
            keys: vec!["sk-a".into(), "sk-b".into()],
        },
    ];
    for u in &ups {
        println!("{u}  (Debug: {u:?})");
    }
    println!();

    // 用 match 检查每个分支，防止「新增分支忘了处理」
    fn describe(u: &Upstream) -> &'static str {
        match u {
            Upstream::OpenRouter => "云端聚合站",
            Upstream::OpenCode => "开放平台",
            Upstream::CommandCode { keys } => {
                if keys.is_empty() {
                    "没配 key"
                } else {
                    "本地 Key 池"
                }
            }
        }
    }
    println!("{:?}", ups.iter().map(describe).collect::<Vec<_>>());
    println!();

    // ---- Option：没有 null，用枚举显式表达「可能没有」 ----------------------
    let a: Option<u16> = parse_port("8787");
    let b: Option<u16> = parse_port("not-a-port");
    println!("parse_port(\"8787\") = {a:?}");
    println!("parse_port(\"xxx\")  = {b:?}");
    println!("a.unwrap_or(0) = {}", a.unwrap_or(0));
    println!("b.unwrap_or(80) = {}", b.unwrap_or(80));
    println!("port_or_default(\"prod\") = {}", port_or_default("prod"));
    println!();

    // ---- 常用组合子 -------------------------------------------------------
    println!("a.map(|p| p + 1) = {:?}", a.map(|p| p + 1));
    println!("b.map(|p| p + 1) = {:?}", b.map(|p| p + 1));
    // 格式化字符串里的 { } 是占位符语法，要原样打印代码里的花括号必须写成 {{ }}
    println!(
        "a.and_then(|p| if p > 100 {{ Some(p) }} else {{ None }}) = {:?}",
        a.and_then(|p| if p > 100 { Some(p) } else { None })
    );
    println!("a.is_some()={} b.is_none()={}", a.is_some(), b.is_none());
    // println!("{:?}", a.expect("端口必须有效")); // None 时 panic 并带上你的提示
    println!();

    // ---- if let：只关心一个分支时的简写 -----------------------------------
    if let Some(p) = a {
        println!("if let 匹配到端口 {p}");
    }
    if let Upstream::CommandCode { keys } = &ups[2] {
        println!("if let 取出 CommandCode 的 keys：{keys:?}");
    }
    println!();

    // ---- while let：循环直到枚举耗尽 ---------------------------------------
    let mut opt = Some(3);
    while let Some(n) = opt {
        println!("while let 得到 {n}");
        opt = if n > 1 { Some(n - 1) } else { None };
    }
    println!();

    // ---- 自己写一个 Option -------------------------------------------------
    println!("MyOption: {:?}", MyOption::Some(5));
    println!();

    // ---- 常见错误：以为 Option 是 null --------------------------------------
    // let x: Option<i32> = None;
    // println!("{}", x + 1); // error: cannot add `Option<i32>` to integer
    // 必须先 unwrap / unwrap_or / match 把它变成真值。

    println!("\n动手练习：");
    println!("  1. 给 Upstream 加一个 `Custom(String)` 分支，让它也能 Display。");
    println!("  2. 写 `fn longest<'a>(a: &'a str, b: &'a str) -> &'a str` 用 Option 表达。");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 枚举携带数据() {
        let u = Upstream::CommandCode { keys: vec!["a".into()] };
        assert_eq!(u.to_string(), "commandcode(1 个 key)");
        assert_eq!(Upstream::OpenCode.to_string(), "opencode");
    }

    #[test]
    fn 解析上游() {
        assert_eq!(Upstream::parse("openrouter"), Some(Upstream::OpenRouter));
        assert_eq!(Upstream::parse("???"), None);
    }

    #[test]
    fn 端口解析() {
        assert_eq!(parse_port("8787"), Some(8787));
        assert_eq!(parse_port("-1"), None);
        assert_eq!(parse_port("99999"), None); // 超范围 → None，不 panic
    }

    #[test]
    fn option_组合子() {
        let a = Some(1u8);
        assert_eq!(a.and_then(|x| x.checked_add(1)), Some(2));
        assert_eq!(a.and_then(|x| x.checked_add(255)), None);
        assert_eq!(Some(1u8).map(|x| x * 10), Some(10));
    }
}
