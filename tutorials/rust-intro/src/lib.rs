//! Rust 入门教程 —— 每一课都是本文件里的一个模块。
//!
//! 运行某一课：
//!     cargo run -- 3
//!     cargo run -- 03
//! 列出所有课程：
//!     cargo run -- --list
//! 跑全部单元测试：
//!     cargo test

pub mod lesson01_hello;
pub mod lesson02_types;
pub mod lesson03_control_flow;
pub mod lesson04_functions;
pub mod lesson05_vec_slice;
pub mod lesson06_struct;
pub mod lesson07_enum_option;
pub mod lesson08_result_error;
pub mod lesson09_generics_trait;
pub mod lesson10_ownership;
pub mod lesson11_modules;
pub mod lesson12_collections_iter;
pub mod lesson13_pointer;
pub mod lesson14_lifetime;
pub mod lesson15_concurrency;
pub mod lesson16_tests;
pub mod project_load_balancer;

/// 所有课程的清单：(编号, 名字, 模块里的 run 函数)
pub const LESSONS: &[(&str, &str)] = &[
    ("01", "hello        第一个程序 / 变量 / 可变性"),
    ("02", "types        基本类型 / 元组 / String vs &str"),
    ("03", "control      if / 循环 / match"),
    ("04", "functions    函数 / 表达式与语句"),
    ("05", "vec-slice    数组 / 切片 / Vec"),
    ("06", "struct       struct / impl / derive"),
    ("07", "enum-option  enum / Option / if let"),
    ("08", "result-error Result / ? / 自定义错误"),
    ("09", "generics     泛型 / trait / dyn Trait"),
    ("10", "ownership    所有权 / 借用（最关键的一课）"),
    ("11", "modules      模块 / 可见性 / 项目组织"),
    ("12", "collections  HashMap / 闭包 / 迭代器"),
    ("13", "pointer      Box / Rc / RefCell"),
    ("14", "lifetime     生命周期"),
    ("15", "concurrency  线程 / channel / Mutex"),
    ("16", "tests        测试怎么写"),
    ("project", "project     综合实战：迷你负载均衡器"),
];

/// 按编号分发到对应课程。`main.rs` 也会调用它，所以是 pub 的。
pub fn dispatch(arg: &str) -> Result<(), String> {
    match arg {
        "01" | "1" => lesson01_hello::run(),
        "02" | "2" => lesson02_types::run(),
        "03" | "3" => lesson03_control_flow::run(),
        "04" | "4" => lesson04_functions::run(),
        "05" | "5" => lesson05_vec_slice::run(),
        "06" | "6" => lesson06_struct::run(),
        "07" | "7" => lesson07_enum_option::run(),
        "08" | "8" => lesson08_result_error::run(),
        "09" | "9" => lesson09_generics_trait::run(),
        "10" => lesson10_ownership::run(),
        "11" => lesson11_modules::run(),
        "12" => lesson12_collections_iter::run(),
        "13" => lesson13_pointer::run(),
        "14" => lesson14_lifetime::run(),
        "15" => lesson15_concurrency::run(),
        "16" => lesson16_tests::run(),
        "project" | "p" => project_load_balancer::run(),
        other => return Err(format!("没有编号为 {other} 的课，先跑 `cargo run -- --list`")),
    }
    Ok(())
}
