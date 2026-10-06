use rust_intro::{dispatch, LESSONS};
use std::process::ExitCode;

fn main() -> ExitCode {
    let arg = match std::env::args().nth(1) {
        Some(a) => a,
        None => {
            print_help();
            return ExitCode::SUCCESS;
        }
    };

    if arg == "--list" || arg == "list" {
        println!("Rust 入门教程 · 课程列表\n");
        let width = LESSONS.iter().map(|(n, _)| n.len()).max().unwrap_or(2);
        for (num, desc) in LESSONS {
            println!("  {num:<width$}  {desc}", width = width);
        }
        println!("\n运行：cargo run -- 1    测试：cargo test");
        return ExitCode::SUCCESS;
    }

    match dispatch(&arg) {
        Ok(()) => ExitCode::SUCCESS,
        Err(msg) => {
            eprintln!("错误：{msg}");
            ExitCode::FAILURE
        }
    }
}

fn print_help() {
    println!("Rust 入门教程 —— 跟着敲就能学会\n");
    println!("  cargo run -- --list     查看全部课程");
    println!("  cargo run -- 1          运行第 1 课");
    println!("  cargo test              运行全部测试\n");
    println!("提示：先读 README.md，再从第 1 课开始。");
}
