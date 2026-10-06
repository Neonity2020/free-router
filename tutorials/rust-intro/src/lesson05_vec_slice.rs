//! 第 5 课：数组、切片（slice）、Vec
//!
//! 运行：cargo run -- 5

use std::collections::HashMap;

/// 切片是对已有数据的「视图」，不拥有数据。
/// `&values[0..3]` 借用数组前 3 个元素。
pub fn sum_slice(values: &[i32]) -> i32 {
    values.iter().sum()
}

// 反面教材（不要真写出来，否则整个 crate 编译不过）：
//
// pub fn bad_first_word(text: String) -> &str {
//     let word = text.split(' ').next().unwrap_or("");
//     word   // error[E0106]: missing lifetime specifier
//     // 原因：返回值指向函数内部临时创建的 String，
//     //       函数返回后它就被释放了 → 悬垂引用
// }
/// 正确做法一：让返回值来自入参（生命周期推断，见第 14 课）。
pub fn first_word(text: &str) -> &str {
    text.split(' ').next().unwrap_or("")
}

/// 正确做法二：自己产出一个拥有数据的 String（对比第 2 课的 String vs &str）。
pub fn owned_first_word(text: String) -> String {
    text.split(' ').next().unwrap_or("").to_string()
}

pub fn run() {
    println!("== 第 5 课：数组 / 切片 / Vec ==\n");

    // ---- 数组：长度写在类型里 ---------------------------------------------
    let arr = [1, 2, 3, 4, 5];
    println!("数组 {:?}，长度 {}", arr, arr.len());
    println!("遍历：{:?}", slice_to_vec(&arr));
    println!();

    // ---- 切片：借用一段 ---------------------------------------------------
    let part = &arr[1..4]; // [2, 3, 4]，左闭右开
    println!("切片 {part:?}，长度 {}", part.len());
    println!("sum_slice(&arr) = {}", sum_slice(&arr));
    println!("sum_slice(part) = {}", sum_slice(part));
    // 传切片不需要写长度：`&arr` 会自动转成 `&[i32]`（自动解引用/退化）
    println!();

    // ---- Vec：可增长的动态数组（栈上只有 三个字：ptr/len/cap）------------
    let mut v: Vec<i32> = Vec::new();
    v.push(1);
    v.push(2);
    v.push(3);
    println!("Vec {v:?}  len={} cap={}", v.len(), v.capacity());

    // 更常见的写法
    let names = vec!["alpha", "beta", "gamma"]; // vec! 是标准库提供的宏
    println!("names = {names:?}");

    // 遍历
    for (i, n) in names.iter().enumerate() {
        println!("  [{i}] {n}");
    }
    println!();

    // ---- 常用操作 ---------------------------------------------------------
    let mut nums = vec![3, 1, 4, 1, 5];
    nums.sort(); // 原地排序，函数名像 mut 一样提示你要调用 mut 方法
    println!("排序后 {nums:?}");
    println!("contains(4) = {}", nums.contains(&4));
    println!("iter().sum() = {}", nums.iter().sum::<i32>());
    nums.dedup();
    println!("去重后 {nums:?}");
    println!();

    // ---- 越界：两种处理方式 ------------------------------------------------
    let v = vec![1, 2, 3];
    // println!("{}", v[5]);            // panic：index out of bounds（编译能过，运行时炸）
    println!("v.get(5) = {:?}", v.get(5)); // 安全返回 None
    println!("v.get(1) = {:?}", v.get(1));
    println!();

    // ---- 字符串也是字节数组，用字节切片处理 UTF-8 -------------------------
    let s = "Rust 语言";
    println!("字符串字节长度：{}", s.len());
    println!("首个字节 slice：{:?}", &s.as_bytes()[0..4]); // 切在字符边界上才安全
    // println!("{}", &s[0..1]); // 取消注释会 panic：byte index 1 is not a char boundary

    // 统计每个词出现次数（第 12 课 HashMap 的预告）
    let text = "a b a c b a";
    let mut counter: HashMap<&str, usize> = HashMap::new();
    for w in text.split(' ') {
        *counter.entry(w).or_insert(0) += 1;
    }
    println!("词频 {:?}", counter);

    println!("\n动手练习：");
    println!("  1. 写 `fn second_largest(v: &[i32]) -> Option<i32>`，一次遍历求出第二大值。");
    println!("  2. 写 `fn join(v: &[&str], sep: &str) -> String`，用 Vec 和迭代器拼接。");
}

pub fn slice_to_vec(values: &[i32]) -> Vec<i32> {
    values.to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 切片求和() {
        assert_eq!(sum_slice(&[1, 2, 3]), 6);
        assert_eq!(sum_slice(&[]), 0);
        let arr = [10, 20, 30, 40];
        assert_eq!(sum_slice(&arr[1..3]), 50);
    }

    #[test]
    fn 字符串切片() {
        assert_eq!(first_word("hello rust world"), "hello");
        assert_eq!(first_word(""), "");
        assert_eq!(owned_first_word("hello rust".to_string()), "hello");
    }

    #[test]
    #[should_panic]
    fn 越界会_panic() {
        let v = vec![1, 2, 3];
        let _ = v[5];
    }

    #[test]
    fn 越界_get_返回_none() {
        let v = vec![1, 2, 3];
        assert_eq!(v.get(5), None);
        assert_eq!(v.get(0), Some(&1));
    }
}
