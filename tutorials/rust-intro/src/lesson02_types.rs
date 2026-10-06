//! 第 2 课：基本类型、复合类型、String 与 &str
//!
//! 运行：cargo run -- 2

use std::fmt;

/// 一个自定义类型，用来演示：类型也可以像函数一样把方法挂在身上（第 6 课会细讲）。
#[derive(Debug, Clone, PartialEq)]
pub struct Temp(f64);

impl Temp {
    pub fn from_f(celsius: f64) -> Self {
        Self(celsius)
    }

    pub fn as_f(&self) -> f64 {
        self.0 * 9.0 / 5.0 + 32.0
    }
}

impl fmt::Display for Temp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.1}°F", self.as_f())
    }
}

/// Rust 没有「具名元组」：想要字段名，就得定义 struct（第 6 课细讲）。
#[derive(Debug, Clone, PartialEq)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

pub fn run() {
    println!("== 第 2 课：类型 ==\n");

    // ---- 整数与浮点 -------------------------------------------------------
    let a: i8 = 127; // 有符号 8 位：-128..=127
    let b: u8 = 255; // 无符号 8 位：0..=255
    println!("i8 {a} / u8 {b}");
    // let overflow: u8 = 255 + 1; // 调试模式下会 panic：attempt to add with overflow

    // i32 是默认整数类型，f64 是默认浮点类型。
    let default_int = 42;
    let default_float = 42.0;
    println!("默认类型：{} 是 i32，{} 是 f64", default_int, default_float);

    // 整数字面量也可以直接写成想要的类型
    let hundred: u64 = 100_000_000;
    println!("下划线只是给人看的：{hundred}");
    println!();

    // ---- 布尔与字符 -------------------------------------------------------
    let rust_is_cool: bool = true;
    let emoji = '🦀'; // char 是一个 4 字节的 Unicode 标量值
    println!("{rust_is_cool} {emoji}");
    println!();

    // ---- 元组：把几个不同类型的值绑在一起 --------------------------------
    let user = ("小菜", 30, 1.75);
    let (name, age, height) = user; // 解构（destructuring）
    println!("{name} / {age} / {height}");

    // 元组按位置取：.0 / .1 / .2
    let point = (3, 4);
    println!("元组按位置取：x={} y={}", point.0, point.1);

    // Rust 没有「具名元组」——需要字段名就定义 struct
    let named = Point { x: 3, y: 4 };
    println!("struct 具名字段：x={} y={}", named.x, named.y);
    println!();

    // ---- 数组 & 元组 ------------------------------------------------------
    // 数组长度是类型的一部分：[i32; 3] 和 [i32; 4] 是不同类型。
    let arr: [i32; 3] = [1, 2, 3];
    println!("数组：{arr:?} 长度 {}", arr.len());
    println!();

    // ---- String vs &str：最容易卡住的地方 ---------------------------------
    // &str  = 借来的、不可变的字符串字面量（编译进二进制，不含长度信息）
    // String = 拥有数据的、可增长可修改的 UTF-8 缓冲区（堆上）
    let mut owned: String = String::from("hello"); // 拥有
    owned.push_str(", rust");
    let borrowed: &str = "hello, world"; // 借用
    println!("String(拥有): {owned}   长度 {}", owned.len());
    println!("&str(借用):  {borrowed}   长度 {}", borrowed.len());
    // println!("{}", borrowed.len()); // ⚠️ 这里写错了会报 trait bound 错误，编译期就拦住你
    println!();

    // ---- 自定义类型 -------------------------------------------------------
    let temp = Temp::from_f(100.0);
    println!("100°C = {temp}（Display 用于 {{}}，Debug 用于 {{:?}}）");
    println!("结构体内容 {:?}，它实现了 Clone 和 PartialEq", temp.clone());

    println!("\n动手练习：");
    println!("  1. 用 &str 创建一个 String，再取它的切片 &str[0..5]。");
    println!("  2. 故意写 `let s: String = \"hi\";`，看编译器如何引导你改成 .to_string()。");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 摄氏转华氏() {
        assert_eq!(Temp::from_f(0.0).as_f(), 32.0);
        assert!((Temp::from_f(100.0).as_f() - 212.0).abs() < 1e-9);
    }

    #[test]
    fn 元组与具名字段() {
        let point = (3, 4);
        assert_eq!(point.0, 3);
        let named = Point { x: 3, y: 4 };
        assert_eq!(named.y, 4);
    }

    #[test]
    fn 元组解构() {
        let (name, age, height): (&str, i32, f64) = ("小菜", 30, 1.75);
        assert_eq!(name, "小菜");
        assert_eq!(age, 30);
        assert!((height - 1.75).abs() < 1e-9);
    }

    #[test]
    fn string_拥有数据() {
        let mut s = String::from("hello");
        s.push_str(", rust");
        assert_eq!(s, "hello, rust");
        assert_eq!(&s[0..5], "hello");
        assert_eq!("hello, world".len(), 12); // len 返回的是字节数！
    }
}
