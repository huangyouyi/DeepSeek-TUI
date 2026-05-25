use std::{convert::Infallible, time::Duration};

use async_stream::stream;
use axum::response::sse::{Event, KeepAlive, Sse};
use futures_core::Stream;
use serde_json::{Value, json};
use tokio::sync::broadcast;

use crate::{
    AppState, ServerEvent,
    routes::{SERVICE_NAME, now_ms},
};

pub fn broadcast_event(state: &AppState, event_type: impl Into<String>, payload: Value) {
    state.broadcast(ServerEvent {
        event_type: event_type.into(),
        payload,
    });
}

pub fn event_stream(state: AppState) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let mut receiver = state.subscribe();
    let initial = ServerEvent {
        event_type: "connection.updated".to_string(),
        payload: json!({
            "status": "connected",
            "service": SERVICE_NAME,
            "created_at_ms": now_ms()
        }),
    };

    let stream = stream! {
        yield Ok(to_sse_event(initial));

        loop {
            match receiver.recv().await {
                Ok(event) => yield Ok(to_sse_event(event)),
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    };

    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    )
}

fn to_sse_event(event: ServerEvent) -> Event {
    let event_name = event.event_type.clone();
    Event::default()
        .event(event_name)
        .json_data(event)
        .unwrap_or_else(|_| Event::default().event("connection.updated").data("{}"))
}
