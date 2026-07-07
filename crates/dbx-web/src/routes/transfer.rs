use std::sync::Arc;

use axum::extract::{Path, State};
use axum::response::sse::{Event, Sse};
use axum::Json;
use dbx_core::transfer::{self, TransferRequest, TransferStatus};
use futures::stream::Stream;
use serde::Deserialize;

use crate::error::AppError;
use crate::state::WebState;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartTransferRequest {
    pub request: TransferRequest,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelTransferRequest {
    pub transfer_id: String,
}

pub async fn start_transfer(
    State(state): State<Arc<WebState>>,
    Json(body): Json<StartTransferRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let req = body.request;
    transfer::validate_transfer_target_table_names(&req).map_err(AppError)?;

    // Reject transfer early if the target connection is read-only
    if let Some(name) = dbx_core::query::connection_readonly_name(&state.app, &req.target_connection_id).await {
        return Err(AppError(format!(
            "Read-only mode: target connection '{}' has read-only protection enabled. Transfer blocked.",
            name
        )));
    }

    let transfer_id = req.transfer_id.clone();

    // Create a broadcast channel for progress
    let (tx, _) = tokio::sync::broadcast::channel::<String>(256);
    state.sse_channels.write().await.insert(transfer_id.clone(), tx.clone());

    let app = state.app.clone();
    let state_clone = state.clone();

    tokio::spawn(async move {
        let source_db_type = match transfer::get_db_type(&app, &req.source_connection_id).await {
            Ok(t) => t,
            Err(e) => {
                let _ = tx.send(serde_json::json!({"error": e}).to_string());
                return;
            }
        };
        let target_db_type = match transfer::get_db_type(&app, &req.target_connection_id).await {
            Ok(t) => t,
            Err(e) => {
                let _ = tx.send(serde_json::json!({"error": e}).to_string());
                return;
            }
        };

        let source_pool_key = match app.get_or_create_pool(&req.source_connection_id, Some(&req.source_database)).await
        {
            Ok(k) => k,
            Err(e) => {
                let _ = tx.send(serde_json::json!({"error": e}).to_string());
                return;
            }
        };
        let target_pool_key = match app.get_or_create_pool(&req.target_connection_id, Some(&req.target_database)).await
        {
            Ok(k) => k,
            Err(e) => {
                let _ = tx.send(serde_json::json!({"error": e}).to_string());
                return;
            }
        };

        let tables = req.tables.clone();
        // Sort by FK dependency so referenced tables are transferred first.
        let tables = transfer::sort_tables_by_fk_dependency(
            &app,
            &req.source_connection_id,
            &req.source_database,
            &req.source_schema,
            &tables,
            true,
        )
        .await
        .unwrap_or_else(|e| {
            log::warn!("[transfer] failed to sort tables by FK dependency, using original order: {e}");
            tables
        });
        let failed_tables: std::sync::Arc<std::sync::Mutex<Vec<String>>> =
            std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let total_tables = tables.len();
        let tables_for_done = tables.clone();
        let tx_for_progress = tx.clone();
        let req_id = req.transfer_id.clone();
        transfer::transfer_tables(
            app.clone(),
            &req,
            &tables,
            &source_db_type,
            &target_db_type,
            &source_pool_key,
            &target_pool_key,
            move |progress| {
                if let Ok(json) = serde_json::to_string(&progress) {
                    let _ = tx_for_progress.send(json);
                }
            },
            {
                let tx = tx.clone();
                let req_id = req_id.clone();
                let failed_tables = failed_tables.clone();
                let tables = tables_for_done.clone();
                move |table: &str, result: transfer::TableTransferResult| {
                    let i = tables.iter().position(|t| t == table).unwrap_or(0);
                    match result {
                        Ok(rows) => {
                            let progress = transfer::TransferProgress {
                                transfer_id: req_id.clone(),
                                table: table.to_string(),
                                table_index: i,
                                total_tables,
                                rows_transferred: rows,
                                total_rows: Some(rows),
                                status: TransferStatus::TableDone,
                                error: None,
                            };
                            if let Ok(json) = serde_json::to_string(&progress) {
                                let _ = tx.send(json);
                            }
                        }
                        Err(e) => {
                            if e == "Cancelled" {
                                let progress = transfer::TransferProgress {
                                    transfer_id: req_id.clone(),
                                    table: table.to_string(),
                                    table_index: i,
                                    total_tables,
                                    rows_transferred: 0,
                                    total_rows: None,
                                    status: TransferStatus::Cancelled,
                                    error: None,
                                };
                                if let Ok(json) = serde_json::to_string(&progress) {
                                    let _ = tx.send(json);
                                }
                                return;
                            }
                            if let Ok(mut ft) = failed_tables.lock() {
                                ft.push(table.to_string());
                            }
                            let progress = transfer::TransferProgress {
                                transfer_id: req_id.clone(),
                                table: table.to_string(),
                                table_index: i,
                                total_tables,
                                rows_transferred: 0,
                                total_rows: None,
                                status: TransferStatus::Error,
                                error: Some(e),
                            };
                            if let Ok(json) = serde_json::to_string(&progress) {
                                let _ = tx.send(json);
                            }
                        }
                    }
                }
            },
        )
        .await;

        if transfer::is_cancelled(&req.transfer_id).await {
            transfer::clear_cancelled(&req.transfer_id).await;
            state_clone.remove_sse_channel(&req.transfer_id).await;
            return;
        }

        // Send done
        let failed_summary = failed_tables
            .lock()
            .map(|ft| {
                if ft.is_empty() {
                    (TransferStatus::Done, None)
                } else {
                    (
                        TransferStatus::Error,
                        Some(format!(
                            "{} table(s) failed: {}",
                            ft.len(),
                            ft.iter().take(5).cloned().collect::<Vec<_>>().join(", ")
                        )),
                    )
                }
            })
            .unwrap_or((TransferStatus::Error, Some("failed to read failed_tables".to_string())));
        let done = transfer::TransferProgress {
            transfer_id: req.transfer_id.clone(),
            table: String::new(),
            table_index: tables.len(),
            total_tables: tables.len(),
            rows_transferred: 0,
            total_rows: None,
            status: failed_summary.0,
            error: failed_summary.1,
        };
        if let Ok(json) = serde_json::to_string(&done) {
            let _ = tx.send(json);
        }

        transfer::clear_cancelled(&req.transfer_id).await;
        state_clone.remove_sse_channel(&req.transfer_id).await;
    });

    Ok(Json(serde_json::json!({ "transferId": transfer_id })))
}

pub async fn transfer_progress(
    State(state): State<Arc<WebState>>,
    Path(transfer_id): Path<String>,
) -> Result<Sse<impl Stream<Item = Result<Event, std::convert::Infallible>>>, AppError> {
    let channels = state.sse_channels.read().await;
    let tx = channels.get(&transfer_id).ok_or_else(|| AppError("Transfer not found".to_string()))?;
    let rx = tx.subscribe();
    drop(channels);
    Ok(crate::sse::sse_from_channel(rx))
}

pub async fn cancel_transfer(
    State(_state): State<Arc<WebState>>,
    Json(req): Json<CancelTransferRequest>,
) -> Json<serde_json::Value> {
    transfer::set_cancelled(&req.transfer_id).await;
    Json(serde_json::json!({ "cancelled": true }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SortTablesByFkRequest {
    pub connection_id: String,
    pub database: String,
    pub schema: String,
    pub tables: Vec<String>,
    pub parents_first: bool,
}

pub async fn sort_tables_by_fk_dependency(
    State(state): State<Arc<WebState>>,
    Json(req): Json<SortTablesByFkRequest>,
) -> Result<Json<Vec<String>>, AppError> {
    transfer::sort_tables_by_fk_dependency(
        &state.app,
        &req.connection_id,
        &req.database,
        &req.schema,
        &req.tables,
        req.parents_first,
    )
    .await
    .map(Json)
    .map_err(AppError)
}
