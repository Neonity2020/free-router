//! 第 4 课：函数、语句 vs 表达式、返回元组
//!
//! 运行：cargo run -- 4

/// 最普通的函数：参数类型和返回值类型都要写出来。
pub fn add(a: i32, b: i32) -> i32 {
    a + b // 注意：没有分号！这就是返回值
}

/// 函数体里的 `5;` 是一个**语句**，它不产生值；
/// 最后的 `a + b` 是**表达式**，它产生值，也就是返回值。
pub fn add_with_statement(a: i32, b: i32) -> i32 {
    let doubled_a = a * 2; // 语句（结尾分号，结果被丢弃）
    doubled_a + b // 表达式
}

/// 默认返回单元类型 `()`，相当于别的语言的 void。
pub fn print_banner() {
    println!("---- 函数演示 ----");
}

/// 想返回多个值？返回元组即可。
pub fn min_max(values: &[i32]) -> (i32, i32) {
    let mut min = values[0];
    let mut max = values[0];
    for &v in values.iter() {
        if v < min {
            min = v;
        }
        if v > max {
            max = v;
        }
    }
    (min, max)
}

/// 提前返回用 `return`，但 99% 的情况 Rust 希望你用表达式。
pub fn first_even(values: &[i32]) -> Option<i32> {
    for &v in values {
        if v % 2 == 0 {
            return Some(v);
        }
    }
    None
}

pub fn run() {
    println!("== 第 4 课：函数 ==\n");

    print_banner();
    println!("add(2, 3) = {}", add(2, 3));
    println!("add_with_statement(2, 3) = {}", add_with_statement(2, 3));
    // println!("{}", add(2, 3;)); // 取消分号会报 mismatched types

    let data = [5, 3, 9, -2, 7];
    let (min, max) = min_max(&data);
    println!("min={min} max={max}");

    println!("第一个偶数：{:?}", first_even(&data));
    println!();

    // ---- 函数也是值：可以赋值给变量、当参数传 ---------------------------
    let f = add;
    println!("函数指针调用：{}", f(1, 1));
    println!();

    // ---- 递归 -------------------------------------------------------------
    println!("factorial(5) = {}", factorial(5));
    println!();

    // ---- 无递归实现阶乘：用迭代器（第 12 课会讲得更细）--------------------
    println!("iter_factorial(5) = {}", iter_factorial(5));
    println!("\n动手练习：");
    println!("  1. 写一个 `is_prime(n: u64) -> bool`，先试奇数因子。");
    println!("  2. 写 `fn swap(pair: (i32, i32)) -> (i32, i32)`，交换元组两项。");
}

/// 递归：函数调用自己，必须有明确的终止条件。
pub fn factorial(n: u64) -> u64 {
    if n <= 1 {
        1
    } else {
        n * factorial(n - 1) // 尾调用，Rust 会自动优化成循环
    }
}

/// 迭代版本：栈安全，推荐写法。
pub fn iter_factorial(n: u64) -> u64 {
    (1..=n).product()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 加法() {
        assert_eq!(add(2, 3), 5);
        assert_eq!(add_with_statement(2, 3), 7);
    }

    #[test]
    fn 最值() {
        assert_eq!(min_max(&[5, 3, 9, -2, 7]), (-2, 9));
        assert_eq!(min_max(&[4]), (4, 4));
    }

    #[test]
    fn 找偶数() {
        assert_eq!(first_even(&[1, 4, 6]), Some(4));
        assert_eq!(first_even(&[1, 3]), None);
    }

    #[test]
    fn 阶乘() {
        assert_eq!(factorial(0), 1);
        assert_eq!(factorial(5), 120);
        assert_eq!(iter_factorial(5), 120);
    }
}
