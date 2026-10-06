//! 第 8 课：错误处理 —— Result、`?`、自定义错误类型
//!
//! 运行：cargo run -- 8

use std::error::Error;
use std::fmt;
use std::num::ParseIntError;

/// 标准库的错误类型：带一个描述性枚举 + 人类可读文本 + 可选的「原因」。
#[derive(Debug, Clone, PartialEq)]
pub enum ConfigError {
    MissingKey { name: String },
    BadNumber { field: String, raw: String },
    OutOfRange { field: String, value: u16 },
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingKey { name } => write!(f, "缺少必需的配置项：{name}"),
            Self::BadNumber { field, raw } => write!(f, "{field} 不是合法数字：{raw:?}"),
            Self::OutOfRange { field, value } => write!(f, "{field} 超出范围：{value}"),
        }
    }
}

/// 为自己的错误实现标准库 Error trait，这样它能进入 `Box<dyn Error>`，
/// 并且可以用 `source()` 串起底层原因。
impl Error for ConfigError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::BadNumber { .. } => Some(&PARSE_HINT),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub struct ParseHint;
impl fmt::Display for ParseHint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "请使用十进制数字")
    }
}
impl Error for ParseHint {}
static PARSE_HINT: ParseHint = ParseHint;

/// 一个配置项
#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    pub port: u16,
    pub upstream: String,
}

/// 用 `?` 逐层上抛错误：这是 Rust 错误处理的主力写法。
/// 规则：`?` 遇到 Err 就立刻 return Err(转换后的错误)。
pub fn parse_config(key: Option<&str>, port_raw: &str, upstream: &str) -> Result<Config, ConfigError> {
    // `?` 在这里是「提前返回 None / Err」的捷径
    let key = key.ok_or_else(|| ConfigError::MissingKey {
        name: "GATEWAY_KEY".to_string(),
    })?;

    let port: u16 = port_raw
        .parse()
        .map_err(|_e: ParseIntError| ConfigError::BadNumber {
            field: format!("PORT({key})"),
            raw: port_raw.to_string(),
        })?;

    if port < 1024 {
        return Err(ConfigError::OutOfRange {
            field: "PORT".to_string(),
            value: port,
        });
    }

    Ok(Config {
        port,
        upstream: upstream.to_string(),
    })
}

/// `?` 也能用在 main 里：main 返回 Result<(), Box<dyn Error>> 即可。
/// 注意把 Result 的错误类型显式写成 Box<dyn Error>。
pub fn try_main() -> Result<(), Box<dyn Error>> {
    let cfg = parse_config(Some("sk-test"), "8787", "openrouter")?;
    println!("main 也用上了 ?：{cfg:?}");
    Ok(())
}

/// 对比：`Box<dyn Error>` 是「什么错都能装」的万能盒子。
fn any_error() -> Result<(), Box<dyn Error>> {
    let _: u32 = "42".parse::<u32>()?; // ParseIntError 自动装进 Box
    Ok(())
}

pub fn run() {
    println!("== 第 8 课：错误处理 ==\n");

    // ---- Result 是枚举，不是异常 -------------------------------------------
    let good: Result<u32, ParseIntError> = "42".parse();
    let bad: Result<u32, ParseIntError> = "4x2".parse();
    println!("{good:?} / {bad:?}");
    match good {
        Ok(n) => println!("成功：{n}"),
        Err(e) => println!("失败：{e}"),
    }
    println!();

    // ---- 自定义错误：match 版 ---------------------------------------------
    match parse_config(Some("sk-1"), "8787", "opencode") {
        Ok(cfg) => println!("解析成功：{cfg:?}"),
        Err(e) => println!("解析失败：{e}（source: {:?}）", e.source().map(|s| s.to_string())),
    }
    println!();

    // ---- 用 ? 写出的清爽版本 ---------------------------------------------
    println!("{:?}", parse_config(None, "8787", "opencode"));
    println!("{:?}", parse_config(Some("sk-1"), "abc", "opencode"));
    println!("{:?}", parse_config(Some("sk-1"), "80", "opencode"));
    println!();

    // ---- panic：不可恢复的错误 --------------------------------------------
    // panic!("不该走到这里");   // 立刻终止程序，打印位置
    let v: Vec<u8> = vec![1, 2, 3];
    let got = v.get(10).copied().unwrap_or_default();
    println!("get(10).unwrap_or_default() = {got}（安全兜底）");
    // let boom = v.get(10).unwrap();  // 取消注释：panic: called `Option::unwrap()` on a `None` value
    println!();

    // 什么时候用 panic，什么时候用 Result？
    // - panic：程序内部 bug、不可恢复
    // - Result：用户输入 / IO / 网络等外部因素
    println!("main 用 ? 的结果：{:?}", try_main());
    println!("Box<dyn Error> 用 ? 的结果：{:?}", any_error());
    println!();

    // 线程/库代码里把 panic 转成 Result（了解即可）
    let caught: Result<i32, _> = std::panic::catch_unwind(|| {
        if std::env::args().count() > 99 {
            panic!("不该到这里");
        }
        1
    })
    .map_err(|_| "内部 panic");
    println!("catch_unwind 演示：{caught:?}");

    println!("\n动手练习：");
    println!("  1. 给 ConfigError 加一个 MissingUpstream 变体，并补全所有 match 分支。");
    println!("  2. 写 `fn load_port(path: &str) -> Result<u16, Box<dyn Error>>`，读文件解析端口。");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 正常解析() {
        let cfg = parse_config(Some("sk-1"), "8787", "openrouter").unwrap();
        assert_eq!(cfg.port, 8787);
        assert_eq!(cfg.upstream, "openrouter");
    }

    #[test]
    fn 缺_key_报错() {
        let err = parse_config(None, "8787", "openrouter").unwrap_err();
        assert_eq!(
            err,
            ConfigError::MissingKey { name: "GATEWAY_KEY".into() }
        );
        assert!(err.to_string().contains("GATEWAY_KEY"));
    }

    #[test]
    fn 端口范围检查() {
        assert!(matches!(
            parse_config(Some("k"), "80", "x"),
            Err(ConfigError::OutOfRange { .. })
        ));
    }

    #[test]
    fn 错误有_source_链() {
        let err = parse_config(Some("k"), "abc", "x").unwrap_err();
        assert!(err.source().is_some());
    }
}
