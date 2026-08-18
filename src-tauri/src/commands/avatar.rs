//! 桌宠/BongoCat 功能已移除（本地化改造）。
//! 保留命令签名与 DTO，返回"功能已移除"错误，保持 commands::xxx 路径不断。

use crate::AppState;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, State};
use work_review_core::error::AppError;

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AvatarFollowupActionInput {
    pub action: String,
    pub project_key: String,
    pub title: String,
    pub date: String,
    pub source_app: String,
    pub source_title: String,
    pub persona: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GnomeAvatarExtensionInstallResult {
    pub installed: bool,
    pub enabled: bool,
    pub requires_relogin: bool,
    pub extension_dir: String,
    pub message: String,
}

#[tauri::command]
pub async fn get_avatar_state(
    _state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<serde_json::Value, AppError> {
    Ok(serde_json::json!({}))
}

#[tauri::command]
pub async fn save_avatar_position(
    _x: i32,
    _y: i32,
    _state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<(), AppError> {
    Ok(())
}

#[tauri::command]
pub async fn set_avatar_window_expanded(
    _expanded: bool,
    _app: AppHandle,
) -> Result<(), AppError> {
    Ok(())
}

#[tauri::command]
pub async fn set_avatar_interactive_regions(
    _precise: bool,
    _app: AppHandle,
) -> Result<(), AppError> {
    Ok(())
}

#[tauri::command]
pub async fn persist_avatar_position(
    _app: AppHandle,
    _state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<(), AppError> {
    Ok(())
}

#[tauri::command]
pub async fn show_main_window(
    _app: AppHandle,
    _source_window_label: Option<String>,
) -> Result<(), AppError> {
    crate::reveal_main_window(&_app, _source_window_label.as_deref())
        .map_err(|e| AppError::Unknown(e.to_string()))
}

#[tauri::command]
pub async fn handle_avatar_followup_action(
    _input: AvatarFollowupActionInput,
    _app: AppHandle,
    _state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<(), AppError> {
    Ok(())
}

#[tauri::command]
pub async fn install_gnome_avatar_extension() -> Result<GnomeAvatarExtensionInstallResult, AppError> {
    Err(AppError::Config("桌宠功能已移除".to_string()))
}