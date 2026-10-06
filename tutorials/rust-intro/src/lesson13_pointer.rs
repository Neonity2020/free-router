//! 第 13 课：智能指针 Box / Rc / RefCell
//!
//! 运行：cargo run -- 13
//!
//! - Box<T>     ：堆上分配，独占所有权（递归类型必须用）
//! - Rc<T>      ：多个所有者，只读，运行时计数
//! - RefCell<T>  ：单线程内部可变性，运行时借用检查
//! - Arc + Mutex ：Arc = 跨线程引用计数，Mutex = 跨线程互斥（第 15 课）

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// 递归结构必须通过 Box 打断「无限大小」问题
#[derive(Debug)]
pub enum Tree {
    Leaf,
    Node {
        value: i32,
        left: Box<Tree>,
        right: Box<Tree>,
    },
}

impl Tree {
    pub fn leaf() -> Box<Self> {
        Box::new(Tree::Leaf)
    }

    pub fn node(v: i32, l: Box<Tree>, r: Box<Tree>) -> Box<Self> {
        Box::new(Tree::Node { value: v, left: l, right: r })
    }

    /// 递归遍历，和二叉树搜索题写法完全一样
    pub fn sum(&self) -> i32 {
        match self {
            Tree::Leaf => 0,
            Tree::Node { value, left, right } => value + left.sum() + right.sum(),
        }
    }
}

/// Rc：共享只读数据（引用计数，非原子，单线程）
#[derive(Debug)]
pub struct SharedStats {
    pub hits: u32,
}

/// RefCell：把借用检查从编译期挪到运行时，从而允许「运行时才决定」的可变借用
#[derive(Debug, Default)]
pub struct Counter {
    map: RefCell<HashMap<String, u32>>,
}

impl Counter {
    pub fn incr(&self, key: &str) {
        // borrow_mut() 运行时检查：同一时刻不允许再有一个可变借用
        let mut m = self.map.borrow_mut();
        *m.entry(key.to_string()).or_insert(0) += 1;
    }

    pub fn get(&self, key: &str) -> u32 {
        *self.map.borrow().get(key).unwrap_or(&0)
    }

    pub fn snapshot(&self) -> HashMap<String, u32> {
        self.map.borrow().clone()
    }
}

/// Rc<RefCell<T>>：共享 + 可变，经典组合
#[derive(Debug, Default)]
pub struct SharedCounter {
    n: Rc<RefCell<u32>>,
}

impl SharedCounter {
    pub fn new() -> Self {
        Self { n: Rc::new(RefCell::new(0)) }
    }

    /// 克隆 Rc 得到另一个所有者（O(1)，不复制数据）
    pub fn handle(&self) -> Rc<RefCell<u32>> {
        Rc::clone(&self.n)
    }
}

/// 为什么不是 Rc：Rc 不是 Send，不能跨线程。跨线程要用 Arc。
#[allow(dead_code)]
fn rc_is_not_send() {
    let _ = Rc::new(1);
    // std::thread::spawn(move || println!("{_}")); // error: Rc<...> cannot be sent between threads safely
}

pub fn run() {
    println!("== 第 13 课：智能指针 ==\n");

    // ---- Box：递归类型 + 装箱转移，避免大对象在栈上移动 ------------------
    let tree = Tree::node(
        1,
        Tree::node(2, Tree::leaf(), Tree::leaf()),
        Tree::node(3, Tree::leaf(), Tree::leaf()),
    );
    println!("递归树 sum = {}", tree.sum());
    // Tree 只派生了 Debug，所以用 {:?}；*tree 就是「解引用 Box 拿到 Tree」
    println!("Box 解引用：{:?}", *tree);
    println!();

    // ---- Rc：共享只读 ------------------------------------------------------
    let stats = Rc::new(SharedStats { hits: 0 });
    let a = Rc::clone(&stats);
    let b = Rc::clone(&stats);
    println!("计数：原 {}，a 的视图 {}，b 的视图 {}", Rc::strong_count(&stats), Rc::strong_count(&a), Rc::strong_count(&b));
    println!("共享数据：{} {}", a.hits, b.hits);
    println!();

    // ---- RefCell：内部可变性 ----------------------------------------------
    let c = Counter::default();
    c.incr("a");
    c.incr("a");
    c.incr("b");
    println!("counter: a={} b={}", c.get("a"), c.get("b"));
    println!("快照 {:?}", c.snapshot());
    println!();

    // Rc<RefCell<T>>：多个所有者都能改
    let sc = SharedCounter::new();
    let h1 = sc.handle();
    let h2 = sc.handle();
    *h1.borrow_mut() += 1;
    *h2.borrow_mut() += 1;
    println!("Rc<RefCell> 共享计数 = {}", h2.borrow());
    println!();

    // ---- RefCell 的代价：运行时 panic --------------------------------------
    let cell = RefCell::new(vec![1, 2]);
    let borrowed = cell.borrow();
    println!("已借出：{borrowed:?}");
    // let _bad = cell.borrow_mut(); // 取消注释：panic: already mutably borrowed
    drop(borrowed); // 必须先结束借用
    cell.borrow_mut().push(3);
    println!("借用结束后修改：{:?}", cell.borrow());
    println!();

    // ---- Deref：Box/Rc/& 都能自动解引用 -----------------------------------
    let b: Box<i32> = Box::new(5);
    let v: i32 = *b; // 手动解引用；i32 是 Copy，所以 b 之后还能用
    println!("*b = {v}");

    // 对非 Copy 类型，*box 会把值「移出来」，Box 随之被消费
    let owned: Box<String> = Box::new(String::from("boxed"));
    let taken: String = *owned;
    // println!("{owned}"); // 取消注释：error[E0382]: borrow of moved value: `owned`
    println!("从 Box 里移出来：{taken}");

    println!("\n动手练习：");
    println!("  1. 用 Tree 实现 depth() 和 contains(x)。");
    println!("  2. 把 Counter 改成支持 `fn incr(&self, key: &str, by: u32)`。");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap as Map;

    #[test]
    fn 递归树求和() {
        let t = Tree::node(2, Tree::node(1, Tree::leaf(), Tree::leaf()), Tree::leaf());
        assert_eq!(t.sum(), 3);
        assert_eq!(Tree::leaf().sum(), 0);
    }

    #[test]
    fn 计数器() {
        let c = Counter::default();
        c.incr("x");
        c.incr("x");
        assert_eq!(c.get("x"), 2);
        let s: Map<String, u32> = c.snapshot();
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn 共享可变() {
        let sc = SharedCounter::new();
        let h = sc.handle();
        *h.borrow_mut() += 5;
        assert_eq!(*h.borrow(), 5);
    }
}
