//! 第 16 课：测试怎么写 —— `#[test]`、断言、`cargo test` 的各种用法
//!
//! 运行：cargo run -- 16
//! 跑测试：cargo test
//! 只跑本课：cargo test lesson16
//!
//! Rust 的测试是语言内置的，不需要额外框架：
//! - 测试就是带 `#[test]` 属性的普通函数，放在 `#[cfg(test)] mod tests` 里
//! - `#[cfg(test)]` 保证这段代码只在 `cargo test` 时编译，发布版本里不存在
//! - 断言失败会 panic，测试框架捕获 panic 并标记为失败

/// 文档注释里的代码块会被 `cargo test` 当成测试真的编译并运行（doctest）。
///
/// ```
/// assert_eq!(rust_intro::lesson16_tests::add(1, 2), 3);
/// ```
///
/// doctest 的好处：文档里的例子永远不会过期，因为它自己会被跑一遍。
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

/// 返回 Result 的函数：用 `?` 上抛错误，方便测试里直接 `?`
pub fn parse_port(raw: &str) -> Result<u16, String> {
    let port: u16 = raw.parse().map_err(|_| format!("{raw:?} 不是合法端口"))?;
    if port == 0 {
        return Err("端口 0 不可用".to_string());
    }
    Ok(port)
}

/// 私有函数同样能被同文件的测试直接调用 —— 这是「单元测试」的关键便利
fn is_even(n: i32) -> bool {
    n % 2 == 0
}

pub fn run() {
    println!("== 第 16 课：测试 ==\n");

    println!("测试函数长这样：");
    println!("    #[cfg(test)]");
    println!("    mod tests {{");
    println!("        use super::*;   // 把外层的东西引进来");
    println!();
    println!("        #[test]");
    println!("        fn 加法正确() {{");
    println!("            assert_eq!(add(1, 2), 3);");
    println!("        }}");
    println!("    }}");
    println!();

    println!("常用断言：");
    println!("    assert!(条件)                  条件为假就失败");
    println!("    assert_eq!(左, 右)             不相等就失败，并打印两边的值");
    println!("    assert_ne!(左, 右)             相等就失败");
    println!("    assert!(条件, \"说明 {{}}\", x)  失败时带上你自己的信息");
    println!("    assert_eq!(左, 右, \"端口解析\")  失败时的说明文字");
    println!();

    println!("特殊形态：");
    println!("    #[should_panic]                断言这个测试会 panic");
    println!("    #[should_panic(expected = \"端口\")]  panic 信息必须包含这段文字");
    println!("    fn t() -> Result<(), E> {{ ... }}  用 ? 代替一堆 unwrap，测试也可以返回 Result");
    println!("    #[ignore]                      默认跳过，用 cargo test -- --ignored 单独跑");
    println!();

    println!("几条实践：");
    println!("  - 测试名用中文完全合法，而且读起来更像需求：`fn 缺_key_报错()`");
    println!("  - 一个测试只验证一件事，失败信息才好读");
    println!("  - 先写测试再写实现（TDD）在 Rust 里很顺手：编译不过会直接告诉你要什么类型");
    println!("  - 集成测试放 tests/ 目录，只能访问 pub API；单元测试放文件内的 mod tests，能测私有函数");
    println!();

    // 当场跑一遍，确认这些断言确实成立
    assert_eq!(add(1, 2), 3);
    assert!(is_even(4));
    println!("现场验证：add(1, 2) = {}，is_even(4) = {}", add(1, 2), is_even(4));
    println!("上面这几行本身就是断言：如果不对，这一课会直接 panic。");

    println!("\n动手练习：");
    println!("  1. 给 parse_port 补一个测试：\"0\" 必须返回 Err。");
    println!("  2. 写一个 #[should_panic] 测试，验证除零会 panic。");
    println!("  3. 用 `cargo test -- --nocapture` 看 println! 的输出（默认会被吞掉）。");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 加法正确() {
        assert_eq!(add(1, 2), 3);
        assert_eq!(add(-1, 1), 0);
    }

    #[test]
    fn 断言带说明文字() {
        let port = parse_port("8787").unwrap();
        assert_eq!(port, 8787, "解析出的端口不对");
        assert!(is_even(i32::from(port) - 1), "8787 的前一个数应该是偶数");
    }

    #[test]
    fn 端口解析错误() {
        assert_eq!(parse_port("abc"), Err("\"abc\" 不是合法端口".to_string()));
        assert!(parse_port("0").is_err());
        assert!(parse_port("70000").is_err(), "超出 u16 范围");
    }

    #[test]
    fn 测试也可以返回_result() -> Result<(), String> {
        // 这样能用 ? 代替 unwrap，失败时错误信息更清楚
        let port = parse_port("443")?;
        assert_eq!(port, 443);
        Ok(())
    }

    #[test]
    #[should_panic(expected = "端口 0 不可用")]
    fn 零端口必须_panic() {
        // 这个函数返回 Err，这里故意 unwrap 触发 panic，
        // 且 panic 信息必须包含 expected 里的文字，否则测试仍然失败。
        parse_port("0").unwrap();
    }

    #[test]
    #[ignore = "演示用：默认跳过，cargo test -- --ignored 才会跑"]
    fn 被忽略的慢测试() {
        // 典型用途：耗时很久、依赖网络或需要真实密钥的测试
        let values = vec![1, 2, 3];
        assert_eq!(values.iter().sum::<i32>(), 6, "如果这行真的跑了，说明你用了 --ignored");
    }
}
