//! AI 问答/助手功能已移除（本地化改造）。
//! 保留命令签名与 DTO，返回"功能已移除"错误，保持 commands::xxx 路径不断。

use crate::AppState;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tauri::State;
use work_review_core::config::ModelConfig;
use work_review_core::error::AppError;

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AssistantAnswer {
    pub answer: String,
    pub references: Vec<work_review_core::database::MemorySearchItem>,
    pub used_ai: bool,
    pub model_name: Option<String>,
    pub tool_labels: Vec<String>,
    pub cards: Vec<AssistantCard>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AssistantChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AssistantCard {
    pub kind: String,
    pub title: String,
    pub content: serde_json::Value,
}

#[tauri::command]
pub async fn chat_work_assistant(
    _question: String,
    _date: Option<String>,
    _date_from: Option<String>,
    _date_to: Option<String>,
    _conversation_id: Option<i64>,
    _history: Option<Vec<AssistantChatMessage>>,
    _model_config: Option<ModelConfig>,
    _locale: Option<String>,
    _request_id: Option<String>,
    _state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<AssistantAnswer, AppError> {
    Err(AppError::Config("AI 助手功能已移除".to_string()))
}

#[tauri::command]
pub async fn cancel_assistant_request(_request_id: String) -> Result<(), AppError> {
    Ok(())
}

#[tauri::command]
pub async fn confirm_assistant_action(_confirm_id: String, _approved: bool) -> Result<(), AppError> {
    Ok(())
}

#[tauri::command]
pub async fn generate_text_with_model(
    _model_config: ModelConfig,
    _prompt: String,
    _state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<String, AppError> {
    Err(AppError::Config("AI 功能已移除".to_string()))
}