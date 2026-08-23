//! 应用快照：UI → command → 领域服务 → 返回 这条接缝通路上的示例读模型。
//! 组装逻辑放在领域层（可集成测试），command 只做转发。

use rusqlite::Connection;
use serde::Serialize;

use crate::clock::Clock;
use crate::domain::settings::{Settings, SettingsService};

/// 启动快照：设置 + 服务端当前时间。
#[derive(Debug, Serialize)]
pub struct AppStateView {
    pub settings: Settings,
    pub server_now: chrono::DateTime<chrono::Local>,
}

/// 组装应用快照（settings 来自 SettingsService，server_now 来自注入时钟）。
pub fn app_state_view(conn: &Connection, clock: &dyn Clock) -> rusqlite::Result<AppStateView> {
    Ok(AppStateView {
        settings: SettingsService::load(conn)?,
        server_now: clock.now(),
    })
}
