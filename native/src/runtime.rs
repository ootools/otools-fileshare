//! 惰性多线程 tokio runtime（AtomicPtr 持有）。
//!
//! 为什么不用 `OnceLock`：`__otools_shutdown__`（宿主升级 / 重装插件前的停机
//! 询问）需要把 runtime **取回所有权**再 `shutdown_background`——`OnceLock`
//! 只能给出共享引用，交不出所有权。热路径（已初始化）只有一次
//! `load(Acquire)` + 判空，和 `OnceLock::get` 同量级。

use std::sync::atomic::{AtomicPtr, Ordering};

static RT: AtomicPtr<tokio::runtime::Runtime> = AtomicPtr::new(std::ptr::null_mut());

fn rt() -> &'static tokio::runtime::Runtime {
    let existing = RT.load(Ordering::Acquire);
    if !existing.is_null() {
        // SAFETY: 指针只在 shutdown() 里被 swap 取回所有权，此后引用失效；
        // 调用方（宿主线程）保证 shutdown 不与业务调用并发。
        return unsafe { &*existing };
    }

    let candidate = Box::into_raw(Box::new(
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("otools-fileshare-native tokio runtime"),
    ));
    match RT.compare_exchange(
        std::ptr::null_mut(),
        candidate,
        Ordering::AcqRel,
        Ordering::Acquire,
    ) {
        Ok(_) => unsafe { &*candidate },
        Err(actual) => {
            // 并发初始化竞争失败：回收本次多建的 runtime，用赢家那份。
            // SAFETY: candidate 刚由 Box::into_raw 产生，尚未发布给其它线程。
            unsafe { drop(Box::from_raw(candidate)) };
            unsafe { &*actual }
        }
    }
}

/// 在插件 tokio runtime 的上下文里同步执行 `f`。
///
/// 进入 runtime 上下文（`Runtime::enter`）后直接跑 `f`：`f` 内可以
/// `tokio::spawn`（任务落在 runtime 的 worker 线程上）、拿
/// `Handle::current()` 再 `block_on` 阻塞等异步结果——即 `Handle::block_on`
/// 的语义（调用线程阻塞，异步任务在 runtime 上推进），且不会出现
/// 「runtime 线程内再 block_on 同一 runtime」的重入 panic。
pub fn with_rt<T>(f: impl FnOnce() -> T) -> T {
    let _guard = rt().enter();
    f()
}

/// 关掉 dispatch 用的 tokio runtime，让 worker 线程自行退出。
///
/// 只在 `__otools_shutdown__`（宿主确认要升级 / 重装插件）时调用：
/// 后台关闭不阻塞宿主调用线程；此后再次 `with_rt` 会按需重建。
pub fn shutdown() {
    let ptr = RT.swap(std::ptr::null_mut(), Ordering::AcqRel);
    if !ptr.is_null() {
        // SAFETY: swap 之后本函数独占该 Box，取出 runtime 交给 shutdown_background 消费。
        let runtime: tokio::runtime::Runtime = unsafe { *Box::from_raw(ptr) };
        runtime.shutdown_background();
    }
}
