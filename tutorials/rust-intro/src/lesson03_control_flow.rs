//! 第 3 课：控制流 —— if / 循环 / match
//!
//! 运行：cargo run -- 3

pub fn run() {
    println!("== 第 3 课：控制流 ==\n");

    // ---- if 是表达式 ------------------------------------------------------
    // Rust 的 if/else 必须有一个值，和块里最后一个表达式一致。
    let n = 7;
    let kind = if n % 2 == 0 { "偶数" } else { "奇数" };
    println!("{n} 是{kind}");

    // 条件必须是 bool，不能用数字或字符串代替（对比 C 的 if (x)）
    // let n = 7;
    // if n { } // 报错：expected `bool`, found integer
    println!();

    // ---- loop / while / for ----------------------------------------------
    let mut i = 0;
    let doubled = loop {
        i += 1;
        if i > 3 {
            break i * 2; // loop 可以带值 break出去
        }
    };
    println!("loop 带值返回：{doubled}");

    let mut n = 3;
    while n > 0 {
        print!("{n} ");
        n -= 1;
    }
    println!("(while 结束)");

    for x in 1..=5 {
        print!("{x} ");
    }
    println!("(for 闭区间 1..=5)");
    for x in [10, 20, 30].iter() {
        print!("{x} ");
    }
    println!("(for 遍历数组)");
    println!();

    // ---- match：Rust 的瑞士军刀 -------------------------------------------
    let code = 200;
    let msg = match code {
        200 => "OK",
        201 | 202 => "已接受", // 多个值用 |
        400..=499 => "客户端错误", // 范围
        500..=599 => "服务端错误",
        _ => "未知", // 兜底，必写（除非范围已覆盖全部情况）
    };
    println!("HTTP {code} => {msg}");
    println!();

    // match 也能做范围、绑定、甚至调用函数
    // 注意：所有分支的返回类型必须一致。下面有的分支是 &str 字面量，
    // 有的分支用 format! 造出 String，所以统一 to_string() 对齐类型。
    let n = 42;
    let bucket = match n {
        0 => "零".to_string(),
        1..=9 => "个位数".to_string(),
        10..=99 => {
            // 块里可以写多行，最后一个表达式就是结果
            let label = if n % 2 == 0 { "偶" } else { "奇" };
            format!("两位数（{label}）")
        }
        _ => "更大".to_string(),
    };
    println!("{n} -> {bucket}");
    println!();

    // ---- loop 标签：多层循环里 break 指定层 --------------------------------
    let mut found = None;
    'outer: for x in 1..10 {
        for y in 1..10 {
            if x * y == 42 {
                found = Some((x, y));
                break 'outer; // 直接跳出外层
            }
        }
    }
    println!("找到因子：{found:?}");
    println!("\n动手练习：");
    println!("  1. 用 loop 写一个倒计时并打印 T-3 T-2 T-1。");
    println!("  2. 写一个 fizzbuzz 打印 1..=20 的结果。");
}

pub fn fizzbuzz(n: u32) -> String {
    match (n % 3 == 0, n % 5 == 0) {
        (true, true) => "FizzBuzz".to_string(),
        (true, false) => "Fizz".to_string(),
        (false, true) => "Buzz".to_string(),
        (false, false) => n.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fizzbuzz_规则() {
        assert_eq!(fizzbuzz(1), "1");
        assert_eq!(fizzbuzz(3), "Fizz");
        assert_eq!(fizzbuzz(5), "Buzz");
        assert_eq!(fizzbuzz(15), "FizzBuzz");
    }

    #[test]
    fn loop_可以返回值() {
        let mut i = 0;
        let r = loop {
            i += 1;
            if i == 5 {
                break i;
            }
        };
        assert_eq!(r, 5);
    }
}
