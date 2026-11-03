//! Change notifications only. Clients fetch an authorized snapshot after each event.
use crate::*;
use axum::{
    http::HeaderMap,
    response::sse::{Event, KeepAlive, Sse},
};
use std::{convert::Infallible, time::Duration};
impl App {
    pub fn notify_change(&self) {
        self.changes
            .send_modify(|revision| *revision = revision.wrapping_add(1));
    }
}
pub async fn subscribe(State(app): State<App>, headers: HeaderMap) -> Response {
    let Some(token) = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
    else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    if app.session(token).await.is_err() {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let token = token.to_string();
    let rx = app.changes.subscribe();
    let stream = futures_util::stream::unfold(
        (app, token, rx, true),
        |(app, token, mut rx, first)| async move {
            if !first && rx.changed().await.is_err() {
                return None;
            }
            // Recheck revoked/expired sessions before emitting. No private data is streamed.
            if app.session(&token).await.is_err() {
                return None;
            }
            rx.borrow_and_update();
            Some((
                Ok::<_, Infallible>(Event::default().event("change").data("refresh")),
                (app, token, rx, false),
            ))
        },
    );
    Sse::new(stream)
        .keep_alive(KeepAlive::new().interval(Duration::from_secs(60)))
        .into_response()
}
