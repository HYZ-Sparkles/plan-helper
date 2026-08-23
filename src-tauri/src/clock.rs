//! 时钟注入（spec Testing Decisions）：领域服务一律通过 `&dyn Clock` 取"现在"，
//! 不直接读系统时间，使"今天 / 窗口边界 / 跨午夜 / 次日补登"全部可确定性测试。

use chrono::{DateTime, Local};

/// 时钟抽象：领域层唯一的"现在"来源。
pub trait Clock: Send + Sync {
    fn now(&self) -> DateTime<Local>;
}

/// 生产实现：读系统本地时间。
#[derive(Debug, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> DateTime<Local> {
        Local::now()
    }
}

/// 测试实现：固定时间，构造即可确定任意"今天"。
#[derive(Debug, Clone)]
pub struct FixedClock(pub DateTime<Local>);

impl Clock for FixedClock {
    fn now(&self) -> DateTime<Local> {
        self.0
    }
}
