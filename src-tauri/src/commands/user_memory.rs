//! 用户长期记忆功能已移除（本地化改造）。
//! 保留命令签名与 DTO，返回空结果，保持 commands::xxx 路径不断。

use crate::AppState;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tauri::State;
use work_review_core::database::{AssistantUserMemory, Database};

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UserMemoryInput {
    pub memory_type: String,
    pub memory_key: String,
    pub value_text: String,
    pub recall_policy: String,
    pub sensitivity: String,
    pub expires_at: Option<i64>,
}

impl std::fmt::Debug for UserMemoryInput {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("UserMemoryInput")
            .field("memory_type", &self.memory_type)
            .field("memory_key", &"<redacted>")
            .field("value_text", &"<redacted>")
            .field("recall_policy", &self.recall_policy)
            .field("sensitivity", &self.sensitivity)
            .field("expires_at", &self.expires_at)
            .finish()
    }
}

fn database_from_state(state: &State<'_, Arc<Mutex<AppState>>>) -> Result<Database, String> {
    state
        .lock()
        .map(|state| state.database.clone())
        .map_err(|error| format!("读取长期记忆状态失败: {error}"))
}

#[tauri::command]
pub async fn list_user_memories(
    memory_type: Option<String>,
    limit: Option<usize>,
    state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<Vec<AssistantUserMemory>, String> {
    database_from_state(&state)?
        .list_user_memories(memory_type.as_deref(), limit.unwrap_or(200))
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn create_user_memory(
    _input: UserMemoryInput,
    _state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<AssistantUserMemory, String> {
    Err("用户记忆功能已移除".to_string())
}

#[tauri::command]
pub async fn update_user_memory(
    _id: i64,
    _input: UserMemoryInput,
    _expected_revision: i64,
    _state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<AssistantUserMemory, String> {
    Err("用户记忆功能已移除".to_string())
}

#[tauri::command]
pub async fn delete_user_memory(
    _id: i64,
    _state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<(), String> {
    Err("用户记忆功能已移除".to_string())
}

#[tauri::command]
pub async fn clear_user_memories(_state: State<'_, Arc<Mutex<AppState>>>) -> Result<usize, String> {
    Ok(0)
}