/**
 * 后端 command 的类型化封装：类型与 src-tauri 领域层的 serde 结构一一对应。
 * 新 command 先在这里登记类型与包装函数，UI 组件不直接 invoke。
 */
import { invoke } from "@tauri-apps/api/core";

/** 日内一段时间窗口（分钟数端点；跨午夜以 end < start 表达） */
export interface TimeWindow {
  start_minute: number;
  end_minute: number;
}

/** 全局设置（对应 domain::settings::Settings） */
export interface Settings {
  daily_minutes: number;
  workdays: number[]; // 周一=1 .. 周日=7
  time_windows: TimeWindow[];
  smoothing_workdays: number;
}

/** 启动快照（对应 domain::app_state::AppStateView） */
export interface AppStateView {
  settings: Settings;
  server_now: string;
}

/** 示例接缝命令：读启动快照（设置 + 服务端时间） */
export function getAppState(): Promise<AppStateView> {
  return invoke<AppStateView>("get_app_state");
}
