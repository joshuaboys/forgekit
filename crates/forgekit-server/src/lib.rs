//! JSON HTTP API.

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use forgekit_core::Host;
use serde::Serialize;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub host: Arc<Host>,
    pub mode: String,
    pub backend: String,
}

#[derive(Debug, Serialize)]
pub struct HealthBody {
    pub ok: bool,
}

#[derive(Debug, Serialize)]
pub struct StatusBody {
    pub mode: String,
    pub backend: String,
    pub repo_count: usize,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/v1/status", get(status))
        .with_state(state)
}

async fn healthz() -> Json<HealthBody> {
    Json(HealthBody { ok: true })
}

async fn status(State(state): State<AppState>) -> impl IntoResponse {
    match state.host.list_repos() {
        Ok(repos) => (
            StatusCode::OK,
            Json(StatusBody {
                mode: state.mode.clone(),
                backend: state.backend.clone(),
                repo_count: repos.len(),
            }),
        )
            .into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

pub async fn serve(addr: std::net::SocketAddr, state: AppState) -> std::io::Result<()> {
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, router(state)).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use forgekit_store::MemoryBackend;
    use tower::ServiceExt;

    fn app() -> Router {
        let host = Host::new(Arc::new(MemoryBackend::new()));
        host.create_repo("acme", "app").unwrap();
        router(AppState {
            host: Arc::new(host),
            mode: "local".into(),
            backend: "memory".into(),
        })
    }

    async fn json(res: axum::http::Response<Body>) -> serde_json::Value {
        let bytes = axum::body::to_bytes(res.into_body(), 1024 * 1024)
            .await
            .unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[tokio::test]
    async fn healthz_is_ok() {
        let res = app()
            .oneshot(
                Request::builder()
                    .uri("/healthz")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let body = json(res).await;
        assert_eq!(body["ok"], true);
    }

    #[tokio::test]
    async fn health_status_reports_mode_backend_and_repo_count() {
        let res = app()
            .oneshot(
                Request::builder()
                    .uri("/v1/status")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let body = json(res).await;
        assert_eq!(body["mode"], "local");
        assert_eq!(body["backend"], "memory");
        assert_eq!(body["repo_count"], 1);
    }
}
