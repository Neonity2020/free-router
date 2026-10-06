//! 第 1 课：第一个程序、变量、可变性、遮蔽（shadowing）
//!
//! 运行：cargo run -- 1

pub fn run() {
    println!("== 第 1 课：Hello, Rust! ==\n");

    // ---- 1. 最小的程序 ----------------------------------------------------
    // println! 是宏（结尾有 !）。它可以像函数一样调用，但编译期会展开成代码。
    println!("Hello, world!");
    println!();

    // ---- 2. 变量：默认不可变 ----------------------------------------------
    // Rust 默认变量不可变，这是它最重要的设计决策之一：
    // 谁都能读这段代码，就能确定它后面不会被偷偷改掉。
    let name = "小菜"; // 不可变变量
    println!("你好，{name}");

    // name = "大盘鸡"; // 取消注释试试：编译报错 cannot assign twice to immutable variable
    println!();

    // ---- 3. 可变变量：加 mut ---------------------------------------------
    let mut count = 1;
    count = count + 1;
    count += 1; // 简写
    println!("count = {count}");
    println!();

    // ---- 4. 遮蔽（shadowing）---------------------------------------------
    // 用同名 let 重新绑定，会创建一个**新的**变量，还可以顺手换类型。
    // 这跟 mut 的区别很重要：mut 是「修改同一个变量」，遮蔽是「换一个变量」。
    let x = 5;
    let x = x + 1;
    {
        let x = x * 100;
        println!("内层作用域的 x = {x}");
    }
    println!("离开作用域后 x = {x}");

    let text = "42";
    let text = text.parse::<i32>().expect("这里应该能解析成整数");
    println!("text 现在是整数：{text}，类型变了但变量名没变");
    println!();

    // ---- 5. 常量：编译期确定，全大写命名 ---------------------------------
    const MAX_RETRIES: u32 = 3;
    println!("MAX_RETRIES = {MAX_RETRIES}");

    // ---- 6. 类型标注：编译器大多数时候能推断，必要时手动写 ---------------
    let explicit: f64 = 3.0; // 浮点数默认是 f64
    let inferred = 3.0; // 和上面一样是 f64
    println!("{explicit} / {inferred}");

    println!("\n动手练习：");
    println!("  1. 取消第 2 节被注释掉的那行，读一遍编译错误信息。");
    println!("  2. 新增一个变量 `let mut city = \"北京\";`，修改它并打印。");
}

/// 练习用的小函数：把分转换成秒
pub fn seconds(minutes: u32) -> u32 {
    minutes * 60
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 分转秒() {
        assert_eq!(seconds(3), 180);
        assert_eq!(seconds(0), 0);
    }
}
