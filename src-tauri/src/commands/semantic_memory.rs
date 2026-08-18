//! 语义记忆功能已移除（本地化改造）。
//! 保留命令签名与 DTO，返回"功能已移除"错误，保持 commands::xxx 路径不断。

use crate::AppState;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tauri::State;
use work_review_core::database::SemanticMemoryStats;
use work_review_core::error::AppError;

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SemanticIndexProgress {
    pub processed_activities: usize,
    pub upserted_chunks: usize,
    pub embedded_chunks: usize,
    pub pending_embeddings: usize,
    pub activities_done: bool,
    pub stats: SemanticMemoryStats,
    pub state: work_review_core::database::SemanticMemoryIndexState,
}

#[tauri::command]
pub async fn index_semantic_memory(
    _state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<SemanticIndexProgress, AppError> {
    Err(AppError::Config("语义记忆功能已移除".to_string()))
}

#[tauri::command]
pub async fn test_embedding_model(
    _provider: String,
    _endpoint: String,
    _model: String,
    _api_key: Option<String>,
) -> Result<serde_json::Value, AppError> {
    Err(AppError::Config("语义记忆功能已移除".to_string()))
}

#[tauri::command]
pub async fn semantic_memory_status(
    _state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<SemanticMemoryStats, AppError> {
    Ok(SemanticMemoryStats {
        total_chunks: 0,
        embedded_chunks: 0,
        last_indexed_activity_id: 0,
    })
}

pub fn embedding_config_fingerprint<T>(_config: &T) -> String {
    String::new()
}

pub fn privacy_fingerprint<T>(_privacy: &T) -> String {
    String::new()
}

pub fn invalidate_semantic_memory_index(_db: &work_review_core::database::Database) -> Result<(), AppError> {
    Ok(())
}