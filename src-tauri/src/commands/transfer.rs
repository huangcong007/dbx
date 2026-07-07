use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};

use crate::commands::connection::{ensure_connection_writable, AppState};

// Re-export types and functions used by other modules
pub use dbx_core::transfer::{get_db_type, TransferProgress, TransferRequest, TransferStatus};

fn emit_progress(app: &AppHandle, progress: TransferProgress) {
    let _ = app.emit("transfer-progress", progress);
}

#[tauri::command]
pub async fn start_transfer(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    request: TransferRequest,
) -> Result<(), String> {
    let state = state.inner().clone();
    let transfer_id = request.transfer_id.clone();

    // Reject transfer early if the target connection is read-only — writing to it is inherently required
    ensure_connection_writable(&state, &request.target_connection_id, "Transfer").await?;

    // Validate connections exist
    let source_db_type = get_db_type(&state, &request.source_connection_id).await?;
    let target_db_type = get_db_type(&state, &request.target_connection_id).await?;
    dbx_core::transfer::validate_transfer_target_table_names(&request)?;

    // Ensure pools
    let source_pool_key =
        state.get_or_create_pool(&request.source_connection_id, Some(&request.source_database)).await?;
    let target_pool_key =
        state.get_or_create_pool(&request.target_connection_id, Some(&request.target_database)).await?;

    tokio::spawn(async move {
        // Sort tables by FK dependency so referenced tables are transferred first.
        let sorted_tables = dbx_core::transfer::sort_tables_by_fk_dependency(
            &state,
            &request.source_connection_id,
            &request.source_database,
            &request.source_schema,
            &request.tables,
            true,
        )
        .await
        .unwrap_or_else(|e| {
            log::warn!("[transfer] failed to sort tables by FK dependency, using original order: {e}");
            request.tables.clone()
        });

        let total_tables = sorted_tables.len();
        log::info!("[transfer] starting transfer_id={} tables={}", transfer_id, total_tables);

        if matches!(source_db_type, dbx_core::models::connection::DatabaseType::Postgres)
            && matches!(target_db_type, dbx_core::models::connection::DatabaseType::Postgres)
        {
            match dbx_core::transfer::transfer_postgres_schema_dependencies(
                &state,
                &request,
                &source_pool_key,
                &target_pool_key,
                |progress| emit_progress(&app, progress),
            )
            .await
            {
                Ok(()) => {}
                Err(e) if e == "Cancelled" => {
                    emit_progress(
                        &app,
                        TransferProgress {
                            transfer_id: transfer_id.clone(),
                            table: "schema dependencies".to_string(),
                            table_index: 0,
                            total_tables,
                            rows_transferred: 0,
                            total_rows: None,
                            status: TransferStatus::Cancelled,
                            error: None,
                        },
                    );
                    dbx_core::transfer::clear_cancelled(&transfer_id).await;
                    return;
                }
                Err(e) => {
                    emit_progress(
                        &app,
                        TransferProgress {
                            transfer_id: transfer_id.clone(),
                            table: "schema dependencies".to_string(),
                            table_index: 0,
                            total_tables,
                            rows_transferred: 0,
                            total_rows: None,
                            status: TransferStatus::Error,
                            error: Some(e),
                        },
                    );
                    dbx_core::transfer::clear_cancelled(&transfer_id).await;
                    return;
                }
            }
        }

        let failed_tables: std::sync::Arc<std::sync::Mutex<Vec<String>>> =
            std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let app_for_progress = app.clone();
        let transfer_id_for_progress = transfer_id.clone();
        let sorted_tables_for_done = sorted_tables.clone();
        dbx_core::transfer::transfer_tables(
            state.clone(),
            &request,
            &sorted_tables,
            &source_db_type,
            &target_db_type,
            &source_pool_key,
            &target_pool_key,
            move |progress| emit_progress(&app_for_progress, progress),
            {
                let app = app.clone();
                let transfer_id = transfer_id.clone();
                let failed_tables = failed_tables.clone();
                let sorted_tables = sorted_tables_for_done.clone();
                move |table: &str, result: dbx_core::transfer::TableTransferResult| {
                    let i = sorted_tables.iter().position(|t| t == table).unwrap_or(0);
                    match result {
                        Ok(rows) => {
                            emit_progress(
                                &app,
                                TransferProgress {
                                    transfer_id: transfer_id.clone(),
                                    table: table.to_string(),
                                    table_index: i,
                                    total_tables,
                                    rows_transferred: rows,
                                    total_rows: Some(rows),
                                    status: TransferStatus::TableDone,
                                    error: None,
                                },
                            );
                        }
                        Err(e) => {
                            if e == "Cancelled" {
                                emit_progress(
                                    &app,
                                    TransferProgress {
                                        transfer_id: transfer_id.clone(),
                                        table: table.to_string(),
                                        table_index: i,
                                        total_tables,
                                        rows_transferred: 0,
                                        total_rows: None,
                                        status: TransferStatus::Cancelled,
                                        error: None,
                                    },
                                );
                                return;
                            }
                            if let Ok(mut ft) = failed_tables.lock() {
                                ft.push(table.to_string());
                            }
                            emit_progress(
                                &app,
                                TransferProgress {
                                    transfer_id: transfer_id.clone(),
                                    table: table.to_string(),
                                    table_index: i,
                                    total_tables,
                                    rows_transferred: 0,
                                    total_rows: None,
                                    status: TransferStatus::Error,
                                    error: Some(e),
                                },
                            );
                        }
                    }
                }
            },
        )
        .await;

        // 检测取消：transfer_tables 内部各表循环会检查 is_cancelled，但取消后不会主动 return，
        // 这里统一判断是否已取消以跳过后续 schema objects。
        if dbx_core::transfer::is_cancelled(&transfer_id_for_progress).await {
            dbx_core::transfer::clear_cancelled(&transfer_id_for_progress).await;
            return;
        }

        if matches!(source_db_type, dbx_core::models::connection::DatabaseType::Postgres)
            && matches!(target_db_type, dbx_core::models::connection::DatabaseType::Postgres)
        {
            match dbx_core::transfer::transfer_postgres_schema_objects(
                &state,
                &request,
                &source_pool_key,
                &target_pool_key,
                |progress| emit_progress(&app, progress),
            )
            .await
            {
                Ok(()) => {}
                Err(e) if e == "Cancelled" => {
                    emit_progress(
                        &app,
                        TransferProgress {
                            transfer_id: transfer_id.clone(),
                            table: "schema objects".to_string(),
                            table_index: total_tables,
                            total_tables,
                            rows_transferred: 0,
                            total_rows: None,
                            status: TransferStatus::Cancelled,
                            error: None,
                        },
                    );
                    dbx_core::transfer::clear_cancelled(&transfer_id).await;
                    return;
                }
                Err(e) => {
                    if let Ok(mut ft) = failed_tables.lock() {
                        ft.push("schema objects".to_string());
                    }
                    emit_progress(
                        &app,
                        TransferProgress {
                            transfer_id: transfer_id.clone(),
                            table: "schema objects".to_string(),
                            table_index: total_tables,
                            total_tables,
                            rows_transferred: 0,
                            total_rows: None,
                            status: TransferStatus::Error,
                            error: Some(e),
                        },
                    );
                }
            }
        }

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
        emit_progress(
            &app,
            TransferProgress {
                transfer_id: transfer_id.clone(),
                table: String::new(),
                table_index: total_tables,
                total_tables,
                rows_transferred: 0,
                total_rows: None,
                status: failed_summary.0,
                error: failed_summary.1,
            },
        );
        dbx_core::transfer::clear_cancelled(&transfer_id).await;
    });

    Ok(())
}

#[tauri::command]
pub async fn cancel_transfer(transfer_id: String) -> Result<(), String> {
    dbx_core::transfer::set_cancelled(&transfer_id).await;
    Ok(())
}

/// Sort table names by foreign key dependency.
/// `parents_first: true` → parent tables first (insert/export order).
/// `parents_first: false` → child tables first (drop order).
#[allow(dead_code)]
#[tauri::command]
pub async fn sort_tables_by_fk_dependency(
    state: tauri::State<'_, std::sync::Arc<AppState>>,
    connection_id: String,
    database: String,
    schema: String,
    tables: Vec<String>,
    parents_first: bool,
) -> Result<Vec<String>, String> {
    dbx_core::transfer::sort_tables_by_fk_dependency(&state, &connection_id, &database, &schema, &tables, parents_first)
        .await
}
