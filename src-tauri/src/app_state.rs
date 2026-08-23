//! Tauri 托管的应用状态：数据库连接 + 注入时钟。
//! command 层从这里解包依赖，领域服务保持窗口无关。

use crate::clock::Clock;
use rusqlite::Connection;
use std::sync::Mutex;

/// 全局共享状态（Tauri manage）
pub struct AppState {
    pub db: Mutex<Connection>,
    pub clock: Box<dyn Clock>,
}
