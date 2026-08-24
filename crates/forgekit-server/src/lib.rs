//! JSON HTTP API.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use forgekit_core::{Error, Host, PromoteRequest, PushRequest};
use serde::{Deserialize, Serialize};
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

#[derive(Debug, Serialize)]
pub struct ErrorBody {
    pub error: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateRepoBody {
    pub owner: String,
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct ApproveBody {
    pub actor: String,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/v1/status", get(status))
        .route("/v1/repos", get(list_repos).post(create_repo))
        .route("/v1/repos/{owner}/{name}", get(get_repo))
        .route("/v1/repos/{owner}/{name}/push", post(push))
        .route(
            "/v1/repos/{owner}/{name}/checkpoints",
            get(list_checkpoints),
        )
        .route(
            "/v1/repos/{owner}/{name}/checkpoints/{id}",
            get(get_checkpoint),
        )
        .route(
            "/v1/repos/{owner}/{name}/checkpoints/{id}/approve",
            post(approve),
        )
        .route("/v1/repos/{owner}/{name}/promote", post(promote))
        .route("/v1/repos/{owner}/{name}/events", get(events))
        .with_state(state)
}

fn map_error(e: Error) -> (StatusCode, Json<ErrorBody>) {
    let status = match &e {
        Error::RepoNotFound(_) | Error::CheckpointNotFound(_) => StatusCode::NOT_FOUND,
        Error::RepoExists(_) | Error::CasConflict(_, _, _) | Error::CheckpointNotPending(_) => {
            StatusCode::CONFLICT
        }
        Error::PromoteRefused(_, _) => StatusCode::FORBIDDEN,
        Error::InvalidName(_) => StatusCode::BAD_REQUEST,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    };
    (
        status,
        Json(ErrorBody {
            error: e.to_string(),
        }),
    )
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
        Err(e) => map_error(e).into_response(),
    }
}

async fn list_repos(State(state): State<AppState>) -> impl IntoResponse {
    match state.host.list_repos() {
        Ok(repos) => (StatusCode::OK, Json(repos)).into_response(),
        Err(e) => map_error(e).into_response(),
    }
}

async fn create_repo(
    State(state): State<AppState>,
    Json(body): Json<CreateRepoBody>,
) -> impl IntoResponse {
    match state.host.create_repo(&body.owner, &body.name) {
        Ok(sum) => (StatusCode::CREATED, Json(sum)).into_response(),
        Err(e) => map_error(e).into_response(),
    }
}

async fn get_repo(
    State(state): State<AppState>,
    Path((owner, name)): Path<(String, String)>,
) -> impl IntoResponse {
    match state.host.get_repo(&owner, &name) {
        Ok(sum) => (StatusCode::OK, Json(sum)).into_response(),
        Err(e) => map_error(e).into_response(),
    }
}

async fn push(
    State(state): State<AppState>,
    Path((owner, name)): Path<(String, String)>,
    Json(req): Json<PushRequest>,
) -> impl IntoResponse {
    match state.host.push(&owner, &name, req) {
        Ok((manifest, oid)) => (
            StatusCode::OK,
            Json(serde_json::json!({ "manifest": manifest, "oid": oid })),
        )
            .into_response(),
        Err(e) => map_error(e).into_response(),
    }
}

async fn list_checkpoints(
    State(state): State<AppState>,
    Path((owner, name)): Path<(String, String)>,
) -> impl IntoResponse {
    match state.host.list_checkpoints(&owner, &name) {
        Ok(list) => (StatusCode::OK, Json(list)).into_response(),
        Err(e) => map_error(e).into_response(),
    }
}

async fn get_checkpoint(
    State(state): State<AppState>,
    Path((owner, name, id)): Path<(String, String, String)>,
) -> impl IntoResponse {
    match state.host.get_checkpoint(&owner, &name, &id) {
        Ok(ck) => (StatusCode::OK, Json(ck)).into_response(),
        Err(e) => map_error(e).into_response(),
    }
}

async fn approve(
    State(state): State<AppState>,
    Path((owner, name, id)): Path<(String, String, String)>,
    Json(body): Json<ApproveBody>,
) -> impl IntoResponse {
    match state.host.approve(&owner, &name, &id, &body.actor) {
        Ok(ck) => (StatusCode::OK, Json(ck)).into_response(),
        Err(e) => map_error(e).into_response(),
    }
}

async fn promote(
    State(state): State<AppState>,
    Path((owner, name)): Path<(String, String)>,
    Json(req): Json<PromoteRequest>,
) -> impl IntoResponse {
    match state.host.promote(&owner, &name, req) {
        Ok(entry) => (StatusCode::OK, Json(entry)).into_response(),
        Err(e) => map_error(e).into_response(),
    }
}

async fn events(
    State(state): State<AppState>,
    Path((owner, name)): Path<(String, String)>,
) -> impl IntoResponse {
    match state.host.list_wal(&owner, &name) {
        Ok(wal) => (StatusCode::OK, Json(wal)).into_response(),
        Err(e) => map_error(e).into_response(),
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
    use forgekit_core::{CheckpointKind, Session};
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

    fn empty() -> Router {
        router(AppState {
            host: Arc::new(Host::new(Arc::new(MemoryBackend::new()))),
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

    async fn call(app: Router, req: Request<Body>) -> axum::http::Response<Body> {
        app.oneshot(req).await.unwrap()
    }

    #[tokio::test]
    async fn healthz_is_ok() {
        let res = call(
            app(),
            Request::builder()
                .uri("/healthz")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(res.status(), StatusCode::OK);
        let body = json(res).await;
        assert_eq!(body["ok"], true);
    }

    #[tokio::test]
    async fn health_status_reports_mode_backend_and_repo_count() {
        let res = call(
            app(),
            Request::builder()
                .uri("/v1/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(res.status(), StatusCode::OK);
        let body = json(res).await;
        assert_eq!(body["mode"], "local");
        assert_eq!(body["backend"], "memory");
        assert_eq!(body["repo_count"], 1);
    }

    #[tokio::test]
    async fn api_create_push_checkpoint_approve_roundtrip() {
        let state = AppState {
            host: Arc::new(Host::new(Arc::new(MemoryBackend::new()))),
            mode: "local".into(),
            backend: "memory".into(),
        };
        let host = state.host.clone();
        let app = router(state);

        let res = call(
            app.clone(),
            Request::builder()
                .method("POST")
                .uri("/v1/repos")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"owner":"acme","name":"app"}"#))
                .unwrap(),
        )
        .await;
        assert_eq!(res.status(), StatusCode::CREATED);
        let created = json(res).await;
        assert_eq!(created["seq"], 0);
        let in_proc = host.get_repo("acme", "app").unwrap();
        assert_eq!(in_proc.seq, 0);

        let push = PushRequest {
            r#ref: "main".into(),
            message: "feat".into(),
            actor: "pi".into(),
            files: vec!["a.rs".into()],
            session: Some(Session {
                prompt: Some("do it".into()),
                ..Default::default()
            }),
            kind: Some(CheckpointKind::Release),
        };
        let res = call(
            app.clone(),
            Request::builder()
                .method("POST")
                .uri("/v1/repos/acme/app/push")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&push).unwrap()))
                .unwrap(),
        )
        .await;
        assert_eq!(res.status(), StatusCode::OK);

        let res = call(
            app.clone(),
            Request::builder()
                .uri("/v1/repos/acme/app/checkpoints")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(res.status(), StatusCode::OK);
        let ckpts = json(res).await;
        assert_eq!(ckpts.as_array().unwrap().len(), 1);
        let id = ckpts[0]["id"].as_str().unwrap().to_string();
        let in_proc = host.list_checkpoints("acme", "app").unwrap();
        assert_eq!(in_proc[0].id, id);
        assert_eq!(in_proc[0].status, forgekit_core::CheckpointStatus::Pending);

        let res = call(
            app.clone(),
            Request::builder()
                .method("POST")
                .uri(format!("/v1/repos/acme/app/checkpoints/{id}/approve"))
                .header("content-type", "application/json")
                .body(Body::from(r#"{"actor":"josh"}"#))
                .unwrap(),
        )
        .await;
        assert_eq!(res.status(), StatusCode::OK);
        let approved = json(res).await;
        assert_eq!(approved["status"], "approved");
        assert_eq!(
            host.get_checkpoint("acme", "app", &id).unwrap().status,
            forgekit_core::CheckpointStatus::Approved
        );
    }

    #[tokio::test]
    async fn api_gate_errors_are_not_500() {
        let app = empty();
        let res = call(
            app.clone(),
            Request::builder()
                .method("POST")
                .uri("/v1/repos")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"owner":"acme","name":"../x"}"#))
                .unwrap(),
        )
        .await;
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);

        let res = call(
            empty(),
            Request::builder()
                .method("POST")
                .uri("/v1/repos")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"owner":"acme","name":"app"}"#))
                .unwrap(),
        )
        .await;
        assert_eq!(res.status(), StatusCode::CREATED);

        let host = Host::new(Arc::new(MemoryBackend::new()));
        host.create_repo("acme", "app").unwrap();
        let app = router(AppState {
            host: Arc::new(host),
            mode: "local".into(),
            backend: "memory".into(),
        });
        let res = call(
            app.clone(),
            Request::builder()
                .method("POST")
                .uri("/v1/repos")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"owner":"acme","name":"app"}"#))
                .unwrap(),
        )
        .await;
        assert_eq!(res.status(), StatusCode::CONFLICT);

        let res = call(
            app.clone(),
            Request::builder()
                .method("POST")
                .uri("/v1/repos/acme/app/promote")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"ref":"main","actor":"josh","required_kind":"release","remote":"github"}"#,
                ))
                .unwrap(),
        )
        .await;
        assert_eq!(res.status(), StatusCode::FORBIDDEN);
        assert_ne!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let body = json(res).await;
        assert!(body["error"].as_str().unwrap().contains("promote refused"));
    }
}
