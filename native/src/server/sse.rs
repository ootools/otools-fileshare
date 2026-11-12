//! SSE 列表事件；队列有界，停服时立即结束连接。

use std::convert::Infallible;
use std::sync::{Arc, OnceLock};

use axum::{
    extract::State,
    response::{
        sse::{Event, KeepAlive},
        Sse,
    },
};
use futures_util::Stream;
use serde_json::{json, Value};
use tokio::sync::broadcast;

use super::AppState;

static EVENTS: OnceLock<broadcast::Sender<Value>> = OnceLock::new();

fn events() -> &'static broadcast::Sender<Value> {
    EVENTS.get_or_init(|| broadcast::channel(64).0)
}

pub fn list_changed() {
    let _ = events().send(json!({"type":"fileDb.listChange"}));
}

pub async fn registry(
    State(state): State<Arc<AppState>>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let mut receiver = events().subscribe();
    let mut stopping = state.stopping.clone();
    let stream = async_stream::stream! {
        let _state = state;
        yield Ok(Event::default().data(json!({"type":"registry","data":{"id":uuid::Uuid::new_v4().to_string()}}).to_string()));
        loop {
            tokio::select! {
                _ = stopping.changed() => break,
                message = receiver.recv() => match message {
                    Ok(message) => yield Ok(Event::default().data(message.to_string())),
                    Err(broadcast::error::RecvError::Lagged(_)) => yield Ok(Event::default().data(json!({"type":"fileDb.listChange"}).to_string())),
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        }
    };
    Sse::new(stream).keep_alive(KeepAlive::default())
}
