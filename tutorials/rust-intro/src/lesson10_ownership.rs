//! 第 10 课：所有权 / 借用 —— Rust 最重要的一课
//!
//! 运行：cargo run -- 10
//!
//! 三条规则：
//!   1. 每个值有且只有一个「所有者」，所有者离开作用域，值被 drop。
//!   2. 同一时刻可以有多个不可变引用，或者一个可变引用，不能同时存在。
//!   3. 引用必须永远有效（不能指向已释放的数据）—— 编译期用生命周期检查。

use std::collections::HashMap;

#[derive(Debug)]
pub struct Server {
    pub name: String,
    pub hits: u32,
}

/// 1) 移动（move）：把所有权交出去
pub fn takes_ownership(s: String) -> usize {
    s.len()
} // s 在这里被 drop

/// 2) 借用：只读引用，不给所有权
pub fn borrows(s: &String) -> usize {
    s.len()
} // 借用结束，s 的所有者还在调用方

/// 3) 可变借用：修改内容
pub fn mutates(s: &mut String) {
    s.push_str("-patched");
}

/// 错误示范：既读又写同一个值
pub fn bad(s: &mut String) {
    let r = &s;   // 不可变借用开始
    // s.push('x'); // error[E0502]: 冲突 —— 可变借用时存在不可变借用
    println!("{r}");
} // 不可变借用结束，这里之后才可以可变借用

/// 返回引用时，编译器需要知道它来自谁 → 这里用省略规则自动推断
pub fn longest<'a>(a: &'a str, b: &'a str) -> &'a str {
    if a.len() >= b.len() { a } else { b }
}

/// Copy 类型在赋值时是复制而不是移动
pub fn copy_demo() {
    let x = 5; // i32 是 Copy
    let y = x;
    println!("x 仍然可用：{x}，y = {y}");

    let s = String::from("hi");
    // let s2 = s;  // 移动
    let s2 = s.clone(); // 想两份就显式克隆
    println!("s = {s}, s2 = {s2}");
}

/// 借用规则在真实代码里的样子：批量更新 HashMap
pub fn bump_all(map: &mut HashMap<String, u32>, by: u32) {
    for v in map.values_mut() {
        *v += by;
    }
}

/// 部分移动（partial move）：从结构体里把一个字段「拿走」
pub fn take_name(s: Server) -> String {
    s.name // 把 name 移出；因为 Server 没实现 Drop，这里允许
}

pub fn run() {
    println!("== 第 10 课：所有权与借用 ==\n");

    // ---- 移动 -------------------------------------------------------------
    let s1 = String::from("hello");
    let len = takes_ownership(s1);
    println!("长度 {len}");
    // println!("{s1}"); // error[E0382]: borrow of moved value: `s1`
    println!("（s1 的所有权已经交给 takes_ownership，不能再用）");
    println!();

    // ---- 借用 -------------------------------------------------------------
    let s2 = String::from("hello rust");
    println!("borrow 长度 {}，之后 s2 还能用：{s2}", borrows(&s2));

    let mut s3 = String::from("v1");
    mutates(&mut s3);
    println!("可变借用后：{s3}");
    println!();

    // ---- Copy vs Clone -----------------------------------------------------
    copy_demo();
    println!();

    // ---- 借用冲突（编译期拦截）-------------------------------------------
    let mut text = String::from("data");
    bad(&mut text);
    text.push('!');
    println!("借用冲突演示后：{text}");
    println!();

    // ---- 多个不可变借用可以共存 -------------------------------------------
    let data = vec![1, 2, 3];
    let a = &data;
    let b = &data;
    println!("两个不可变借用同时存在：{} + {}", a.len(), b.len());
    println!();

    // ---- 可变借用期间不能有别的引用 ---------------------------------------
    let mut counters = HashMap::from([("a".to_string(), 1u32), ("b".to_string(), 2)]);
    bump_all(&mut counters, 10);
    println!("{counters:?}（排序后输出）");
    let mut sorted: Vec<_> = counters.iter().collect();
    sorted.sort();
    println!("排序后 {:?}", sorted);
    println!();

    // ---- 部分移动 ---------------------------------------------------------
    // 从结构体里把一个字段「拿走」：server 整体之后不能再用，
    // 但没被移走的字段可以先读出来（hits 是 u32，Copy 直接复制）。
    let server = Server { name: "gw".into(), hits: 7 };
    let hits = server.hits;
    let name = take_name(server);
    println!("部分移动：name = {name}，之前读到的 hits = {hits}");
    println!();

    // ---- 生命周期简化 ------------------------------------------------------
    println!("longest = {}", longest("apple", "banana"));

    println!("\n动手练习：");
    println!("  1. 写一个 `fn split_at_space(s: &str) -> (&str, &str)`，只借用不分配。");
    println!("  2. 写 `fn replace_all(text: &mut String, from: &str, to: &str)`，原地替换。");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 移动后原变量不可用() {
        let s = String::from("abc");
        assert_eq!(takes_ownership(s), 3);
    }

    #[test]
    fn 借用后原变量可用() {
        let s = String::from("abcd");
        assert_eq!(borrows(&s), 4);
        assert_eq!(s, "abcd");
    }

    #[test]
    fn 可变借用生效() {
        let mut s = String::from("v1");
        mutates(&mut s);
        assert_eq!(s, "v1-patched");
    }

    #[test]
    fn 生命周期不超输入() {
        assert_eq!(longest("aa", "bbb"), "bbb");
        assert_eq!(longest("aaaa", "b"), "aaaa");
    }

    #[test]
    fn 部分移动() {
        let server = Server { name: "gw".into(), hits: 7 };
        let hits = server.hits; // 先复制出 Copy 字段
        assert_eq!(take_name(server), "gw");
        assert_eq!(hits, 7);
    }

    #[test]
    fn 批量更新() {
        let mut m = HashMap::from([("a".to_string(), 1u32)]);
        bump_all(&mut m, 5);
        assert_eq!(m["a"], 6);
    }
}
