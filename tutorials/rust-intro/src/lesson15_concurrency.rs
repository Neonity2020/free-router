//! 第 15 课：并发 —— 线程、channel、Mutex，以及 async 基础
//!
//! 运行：cargo run -- 15
//!
//! 编译器帮你拦下的数据竞争：任何被多线程共享的可变数据，
//! 都必须用 Mutex/RwLock/原子类型包起来，否则根本编译不过。

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Duration;

/// 共享可变数据：Arc（共享所有权）+ Mutex（互斥访问）
pub type SharedStats = Arc<Mutex<HashMap<String, u32>>>;

/// ❌ 这个函数编译不过：闭包捕获 &mut，但要求 'static
// pub fn bad<F: FnMut() + Send + 'static>(f: F) {
//     thread::spawn(f);
// }
/// ✅ 解决办法：move 闭包 + Arc/Mutex
pub fn spawn_worker(id: u32, stats: SharedStats) -> thread::JoinHandle<u32> {
    thread::spawn(move || {
        let mut count = 0u32;
        for i in 0..3 {
            count += i;
            let mut map = stats.lock().unwrap(); // MutexGuard，作用域结束自动解锁
            *map.entry(format!("w{id}")).or_insert(0) += 1;
            drop(map); // 也可以不写，作用域结束就解锁
            thread::sleep(Duration::from_millis(1));
        }
        count
    })
}

/// 通道：多生产者单消费者
pub fn producer_consumer(n: u32) -> (u32, u32, u32) {
    let (tx, rx) = mpsc::channel();
    for i in 0..n {
        let tx = tx.clone(); // 每个生产者克隆一份发送端
        thread::spawn(move || {
            tx.send(i * 10).unwrap();
        });
    }
    drop(tx); // 关键！最后一个发送端必须丢弃，否则接收端永远等不到关闭

    let mut a = 0;
    let mut b = 0;
    let mut c = 0;
    for v in rx.iter() {
        match v % 3 {
            0 => a += v,
            1 => b += v,
            _ => c += v,
        }
    }
    (a, b, c)
}

/// 原子类型：单变量的轻量同步
pub fn atomic_counter(n: u32) -> u64 {
    let c = Arc::new(AtomicU64::new(0));
    let mut handles = vec![];
    for _ in 0..4 {
        let c = Arc::clone(&c);
        handles.push(thread::spawn(move || {
            for _ in 0..n {
                c.fetch_add(1, Ordering::Relaxed);
            }
        }));
    }
    for h in handles {
        h.join().unwrap();
    }
    c.load(Ordering::Relaxed)
}

/// 并行分片求和（最简单的并行计算）
pub fn parallel_sum(values: &[u64]) -> u64 {
    if values.len() < 2 {
        return values.iter().sum();
    }
    let mid = values.len() / 2;
    let (l, r) = values.split_at(mid);
    let (s1, s2) = thread::scope(|s| {
        let h = s.spawn(|| parallel_sum(l));
        let right = parallel_sum(r);
        (h.join().unwrap(), right)
    });
    s1 + s2
}

/// 演示：故意制造死锁（会 hang，别在真代码里这么写）
#[allow(dead_code)]
pub fn deadlock_demo_note() {
    let m = Arc::new(Mutex::new(0u32));
    let m2 = Arc::clone(&m);
    let _ = std::thread::spawn(move || {
        let _a = m2.lock().unwrap();
        // 下一个线程在这里 lock 同一把锁 → 双方互等，死锁
    });
    let _b = m.lock().unwrap();
    println!("线程一直阻塞，程序 hang 住 —— 这就是死锁");
}

// ============================================================================
// async 部分：标准库自带 Future，我们手写一个最小执行器，不引入任何依赖。
// ============================================================================
use std::future::Future;
use std::task::{Context, Poll, Wake, Waker};

/// 最小执行器：反复 poll 未来的 future，Pending 就 park 当前线程等待唤醒
pub fn block_on<F: Future>(fut: F) -> F::Output {
    let mut fut = Box::pin(fut);
    let waker = Waker::from(Arc::new(ThreadWaker(thread::current())));
    let mut cx = Context::from_waker(&waker);
    loop {
        match fut.as_mut().poll(&mut cx) {
            Poll::Ready(v) => return v,
            Poll::Pending => thread::park(),
        }
    }
}

struct ThreadWaker(thread::Thread);
impl Wake for ThreadWaker {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
    fn wake_by_ref(self: &Arc<Self>) {
        self.0.unpark();
    }
}

/// 手写一个「一次性让出」的 future 工厂
pub async fn double_later(n: u32) -> u32 {
    poll_fn(|cx| {
        cx.waker().wake_by_ref(); // 立刻通知调度器「我还能继续」
        Poll::Ready(n * 2)
    })
    .await
}

fn poll_fn<T, F: FnMut(&mut Context<'_>) -> Poll<T>>(f: F) -> impl Future<Output = T> {
    std::future::poll_fn(f)
}

/// 手写 join：并发等待两个 future（真实项目里用 futures 宏，这里提供最小可用版）
pub async fn join2<A: Future, B: Future>(a: A, b: B) -> (A::Output, B::Output) {
    let mut a = Box::pin(a);
    let mut b = Box::pin(b);
    let mut av: Option<A::Output> = None;
    let mut bv: Option<B::Output> = None;
    poll_fn(move |cx| {
        if av.is_none() {
            if let Poll::Ready(v) = a.as_mut().poll(cx) {
                av = Some(v);
            }
        }
        if bv.is_none() {
            if let Poll::Ready(v) = b.as_mut().poll(cx) {
                bv = Some(v);
            }
        }
        if av.is_some() && bv.is_some() {
            Poll::Ready((av.take().unwrap(), bv.take().unwrap()))
        } else {
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    })
    .await
}

/// async fn 会被编译成状态机
async fn fetch_two(a: u32, b: u32) -> u32 {
    let x = double_later(a).await;
    let y = double_later(b).await;
    x + y
}

async fn fetch_concurrent(a: u32, b: u32) -> u32 {
    let (x, y) = join2(double_later(a), double_later(b)).await;
    x + y
}

pub fn run() {
    println!("== 第 15 课：并发 ==\n");

    // ---- 多线程共享状态 ----------------------------------------------------
    let stats: SharedStats = Arc::new(Mutex::new(HashMap::new()));
    let handles: Vec<_> = (0..3).map(|i| spawn_worker(i, Arc::clone(&stats))).collect();
    let results: Vec<u32> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    println!("各线程结果 {results:?}，统计 {:?}", *stats.lock().unwrap());
    println!();

    // ---- 通道 -------------------------------------------------------------
    let t = thread::spawn(|| producer_consumer(9));
    println!("通道消费按 %3 分组：{:?}", t.join().unwrap());
    println!();

    // ---- 原子变量 ----------------------------------------------------------
    println!("4 个线程各加 {} 次 = {}", 1000, atomic_counter(1000));
    println!();

    // ---- scoped 线程：安全借用栈上数据，不用 Arc ---------------------------
    let data = vec![1u64, 2, 3, 4, 5, 6];
    println!("并行分片求和 = {}", parallel_sum(&data));
    println!();

    // ---- async -------------------------------------------------------------
    let r1 = block_on(fetch_two(2, 3));
    let r2 = block_on(fetch_concurrent(2, 3));
    println!("串行 await: {r1}，并发 join: {r2}");
    println!();

    println!("要点：");
    println!("  - 要共享可变状态 → Arc<Mutex<T>>（写多读少时可换 RwLock）");
    println!("  - 单线程共享可变 → Rc<RefCell<T>>（第 13 课）");
    println!("  - 通道负责线程间传数据，Mutex 负责共享数据，二选一通常更清晰");
    println!("  - async 解决的是「大量连接、少量线程」的网络 IO 场景");
    println!("  - 死锁来自 lock 顺序不一致：全局约定一个加锁顺序即可避免");

    println!("\n动手练习：");
    println!("  1. 用 channel 把 4 个线程的结果汇总到一个线程求和。");
    println!("  2. 用 RwLock 替换示例 1 里的 Mutex，比较读多场景。");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap as Map;

    #[test]
    fn 多线程汇总() {
        let stats: SharedStats = Arc::new(Mutex::new(Map::new()));
        let hs: Vec<_> = (0..4).map(|i| spawn_worker(i, Arc::clone(&stats))).collect();
        for h in hs {
            h.join().unwrap();
        }
        let m = stats.lock().unwrap();
        assert_eq!(m.len(), 4);
        assert_eq!(m["w0"], 3);
    }

    #[test]
    fn 通道分组求和() {
        let (a, b, c) = producer_consumer(9);
        // 0,10,20,...,80 按 %3 分组
        assert_eq!(a, 90); // 0,30,60
        assert_eq!(b, 120); // 10,40,70
        assert_eq!(c, 150); // 20,50,80
    }

    #[test]
    fn 原子计数正确() {
        assert_eq!(atomic_counter(500), 2000);
    }

    #[test]
    fn 并行分片求和() {
        assert_eq!(parallel_sum(&[1, 2, 3, 4, 5, 6]), 21);
        assert_eq!(parallel_sum(&[7]), 7);
    }

    #[test]
    fn 最小执行器能跑_async() {
        assert_eq!(block_on(fetch_two(2, 3)), 10);
        assert_eq!(block_on(fetch_concurrent(2, 3)), 10);
    }
}
