//! AI 功能已移除（本地化改造）。
//! 保留命令签名与 DTO，返回"功能已移除"错误，保持 commands::xxx 路径不断。

use serde::{Deserialize, Serialize};
use work_review_core::config::ModelConfig;
use work_review_core::error::AppError;

#[derive(Serialize, Deserialize, Debug)]
pub struct ModelTestResult {
    pub success: bool,
    pub message: String,
    pub response_time_ms: u64,
    pub model_info: Option<String>,
}

#[tauri::command]
pub async fn test_model(_model_config: ModelConfig) -> Result<ModelTestResult, AppError> {
    Err(AppError::Config("AI 功能已移除".to_string()))
}

#[tauri::command]
pub async fn fetch_models(
    _provider: String,
    _endpoint: String,
    _api_key: Option<String>,
) -> Result<Vec<serde_json::Value>, AppError> {
    Err(AppError::Config("AI 功能已移除".to_string()))
}

#[tauri::command]
pub async fn test_assistant_search(
    _provider: String,
    _api_key: Option<String>,
    _query: Option<String>,
) -> Result<serde_json::Value, AppError> {
    Err(AppError::Config("AI 功能已移除".to_string()))
}

#[tauri::command]
pub async fn get_ai_providers() -> Result<Vec<serde_json::Value>, AppError> {
    Ok(vec![])
}

pub fn validate_model_endpoint(_endpoint: &str) -> Result<(), AppError> {
    Ok(())
}