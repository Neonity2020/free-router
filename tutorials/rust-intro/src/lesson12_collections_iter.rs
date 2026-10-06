//! 第 12 课：Vec / HashMap、闭包、迭代器
//!
//! 运行：cargo run -- 12

use std::collections::{BTreeMap, HashMap, HashSet};

pub fn run() {
    println!("== 第 12 课：集合 / 闭包 / 迭代器 ==\n");

    // ---- Vec 作为栈 --------------------------------------------------------
    let mut stack = vec![1, 2, 3];
    stack.push(4);
    println!("栈 pop 顺序：{:?}", stack.pop());
    println!();

    // ---- HashMap 的三种插入方式 -------------------------------------------
    let mut m: HashMap<&str, u32> = HashMap::new();
    m.insert("a", 1);
    m.insert("a", 2); // 覆盖
    println!("m = {m:?}");

    // entry API：不存在就插入，存在就修改（最常用）
    *m.entry("b").or_insert(0) += 10;
    *m.entry("b").or_insert(0) += 5;
    println!("m = {m:?}");

    // and_modify + or_insert 组合
    match m.entry("c") {
        std::collections::hash_map::Entry::Occupied(mut e) => {
            *e.get_mut() += 1;
        }
        std::collections::hash_map::Entry::Vacant(e) => {
            e.insert(100);
        }
    }
    println!("m = {m:?}");
    println!();

    // ---- BTreeMap / HashSet：需要有序、去重时用 ----------------------------
    let bt: BTreeMap<&str, u32> = [("b", 2), ("a", 1)].into_iter().collect();
    println!("BTreeMap 天然有序：{bt:?}");
    let hs: HashSet<i32> = vec![1, 2, 2, 3].into_iter().collect();
    println!("HashSet 去重：{hs:?}");
    println!();

    // ---- 排序与去重要显式调用（因为它们有 O(n log n) 成本）-----------------
    let mut v = vec![3, 1, 2, 1];
    v.sort_unstable(); // 更快，不稳定排序
    v.dedup();
    println!("sort_unstable + dedup = {v:?}");
    println!();

    // ---- 闭包 ---------------------------------------------------------------
    // 三种形式：Fn（只读）、FnMut（改捕获状态）、FnOnce（消耗捕获状态）
    let mut total = 0;
    let mut add = |x: i32| total += x; // FnMut
    add(1);
    add(2);
    println!("闭包捕获的 total = {total}");

    let owned = String::from("take me");
    let consume = move || owned; // move 闭包捕获所有权 → FnOnce
    println!("move 闭包：{}", consume());
    println!();

    // ---- 迭代器：惰性求值，链式组合 ------------------------------------------
    let nums: Vec<i32> = (1..=20).collect();
    let evens_squares: Vec<i32> = nums.iter().filter(|x| **x % 2 == 0).map(|x| x * x).collect();
    println!("偶数的平方：{evens_squares:?}");
    println!("sum = {}", nums.iter().sum::<i32>());
    println!("position(7) = {:?}", nums.iter().position(|x| *x == 7));
    println!("any(>19) = {}", nums.iter().any(|x| *x > 19));
    println!("all(>0) = {}", nums.iter().all(|x| *x > 0));
    println!();

    // ---- 消费型 vs 借用型 vs 可变借用型迭代器 -------------------------------
    let v = vec![1, 2, 3];
    let a: Vec<&i32> = v.iter().collect(); // 借用
    let b: Vec<i32> = v.iter().cloned().collect(); // 借用后复制出值
    let c: Vec<i32> = v.clone().into_iter().collect(); // 消费原 Vec
    let mut d = vec![1, 2, 3];
    for x in d.iter_mut() {
        *x *= 10; // 可变借用
    }
    println!("iter={a:?} cloned={b:?} into_iter={c:?} iter_mut 后 d={d:?}");
    println!();

    // ---- 常用组合子速查 ---------------------------------------------------
    println!("rev + take(3) = {:?}", nums.iter().rev().take(3).collect::<Vec<_>>());
    println!("fold 求和 = {}", nums.iter().fold(0, |acc, x| acc + x));
    println!("flat_map 拆行 = {:?}", vec!["a b", "c"].iter().flat_map(|s| s.split(' ')).collect::<Vec<_>>());
    println!("partition = {:?}", {
        let (even, odd): (Vec<i32>, Vec<i32>) = nums.iter().cloned().partition(|x| x % 2 == 0);
        (even.len(), odd.len())
    });
    println!("zip + map = {:?}", vec![1, 2].into_iter().zip("ab".chars()).map(|(i, c)| format!("{i}{c}")).collect::<Vec<_>>());
    println!("windows = {:?}", nums.windows(5).next());
    println!("chunks = {:?}", nums.chunks(7).collect::<Vec<_>>());

    println!("\n动手练习：");
    println!("  1. 用 fold 统计一段文本里每个字符出现的次数。");
    println!("  2. 用 iter().max() 找出最大分数（注意比较的是 &i32）。");
}

/// 例子：把闭包作为参数传入的泛型函数
pub fn apply_twice<F>(mut f: F, x: i32) -> i32
where
    F: FnMut(i32) -> i32,
{
    // f 是 FnMut：不能在同一表达式里 f(f(x))（会同时借两次可变），
    // 先存下第一次结果再调用第二次。
    let once = f(x);
    f(once)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entry_模式() {
        let mut m = HashMap::new();
        for w in "a b a c b a".split(' ') {
            *m.entry(w).or_insert(0) += 1;
        }
        assert_eq!(m["a"], 3);
        assert_eq!(m["b"], 2);
        assert_eq!(m["c"], 1);
    }

    #[test]
    fn 迭代器链() {
        let nums: Vec<i32> = (1..=20).collect();
        let r: Vec<i32> = nums.iter().filter(|x| **x % 2 == 0).map(|x| x * x).collect();
        assert_eq!(r, vec![4, 16, 36, 64, 100, 144, 196, 256, 324, 400]);
    }

    #[test]
    fn 闭包传参() {
        assert_eq!(apply_twice(|x| x + 1, 0), 2);
        assert_eq!(apply_twice(|x| x * 3, 2), 18);
    }

    #[test]
    fn 分区() {
        let (even, odd): (Vec<i32>, Vec<i32>) = (1..=10).partition(|x| x % 2 == 0);
        assert_eq!(even, vec![2, 4, 6, 8, 10]);
        assert_eq!(odd.len(), 5);
    }
}
