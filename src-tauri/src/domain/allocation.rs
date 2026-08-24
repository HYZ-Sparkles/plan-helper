//! 今日分配领域（工单 06，术语 MainBoardTodayAllocation / TodayLoadCommitment /
//! AutoOpenMainBoard）：大面板的数据装配、当日分配的存取（可覆盖重选）、
//! 以及自动打开大面板的判定。分配只记录"日期 + 选中任务集"——
//! 已推进的进度从 ProgressLog 派生、与选择无关（工单 07 接线），本表不冗余任何汇总。
//!
//! 触发侧（进入工作模式 / 启动序列后检测 / 工作窗口开始）分别由工单 08 / 13 接入，
//! 本层只提供纯判定 `should_auto_open`。

use chrono::Datelike;
use rusqlite::{params, Connection};
use serde::Serialize;

use crate::clock::Clock;
use crate::domain::deps::DependencyService;
use crate::domain::plans::{db_err, PlanError, PlanService, PlanStatus, Priority, TaskStatus};
use crate::domain::settings::SettingsService;

/// 大面板一次装配的完整视图（工单 06）。
#[derive(Debug, Serialize)]
pub struct AllocationBoardView {
    /// 分配归属的工作日（YYYY-MM-DD，注入时钟的本地日期）
    pub date: String,
    /// 今日是否在设置的每周工作日里（CONTEXT WorkingHours：非工作日分配不加载，UI 显示休息日态）
    pub workday: bool,
    /// 当日实际目标（分钟）。工单 06 = 基准工作时间；工单 10 升级为含结转的调整后目标并加标注
    pub target_minutes: u32,
    /// 今日已分配的选中集（与当前可选任务取交集后的回显集——计划暂停等 stale 选择被剔除）
    pub selected_task_ids: Vec<i64>,
    /// 候选分组：所有进行中计划，PlanOrdering 默认排序（Priority 降序 + 创建倒序）
    pub groups: Vec<AllocationGroup>,
}

/// 一个进行中计划的分组（section header = 计划名 + 优先级）。
#[derive(Debug, Serialize)]
pub struct AllocationGroup {
    pub plan_id: i64,
    pub plan_name: String,
    pub priority: Priority,
    pub tasks: Vec<AllocationTask>,
}

/// 分组内一条候选任务。被依赖阻塞的任务不进入大面板——只展示可选任务
/// （2026-08-24 用户决策，原"置灰展示等待项"砍掉；阻塞与否仍实时派生）。
#[derive(Debug, Serialize)]
pub struct AllocationTask {
    pub id: i64,
    pub name: String,
    /// 预计耗时（分钟）：无子目标 = 手填值；有子目标 = 子目标求和（落库已归一到本列）
    pub estimated_minutes: u32,
}

/// 今日分配读写服务。
pub struct AllocationService;

impl AllocationService {
    /// 装配大面板视图：候选分组（依赖过滤）+ 今日回显 + 当日目标（基准工作时间）。
    pub fn board(conn: &Connection, clock: &dyn Clock) -> Result<AllocationBoardView, PlanError> {
        let groups = Self::candidates(conn)?;
        let selectable = selectable_of(&groups);
        Ok(AllocationBoardView {
            date: today_string(clock),
            workday: Self::is_workday(conn, clock)?,
            target_minutes: SettingsService::load(conn)
                .map_err(db_err)?
                .daily_minutes,
            selected_task_ids: Self::stored_selection(conn, clock)?
                .into_iter()
                .filter(|id| selectable.contains(id))
                .collect(),
            groups,
        })
    }

    /// 提交当日分配（日期 + 选中任务集），再次提交整行覆盖（重开重选）。
    /// 校验：选中集 ⊆ 当前可选集（进行中计划 + 未完成 + 依赖就绪），防 UI 失步后端兜底。
    pub fn commit(conn: &Connection, clock: &dyn Clock, task_ids: &[i64]) -> Result<(), PlanError> {
        let selectable = selectable_of(&Self::candidates(conn)?);
        if let Some(&id) = task_ids.iter().find(|id| !selectable.contains(id)) {
            return Err(PlanError::TaskNotAllocatable { task_id: id });
        }
        let ids = serde_json::to_string(task_ids).unwrap();
        conn.execute(
            "INSERT INTO today_allocations (id, date, task_ids) VALUES (1, ?1, ?2)
             ON CONFLICT(id) DO UPDATE SET date = ?1, task_ids = ?2",
            params![today_string(clock), ids],
        )
        .map_err(db_err)?;
        Ok(())
    }

    /// 自动打开大面板的判定（AutoOpenMainBoard）：工作模式 + 今日是工作日 + 今日未分配。
    /// 工作模式由调用方传入（桌宠状态，工单 08 落地）；日期例外叠加在工单 13。
    pub fn should_auto_open(conn: &Connection, clock: &dyn Clock, work_mode: bool) -> Result<bool, PlanError> {
        Ok(work_mode
            && Self::is_workday(conn, clock)?
            && !Self::is_allocated_today(conn, clock)?)
    }

    /// 今日是否在设置的每周工作日里（周循环；13 前无日期例外）。
    fn is_workday(conn: &Connection, clock: &dyn Clock) -> Result<bool, PlanError> {
        let workdays = SettingsService::load(conn).map_err(db_err)?.workdays;
        // chrono 周一 = 1 .. 周日 = 7，与 settings.workdays 口径一致
        Ok(workdays.contains(&(clock.now().weekday().number_from_monday() as u8)))
    }

    /// 今日是否已分配（分配行存在且日期就是今天；隔日行视为未分配）。
    fn is_allocated_today(conn: &Connection, clock: &dyn Clock) -> Result<bool, PlanError> {
        Ok(conn
            .query_row(
                "SELECT COUNT(*) FROM today_allocations WHERE id = 1 AND date = ?1",
                params![today_string(clock)],
                |row| row.get::<_, i64>(0),
            )
            .map_err(db_err)?
            > 0)
    }

    /// 库中存储的选中集原样读出（无行或 JSON 损坏 → 空；调用方自行与候选求交）。
    /// progress 域（工单 07）复用：当前任务须落在今日推进列表内。
    pub(crate) fn stored_selection(conn: &Connection, clock: &dyn Clock) -> Result<Vec<i64>, PlanError> {
        let raw: Option<String> = conn
            .query_row(
                "SELECT task_ids FROM today_allocations WHERE id = 1 AND date = ?1",
                params![today_string(clock)],
                |row| row.get(0),
            )
            .map(Some)
            .or_else(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Ok(None),
                e => Err(e),
            })
            .map_err(db_err)?;
        Ok(raw
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default())
    }

    /// 候选分组：进行中计划（抢占未实现，天然同等级）→ 未完成任务 → 依赖就绪过滤
    /// （被阻塞的不展示）。排序直接复用 PlanService::list 的 PlanOrdering；
    /// 就绪判定复用 DependencyService（waiting_on 派生）。
    fn candidates(conn: &Connection) -> Result<Vec<AllocationGroup>, PlanError> {
        let mut groups = Vec::new();
        for plan in PlanService::list(conn)?.into_iter().filter(|p| p.status == PlanStatus::Active) {
            let mut tasks = Vec::new();
            for t in plan.tasks.iter().filter(|t| t.status != TaskStatus::Completed) {
                if !DependencyService::is_unblocked(conn, t.id)? {
                    continue; // 前置未完成：不进入大面板（解锁当日自然回到列表）
                }
                tasks.push(AllocationTask {
                    id: t.id,
                    name: t.name.clone(),
                    estimated_minutes: t.estimated_minutes.unwrap_or(0),
                });
            }
            if !tasks.is_empty() {
                groups.push(AllocationGroup {
                    plan_id: plan.id,
                    plan_name: plan.name,
                    priority: plan.priority,
                    tasks,
                });
            }
        }
        Ok(groups)
    }
}

/// 从候选分组摊平出可选任务 id 集（进入分组的任务全部可选）：board 回显求交与 commit 校验共用。
fn selectable_of(groups: &[AllocationGroup]) -> Vec<i64> {
    groups
        .iter()
        .flat_map(|g| &g.tasks)
        .map(|t| t.id)
        .collect()
}

/// 注入时钟的本地日期（YYYY-MM-DD）。分配/判定的"今日"统一走这里，跨午夜安全由调用侧窗口语义保证。
/// progress 域（工单 07）的今日完成量口径复用。
pub(crate) fn today_string(clock: &dyn Clock) -> String {
    clock.now().format("%Y-%m-%d").to_string()
}
