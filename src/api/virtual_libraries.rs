use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use serde::Deserialize;

use crate::{
    api::middleware::{auth::AuthUser, library_admin::LibraryAdminUser},
    dal::VirtualLibrarySourceInput,
    error::AppError,
    models::{UpsertVirtualLibrary, VirtualLibrary, VirtualLibrarySource},
    state::AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_virtual_libraries).post(create_virtual_library))
        .route(
            "/:id",
            get(get_virtual_library)
                .put(update_virtual_library)
                .delete(delete_virtual_library),
        )
        .route("/:id/sources", get(get_sources).put(set_sources))
        .route("/:id/sync", axum::routing::post(enqueue_sync))
}

#[derive(Deserialize)]
struct VirtualLibraryBody {
    name: String,
    root_path: String,
    link_type: String,
}

impl From<VirtualLibraryBody> for UpsertVirtualLibrary {
    fn from(b: VirtualLibraryBody) -> Self {
        UpsertVirtualLibrary {
            name: b.name,
            root_path: b.root_path,
            link_type: b.link_type,
        }
    }
}

#[derive(Deserialize)]
struct SourceEntry {
    library_id: i64,
    library_profile_id: Option<i64>,
    priority: i32,
}

async fn list_virtual_libraries(
    State(state): State<AppState>,
    _auth: LibraryAdminUser,
) -> Result<Json<Vec<VirtualLibrary>>, AppError> {
    Ok(Json(state.db.list_virtual_libraries().await?))
}

async fn get_virtual_library(
    State(state): State<AppState>,
    _auth: LibraryAdminUser,
    Path(id): Path<i64>,
) -> Result<Json<VirtualLibrary>, AppError> {
    state.db.get_virtual_library(id).await.map(Json)
}

async fn create_virtual_library(
    State(state): State<AppState>,
    _auth: LibraryAdminUser,
    Json(body): Json<VirtualLibraryBody>,
) -> Result<(StatusCode, Json<VirtualLibrary>), AppError> {
    let vlib = state.db.create_virtual_library(body.into()).await?;
    Ok((StatusCode::CREATED, Json(vlib)))
}

async fn update_virtual_library(
    State(state): State<AppState>,
    _auth: LibraryAdminUser,
    Path(id): Path<i64>,
    Json(body): Json<VirtualLibraryBody>,
) -> Result<Json<VirtualLibrary>, AppError> {
    state
        .db
        .update_virtual_library(id, body.into())
        .await
        .map(Json)
}

async fn delete_virtual_library(
    State(state): State<AppState>,
    _auth: LibraryAdminUser,
    Path(id): Path<i64>,
) -> Result<StatusCode, AppError> {
    state.db.delete_virtual_library(id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn get_sources(
    State(state): State<AppState>,
    _auth: LibraryAdminUser,
    Path(id): Path<i64>,
) -> Result<Json<Vec<VirtualLibrarySource>>, AppError> {
    Ok(Json(state.db.list_virtual_library_sources(id).await?))
}

async fn set_sources(
    State(state): State<AppState>,
    _auth: LibraryAdminUser,
    Path(id): Path<i64>,
    Json(body): Json<Vec<SourceEntry>>,
) -> Result<StatusCode, AppError> {
    let sources: Vec<VirtualLibrarySourceInput> = body
        .into_iter()
        .map(|s| VirtualLibrarySourceInput {
            library_id: s.library_id,
            library_profile_id: s.library_profile_id,
            priority: s.priority,
        })
        .collect();
    state.db.set_virtual_library_sources(id, sources).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn enqueue_sync(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(id): Path<i64>,
) -> Result<StatusCode, AppError> {
    state
        .db
        .enqueue_job(
            "virtual_sync",
            serde_json::json!({ "virtual_library_id": id }),
            0,
        )
        .await?;
    Ok(StatusCode::ACCEPTED)
}
