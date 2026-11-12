//! 事件队列：业务侧 [`push`]，宿主经 `poll_events` 方法轮询取走。
//!
//! 队列容量 1024、满时丢最旧：前端长时间未轮询时不至于无限膨胀；
//! 状态类事件（服务器启停、列表变更）丢一条旧的换新的是可接受的。
//! 高频数据不走这里（后续任务的文件传输进度事件要自行节流）。

use serde_json::{json, Value};
use std::collections::VecDeque;
use std::sync::Mutex;

const EVENT_QUEUE_CAPACITY: usize = 1024;

static EVENT_QUEUE: Mutex<VecDeque<Value>> = Mutex::new(VecDeque::new());

/// 推送一条事件（`{"topic": topic, "payload": payload}`）。
///
/// topic 约定前缀 `fileshare:`（如 `fileshare:server.statusChange`）。
/// 锁中毒时静默丢弃——事件通知失败不应影响业务主流程。
pub fn push(topic: &str, payload: Value) {
    let Ok(mut queue) = EVENT_QUEUE.lock() else {
        return;
    };
    if queue.len() >= EVENT_QUEUE_CAPACITY {
        queue.pop_front();
    }
    queue.push_back(json!({ "topic": topic, "payload": payload }));
}

/// 取出并清空已排队的事件，供宿主 `poll_events` 转发给前端。
pub fn drain() -> Vec<Value> {
    match EVENT_QUEUE.lock() {
        Ok(mut queue) => queue.drain(..).collect(),
        Err(_) => Vec::new(),
    }
}
