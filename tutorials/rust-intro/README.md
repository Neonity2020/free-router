# Rust 入门教程

跟着敲就能学会的 Rust 入门教程。**全程只用标准库**，没有任何第三方依赖：
`cargo build` 不需要联网，也不会因为依赖版本变动而失效。

## 前置条件

- Rust 1.89+（`rustup show` 查看当前版本，`rustup update stable` 升级）
- 不需要 Node、Python 或数据库

## 怎么用

```sh
cargo run -- --list    # 列出所有课程
cargo run -- 1         # 运行第 1 课
cargo run -- 10        # 两位数直接写编号
cargo run -- project   # 综合实战：迷你负载均衡器
cargo test             # 跑全部测试
```

建议节奏：先读 `src/lessonNN_*.rs` 里的代码和注释，自己再敲一遍，
最后做每课末尾的「动手练习」。每一课都是可以直接运行的完整程序，
不需要你补全残缺的代码。

## 课程表

| 编号 | 主题 | 关键内容 |
| --- | --- | --- |
| 01 | hello | 第一个程序、变量、可变性、遮蔽 |
| 02 | types | 基本类型、元组、`String` vs `&str` |
| 03 | control | `if`、循环、`match` |
| 04 | functions | 函数、表达式与语句 |
| 05 | vec-slice | 数组、切片、`Vec` |
| 06 | struct | `struct`、`impl`、`derive` |
| 07 | enum-option | `enum`、`Option`、`if let` |
| 08 | result-error | `Result`、`?`、自定义错误 |
| 09 | generics | 泛型、`trait`、`dyn Trait` |
| 10 | ownership | 所有权、借用（最关键的一课） |
| 11 | modules | 模块、可见性、项目组织 |
| 12 | collections | `HashMap`、闭包、迭代器 |
| 13 | pointer | `Box`、`Rc`、`RefCell` |
| 14 | lifetime | 生命周期 |
| 15 | concurrency | 线程、`channel`、`Mutex`、async 基础 |
| 16 | tests | 测试怎么写 |
| project | 实战 | 综合实战：迷你负载均衡器 |

## 目录结构

```text
rust-intro/
  Cargo.toml
  README.md
  src/
    main.rs                     命令行入口：--list / 课程编号
    lib.rs                      课程清单 LESSONS 与 dispatch
    lesson01_hello.rs
    ...
    lesson16_tests.rs
    project_load_balancer/      综合实战（多文件模块，对应第 11 课的模块树）
      mod.rs                    组装与演示
      provider.rs               上游抽象
      pool.rs                   Key 轮询池与冷却
      stats.rs                  统计
```

## 测试就是规格

每一课都自带 `#[cfg(test)] mod tests`，所以 `cargo test` 同时是这份教程的
规格说明。测试名用中文写，读起来就是一句需求：

```sh
cargo test                       # 全部
cargo test lesson16              # 只跑第 16 课
cargo test 冷却                  # 按名字过滤
cargo test -- --nocapture        # 让 println! 的输出显示出来
cargo test -- --ignored          # 单独跑被 #[ignore] 跳过的测试
```

第 16 课的文档注释里有一个 **doctest**，`cargo test` 会真的编译并运行它 ——
文档里的例子因此不会过期。

## 关于代码风格

- 为了让概念看得清楚，示例里刻意保留了「教学写法」，没有全部按 clippy 的
  风格建议改写（例如第 10 课故意用 `&String` 演示借用规则，第 12 课故意
  展示 `vec![...]` 的写法）。
- 因此这份教程**不在仓库 CI 的检查范围内**，它只保证 `cargo build` 和
  `cargo test` 通过。
