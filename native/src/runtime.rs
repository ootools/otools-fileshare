//! 惰性 runtime。调用持读锁，停机持写锁；不会并发释放正在使用的 runtime。

use std::sync::RwLock;

struct RuntimeSlot {
    runtime: RwLock<Option<tokio::runtime::Runtime>>,
}

impl RuntimeSlot {
    const fn new() -> Self {
        Self {
            runtime: RwLock::new(None),
        }
    }

    fn with_rt<T>(&self, f: impl FnOnce() -> T) -> T {
        loop {
            let guard = self
                .runtime
                .read()
                .unwrap_or_else(|error| error.into_inner());
            if let Some(runtime) = guard.as_ref() {
                let _enter = runtime.enter();
                return f();
            }
            drop(guard);
            let mut guard = self
                .runtime
                .write()
                .unwrap_or_else(|error| error.into_inner());
            if guard.is_none() {
                *guard = Some(
                    tokio::runtime::Builder::new_multi_thread()
                        .worker_threads(2)
                        .enable_all()
                        .build()
                        .expect("fileshare runtime"),
                );
            }
            // 初始化到重获读锁之间可能有 shutdown，必须重新检查。
        }
    }

    fn shutdown(&self) {
        let mut guard = self
            .runtime
            .write()
            .unwrap_or_else(|error| error.into_inner());
        // 服务层先取消流与后台任务，再等待 worker 真正退出。
        if let Some(runtime) = guard.take() {
            drop(runtime);
        }
    }
}

static RT: RuntimeSlot = RuntimeSlot::new();

pub fn with_rt<T>(f: impl FnOnce() -> T) -> T {
    RT.with_rt(f)
}

pub fn shutdown() {
    RT.shutdown();
}

#[cfg(test)]
mod tests {
    use std::sync::{mpsc, Arc};
    use std::time::Duration;

    #[test]
    fn shutdown_waits_for_in_flight_context_and_runtime_rebuilds() {
        let slot = Arc::new(super::RuntimeSlot::new());
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let worker_slot = slot.clone();
        let worker = std::thread::spawn(move || {
            worker_slot.with_rt(|| {
                entered_tx.send(()).unwrap();
                release_rx.recv().unwrap();
                assert!(tokio::runtime::Handle::try_current().is_ok());
            })
        });
        entered_rx.recv().unwrap();
        let shutdown_slot = slot.clone();
        let (stopped_tx, stopped_rx) = mpsc::channel();
        let stopper = std::thread::spawn(move || {
            shutdown_slot.shutdown();
            stopped_tx.send(()).unwrap();
        });
        assert!(stopped_rx.recv_timeout(Duration::from_millis(30)).is_err());
        release_tx.send(()).unwrap();
        worker.join().unwrap();
        stopped_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        stopper.join().unwrap();
        slot.with_rt(|| assert!(tokio::runtime::Handle::try_current().is_ok()));
        slot.shutdown();
    }
}
