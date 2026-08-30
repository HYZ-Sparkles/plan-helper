//! 领域服务层：不依赖 Tauri 窗口与时钟系统（时钟经 `Clock` 注入），
//! 全部自动化测试打在这一层（spec 接缝决策）。

pub mod allocation;
pub mod app_state;
pub mod deps;
pub mod ledger;
pub mod lifecycle;
pub mod plans;
pub mod progress;
pub mod settings;
pub mod summary;
