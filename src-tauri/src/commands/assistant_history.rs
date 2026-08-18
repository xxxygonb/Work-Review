//! 助手会话持久化功能已移除（本地化改造）。
//! 保留命令签名，返回空结果，保持 commands::xxx 路径不断。

use crate::AppState;
use std::sync::{Arc, Mutex};
use tauri::State;
use work_review_core::database::{AssistantConversation, AssistantStoredMessage};
use work_review_core::error::AppError;

#[tauri::command]
pub async fn list_assistant_conversations(
    _limit: Option<u32>,
    _state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<Vec<AssistantConversation>, AppError> {
    Ok(vec![])
}

#[tauri::command]
pub async fn create_assistant_conversation(
    _title: String,
    _state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<i64, AppError> {
    Err(AppError::Config("AI 助手功能已移除".to_string()))
}

#[tauri::command]
pub async fn get_assistant_messages(
    _conversation_id: i64,
    _state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<Vec<AssistantStoredMessage>, AppError> {
    Ok(vec![])
}

#[tauri::command]
pub async fn append_assistant_message(
    _conversation_id: i64,
    _role: String,
    _content: String,
    _tool_digest: Option<String>,
    _model_name: Option<String>,
    _state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<i64, AppError> {
    Err(AppError::Config("AI 助手功能已移除".to_string()))
}

#[tauri::command]
pub async fn delete_assistant_conversation(
    _conversation_id: i64,
    _state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<(), AppError> {
    Ok(())
}