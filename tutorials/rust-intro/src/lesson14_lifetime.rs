//! 第 14 课：生命周期（lifetime）—— 只是「借用能活多久」的名字
//!
//! 运行：cargo run -- 14

use std::fmt;

/// 1) 为什么需要它：多个参数都可能作为返回值来源时
pub fn longest_with_explicit<'a>(a: &'a str, b: &'a str) -> &'a str {
    if a.len() >= b.len() { a } else { b }
}

/// 生命周期省略（elision）规则：能推断时编译器自动补，你不用写：
/// - 每个引用参数各得一个独立生命周期
/// - 有且仅有一个输入生命周期时（或者方法里的 &self），它赋给所有输出生命周期
///
/// 对比上面的 longest_with_explicit：那里有两个输入生命周期，
/// 编译器无法判断返回值来自 a 还是 b，所以必须手写 'a。
pub fn first_line(text: &str) -> &str {
    text.lines().next().unwrap_or("")
}

/// 2) 返回值永远是某个入参的子切片（不是自己造的）—— 这样才安全
pub struct Parser<'a> {
    text: &'a str,
    pos: usize,
}

impl<'a> Parser<'a> {
    pub fn new(text: &'a str) -> Self {
        Self { text, pos: 0 }
    }

    /// 返回的 &str 借用了 self.text，因此必须带上 'a
    pub fn rest(&self) -> &'a str {
        &self.text[self.pos..]
    }

    pub fn next_token(&mut self) -> Option<&'a str> {
        let rest = self.rest().trim_start();
        // text.len() - rest.len() 就是 rest 在 text 里的起点（rest 是 text 的后缀）
        self.pos = self.text.len() - rest.len();
        if rest.is_empty() {
            return None;
        }

        match rest.find(char::is_whitespace) {
            Some(i) => {
                self.pos += i; // 停在分隔空白处，下一次 trim_start 会跳过它
                Some(&rest[..i])
            }
            None => {
                self.pos = self.text.len(); // 最后一个 token
                Some(rest)
            }
        }
    }
}

/// 3) 结构体持有引用：必须标注生命周期
pub struct Excerpt<'a> {
    part: &'a str,
}

impl<'a> Excerpt<'a> {
    pub fn first_sentence(text: &'a str) -> Option<Self> {
        text.find('。').map(|i| Excerpt {
            // 注意：'。' 在 UTF-8 里占 3 字节，i 是它的起始字节位置，
            // 所以必须加上 len_utf8()，否则切片会落在字符中间而 panic。
            part: &text[..i + '。'.len_utf8()],
        })
    }

    pub fn level(&self) -> usize {
        self.part.lines().count()
    }
}

impl fmt::Display for Excerpt<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.part)
    }
}

/// 4) 静态生命周期：'static 数据活到程序结束（字符串字面量就是 &'static str）
pub fn leaked() -> &'static str {
    Box::leak(String::from("故意泄漏到堆上").into_boxed_str())
}

/// 5) 常见误解：
/// - 生命周期不改变任何值的实际存活时间，只是标注关系
/// - 多个不同生命周期时不能直接合并（除非你确实要最短的那个）
pub fn two_lifetimes<'short, 'long>(a: &'short str, b: &'long str) -> (&'short str, &'long str) {
    (a, b)
}

pub fn run() {
    println!("== 第 14 课：生命周期 ==\n");

    let a = String::from("很长的一句话");
    let b = String::from("短");
    println!("显式：{}", longest_with_explicit(&a, &b));
    println!("省略：{}", first_line(&a));
    println!();

    // 结构体持有借用
    let mut p = Parser::new("  hello rust world  ");
    println!("rest = {:?}", p.rest());
    while let Some(tok) = p.next_token() {
        println!("token: {tok}");
    }
    println!();

    let text = String::from("第一句。第二句。第三句。");
    match Excerpt::first_sentence(&text) {
        Some(e) => println!("摘录：{e}（行数 {}）", e.level()),
        None => println!("没有找到句号"),
    }
    println!();

    // 'static
    println!("leaked: {}", leaked());
    println!();

    // 不同生命周期可以并列存在
    let (s, l) = two_lifetimes("短", "长一点");
    println!("{s} / {l}");
    println!();

    println!("心法：99% 的情况你不需要手写生命周期，编译器能推断；");
    println!("      只有当返回值可能来自多个输入时才需要你明确写出来。");

    println!("\n动手练习：");
    println!("  1. 写 `fn trim_and_upper(s: &str) -> &str`，返回首行去掉空白后的切片。");
    println!("  2. 把 Excerpt 改成能持有 String（对比一下差别）。");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 取较长者() {
        assert_eq!(longest_with_explicit("aa", "bbb"), "bbb");
    }

    #[test]
    fn 省略生命周期() {
        assert_eq!(first_line("aaaa\nb"), "aaaa");
        assert_eq!(first_line("只有一行"), "只有一行");
    }

    #[test]
    fn 解析器分词() {
        let mut p = Parser::new("  a b c  ");
        let toks: Vec<&str> = std::iter::from_fn(|| p.next_token()).collect();
        assert_eq!(toks, vec!["a", "b", "c"]);
    }

    #[test]
    fn 摘录第一句() {
        let text = String::from("第一句。第二句。");
        let e = Excerpt::first_sentence(&text).unwrap();
        assert_eq!(e.part, "第一句。");
        assert!(Excerpt::first_sentence("没有句号").is_none());
    }

    #[test]
    fn 两个生命周期() {
        let a = String::from("x");
        let b = String::from("yy");
        {
            let (s, l) = two_lifetimes(&a, &b);
            assert_eq!((s, l), ("x", "yy"));
        }
        println!("a 依旧可用：{a}"); // 证明没有把 a 借走
    }
}
