//! 工作洞察功能已移除（本地化改造）。
//! 保留命令签名，返回空结果，保持 commands::xxx 路径不断。

use crate::AppState;
use std::sync::{Arc, Mutex};
use tauri::State;
use work_review_core::error::AppError;

#[tauri::command]
pub async fn synthesize_insights(
    _date: Option<String>,
    _state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<Vec<work_review_core::database::WorkInsight>, AppError> {
    Ok(vec![])
}