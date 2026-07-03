use std::sync::Arc;

use dbx_core::connection::AppState;
use dbx_core::transfer_task::TransferTask;
use tauri::State;

#[tauri::command]
pub async fn load_transfer_tasks(state: State<'_, Arc<AppState>>) -> Result<Vec<TransferTask>, String> {
    state.storage.load_transfer_tasks().await
}

#[tauri::command]
pub async fn save_transfer_task(state: State<'_, Arc<AppState>>, task: TransferTask) -> Result<TransferTask, String> {
    state.storage.save_transfer_task(&task).await?;
    Ok(task)
}

#[tauri::command]
pub async fn delete_transfer_task(state: State<'_, Arc<AppState>>, id: String) -> Result<(), String> {
    state.storage.delete_transfer_task(&id).await
}
