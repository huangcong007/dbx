use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;
use dbx_core::transfer_task::TransferTask;

use crate::error::AppError;
use crate::state::WebState;

pub async fn load_transfer_tasks(State(state): State<Arc<WebState>>) -> Result<Json<Vec<TransferTask>>, AppError> {
    let tasks = state.app.storage.load_transfer_tasks().await.map_err(AppError)?;
    Ok(Json(tasks))
}

pub async fn save_transfer_task(
    State(state): State<Arc<WebState>>,
    Json(task): Json<TransferTask>,
) -> Result<Json<TransferTask>, AppError> {
    state.app.storage.save_transfer_task(&task).await.map_err(AppError)?;
    Ok(Json(task))
}

pub async fn delete_transfer_task(
    State(state): State<Arc<WebState>>,
    Path(id): Path<String>,
) -> Result<Json<()>, AppError> {
    state.app.storage.delete_transfer_task(&id).await.map_err(AppError)?;
    Ok(Json(()))
}
