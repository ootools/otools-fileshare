//! dispatch 用的惰性多线程 tokio runtime（`RwLock` 持有）。
//!
//! 为什么不用 `OnceLock`：`__otools_shutdown__`（宿主升级 / 重装插件前的停机
//! 询问）需要停掉 worker 线程——否则宿主 FreeLibrary 后旧线程执行已消失的
//! 代码会崩（见宿主 `native_plugins/core.rs` 对不卸载退役库的注释）——停完
//! 还要支持再次 `with_rt` 重建。
//!
//! 为什么不用裸 `AtomicPtr` + `Box::from_raw`（otools-git 的做法）：宿主在
//! 注册表锁**外**执行 FFI 调用（`native_plugins/core.rs`），`__otools_shutdown__`
//! 可能与仍在途的业务 invoke 并发——裸 swap + drop 会释放其它线程正在使用的
//! runtime（UAF）。`RwLock` 让 `with_rt` 的读锁与 `shutdown` 的写锁天然互斥：
//! shutdown 会等在途调用退出后再取走所有权关停。
//!
//! 约定：dispatch 方法在 `with_rt` 内**不得长时间停留**（bind/spawn 级别的
//! 毫秒操作；长任务 `tokio::spawn` 给 runtime），否则会阻塞宿主的停机询问。

use std::sync::{RwLock, RwLockReadGuard};

static RT: RwLock<Option<Box<tokio::runtime::Runtime>>> = RwLock::new(None);

fn init_if_needed() {
    let mut guard = RT.write().expect("otools-fileshare-native runtime lock poisoned");
    if guard.is_none() {
        *guard = Some(Box::new(
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("otools-fileshare-native tokio runtime"),
        ));
    }
}

fn read_or_init() -> RwLockReadGuard<'static, Option<Box<tokio::runtime::Runtime>>> {
    // 快路径：runtime 已就绪，直接读锁返回。
    {
        let guard = RT.read().expect("otools-fileshare-native runtime lock poisoned");
        if guard.is_some() {
            return guard;
        }
    }
    // 慢路径：写锁内初始化（双检靠写锁互斥），再降到读锁供业务使用。
    init_if_needed();
    RT.read().expect("otools-fileshare-native runtime lock poisoned")
}

/// 在插件 tokio runtime 的上下文里同步执行 `f`。
///
/// 读锁活过整个 `f`（与 `shutdown` 的写锁互斥，保证 runtime 不会被并发释放）；
/// `f` 内可以 `tokio::spawn`（任务落在 runtime 的 worker 线程上）、拿
/// `Handle::current()` 再 `block_on` 阻塞等异步结果——即 `Handle::block_on`
/// 的语义（调用线程阻塞，异步任务在 runtime 上推进），且不会出现
/// 「runtime 线程内再 block_on 同一 runtime」的重入 panic。
pub fn with_rt<T>(f: impl FnOnce() -> T) -> T {
    let guard = read_or_init();
    let rt: &tokio::runtime::Runtime = guard.as_ref().expect("runtime initialized above");
    let _enter = rt.enter();
    f()
}

/// 关掉 dispatch 用的 tokio runtime，让 worker 线程自行退出。
///
/// 只在 `__otools_shutdown__`（宿主确认要升级 / 重装插件）时调用。写锁保证
/// 没有在途 `with_rt`；`shutdown_background` 在后台收 worker，不阻塞宿主线程。
/// 此后再次 `with_rt` 会按需重建新 runtime。
pub fn shutdown() {
    let mut guard = RT.write().expect("otools-fileshare-native runtime lock poisoned");
    if let Some(rt) = guard.take() {
        // Box<Runtime> → Runtime（shutdown_background 消费所有权）。
        rt.shutdown_background();
    }
}
