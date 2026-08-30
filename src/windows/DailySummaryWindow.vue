<template>
  <!--
    今日总结（工单 11，术语 DailySummaryLayout）：顶部总览行（推进计划数 / 完成 /
    目标含结转 / 进度条）+「计划 → 任务 → 推进内容」三层嵌套。被抢占暂停的计划照常
    展示并标注；当日零推进的计划不出现。内容从 ProgressLog 实时派生（ADR-0009）——
    打开时与进展事件（daily-summary:refresh）到达时都重取最新，无任何本地缓存账。
    休息日打开 = 加班态（无目标义务：不显示目标与进度条，与大小面板同一口径）。
  -->
  <div class="summary">
    <header class="summary-header">
      <!-- 内容列限宽居中（与大面板同款全局 .content-col，2026-08-30 反馈：最大化后不横跨整屏） -->
      <div class="header-inner content-col">
        <div>
          <h1 class="title">{{ titleLabel }}</h1>
          <p class="hint">当日推进按工作窗口开始日归属；撤销与修正已实时重算</p>
        </div>
        <p class="date">{{ dateLabel }}</p>
      </div>
    </header>

    <main class="summary-body">
      <!-- 滚动收进列自身（content-col-scroll）：与大面板同款，滚动条贴列右缘三段对齐不漂移 -->
      <div class="content-col content-col-scroll thin-scrollbar">
        <p v-if="loadError" class="error">{{ loadError }}</p>
        <p v-else-if="!summary" class="hint loading">加载中…</p>

        <template v-else>
          <!-- 总览行：N 计划 / 完成 X / 目标 Y（含结转标注）/ 进度条 -->
          <section class="overview">
            <div class="stat">
              <span class="stat-num">{{ summary.plans.length }}</span>
              <span class="stat-unit">推进计划</span>
            </div>
            <div class="stat">
              <PhCheckCircle :size="18" class="stat-icon done" />
              <span class="stat-num">{{ hoursLabel(summary.total_minutes) }}</span>
              <span class="stat-unit">完成小时</span>
            </div>
            <div v-if="summary.workday" class="stat">
              <PhTarget :size="18" class="stat-icon" />
              <span class="stat-num">{{ hoursLabel(summary.target_minutes) }}</span>
              <span class="stat-unit">目标小时</span>
              <span v-if="carryText" class="carry-note">（{{ carryText }}）</span>
            </div>
            <MicroBar
              v-if="summary.workday"
              class="overview-bar"
              :ratio="summary.target_minutes > 0 ? summary.total_minutes / summary.target_minutes : 0"
              :reached="summary.total_minutes >= summary.target_minutes"
            />
          </section>
          <p v-if="!summary.workday" class="hint overtime-note">
            休息日加班：推进按超额并入工时账户，无目标义务
          </p>

          <!-- 有更高优先级计划未开始（不忘主次） -->
          <p v-if="summary.higher_priority_hint" class="priority-hint">
            <PhCaretUp :size="14" /> 有更高优先级的计划还未开始
          </p>

          <!-- 空态：当日没有任何推进 -->
          <div v-if="summary.plans.length === 0" class="empty">
            <PhCoffee :size="26" />
            <p>这一天没有推进记录</p>
          </div>

          <!-- 计划 section（第一层）→ 任务行（第二层）→ 推进内容（第三层） -->
          <section v-for="p in summary.plans" :key="p.plan_id" class="plan">
            <header class="plan-head">
              <PhFlag :size="15" class="plan-flag" />
              <span class="plan-name">{{ p.plan_name }}</span>
              <PriorityLabel :priority="p.priority" />
              <span v-if="p.preempted" class="preempted-tag">已被抢占暂停</span>
              <span class="plan-minutes">{{ hoursFromMinutes(p.minutes) }} 小时</span>
            </header>
            <div v-for="t in p.tasks" :key="t.task_id" class="task">
              <div class="task-row">
                <span class="task-name">{{ t.name }}</span>
                <span class="task-percent">{{ t.percent }}%</span>
                <span class="task-minutes">{{ hoursFromMinutes(t.minutes) }} 小时</span>
              </div>
              <div class="task-detail">
                <template v-if="t.has_subgoals">
                  <span v-for="sg in completedOf(t)" :key="sg.id" class="sg done">
                    <PhCheck :size="12" weight="bold" /> {{ sg.name }}
                  </span>
                  <span v-if="doingOf(t)" class="sg doing">{{ doingOf(t)!.name }} 进行中</span>
                </template>
                <MicroBar
                  v-else
                  class="task-bar"
                  :ratio="t.percent / 100"
                  :reached="t.percent >= 100"
                />
              </div>
            </div>
          </section>
        </template>
      </div>
    </main>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { PhCheck, PhCheckCircle, PhCoffee, PhCaretUp, PhFlag, PhTarget } from "@phosphor-icons/vue";
import MicroBar from "../components/MicroBar.vue";
import PriorityLabel from "../components/PriorityLabel.vue";
import {
  getDailySummary,
  getDailySummaryStatus,
  type DailySummaryView,
  type SummaryTask,
} from "../lib/api";
import { carryLabel, hoursFromMinutes, hoursLabel, planErrorMessage } from "../lib/labels";
import { SUMMARY_REFRESH_EVENT, SUMMARY_SHOW_EVENT } from "../lib/summary";

const win = getCurrentWebviewWindow();
const summary = ref<DailySummaryView | null>(null);
const loadError = ref("");

/** 装载指定日期（缺省重取当前日期）；调用方必须给得出日期才发请求 */
async function load(date?: string) {
  const target = date ?? summary.value?.date;
  if (!target) return;
  try {
    summary.value = await getDailySummary(target);
    loadError.value = "";
  } catch (err) {
    loadError.value = `今日总结加载失败：${planErrorMessage(err as { kind?: string })}`;
  }
}

/** 标题：今日 / 昨日（补登）/ 更早的按日期称呼（如"8月28日总结"） */
const titleLabel = computed(() => {
  const v = summary.value;
  if (!v) return "今日总结";
  if (v.is_today) return "今日总结";
  if (v.is_yesterday) return "昨日总结";
  const [, m, d] = v.date.split("-").map(Number);
  return `${m}月${d}日总结`;
});

/** 归属日文案（"8月24日 星期一"）——从服务端日期解析，避免本地时区漂移 */
const dateLabel = computed(() => {
  const raw = summary.value?.date;
  if (!raw) return "";
  const [y, m, d] = raw.split("-").map(Number);
  return new Date(y, m - 1, d).toLocaleDateString("zh-CN", {
    month: "long",
    day: "numeric",
    weekday: "long",
  });
});

/** 结转标注（工单 10 同款）：目标 ≠ 基准时透明标出；休息日无目标不加注 */
const carryText = computed(() => {
  const v = summary.value;
  return v && v.workday ? carryLabel(v.target_minutes, v.base_minutes) : "";
});

/** 有子目标任务的"推进内容"：已完成列表 + 最早未完成项（进行中） */
function completedOf(t: SummaryTask) {
  return t.subgoals.filter((s) => s.completed);
}

function doingOf(t: SummaryTask) {
  return t.subgoals.find((s) => !s.completed);
}

onMounted(async () => {
  // 预装载待弹/最近一次的总结：自动触发路径是「先发事件再 show 窗口」，窗口 webview
  // 尚未就绪时事件可能丢失——这里按触发状态自愈（读到即最新，无本地账可失步）
  try {
    const st = await getDailySummaryStatus();
    await load(st.due ?? st.last_shown ?? undefined);
  } catch {
    /* 浏览器直开 / 后端不可达：等 show 事件 */
  }
  // 自动触发与控制面板调出都走 show 事件带日期；进展事件到达重取（实时重算）
  await listen<{ date: string }>(SUMMARY_SHOW_EVENT, (e) => load(e.payload.date));
  await listen(SUMMARY_REFRESH_EVENT, () => void load());
  // 关闭 = 隐藏（与大小看板同一窗口关闭语义；完整语义归工单 14）
  await win.onCloseRequested(async (e) => {
    e.preventDefault();
    await win.hide();
  });
});
</script>

<style scoped>
.summary {
  --col-max: 564px; /* 内容列宽 = 默认 620 窗口 − 2×28 padding（tokens.css .content-col 消费） */
  display: flex;
  flex-direction: column;
  height: 100%;
  background: var(--bg-base);
}

/* ---- 头部：标题 + 归属日期（同大面板头部节奏）----
   窗口可最大化：线条（下边框）通栏，内容收进限宽居中列（全局 .content-col）——拉大不散架 */
.summary-header {
  padding: 24px 28px 16px;
  border-bottom: var(--border-default);
}

.header-inner {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
}

.title {
  margin: 0;
  font-size: 18px;
}

.summary-header .hint {
  margin: 4px 0 0;
}

.date {
  margin: 2px 0 0;
  color: var(--text-muted);
  font-size: 13px;
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}

/* 滚动归内容列 .content-col-scroll，本体只留框架（与大面板同款） */
.summary-body {
  flex: 1;
  min-height: 0;
  padding: 20px 28px;
}

.loading {
  text-align: center;
  margin-top: 40px;
}

.error {
  color: var(--color-danger);
}

/* ---- 总览行：核心数字一行排开，进度条独占一行（数字旁不挤压） ---- */
.overview {
  display: flex;
  align-items: baseline;
  flex-wrap: wrap;
  gap: 10px 28px;
  padding: 14px 16px;
  border: var(--border-default);
  border-radius: var(--radius-md);
  background: var(--bg-group);
}

.stat {
  display: inline-flex;
  align-items: baseline;
  gap: 6px;
}

.stat-icon {
  align-self: center;
  color: var(--text-muted);
}

.stat-icon.done {
  color: var(--color-done);
}

.stat-num {
  font-size: 20px;
  font-weight: 700;
  font-variant-numeric: tabular-nums;
}

.stat-unit {
  color: var(--text-secondary);
  font-size: 13px;
}

.carry-note {
  color: var(--text-muted);
  font-size: 12px;
}

.overview-bar {
  flex-basis: 100%;
}

.overtime-note {
  margin: 8px 2px 0;
}

/* ---- 更高优先级提示（软提示色，同大面板差额提示的语气） ---- */
.priority-hint {
  display: flex;
  align-items: center;
  gap: 6px;
  margin: 12px 2px 0;
  color: var(--color-progress);
  font-size: 13px;
}

/* ---- 空态 ---- */
.empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 8px;
  margin-top: 48px;
  color: var(--text-muted);
}

.empty p {
  margin: 0;
}

/* ---- 计划 section（第一层） ---- */
.plan {
  margin-top: 20px;
}

.plan-head {
  display: flex;
  align-items: center;
  gap: 8px;
  padding-bottom: 8px;
  border-bottom: var(--border-default);
}

.plan-flag {
  color: var(--primary);
}

.plan-name {
  font-weight: 600;
  font-size: 15px;
}

.plan-minutes {
  margin-left: auto;
  color: var(--text-secondary);
  font-size: 13px;
  font-variant-numeric: tabular-nums;
}

/* 已被抢占暂停标注（琥珀软提示，干了的活照常计入） */
.preempted-tag {
  padding: 1px 8px;
  border: 1px solid var(--color-progress);
  border-radius: var(--radius-sm);
  color: var(--color-progress);
  font-size: 12px;
  white-space: nowrap;
}

/* ---- 任务行（第二层）+ 推进内容（第三层） ---- */
.task {
  padding: 10px 2px 10px 23px;
}

.task + .task {
  border-top: 1px dashed var(--text-muted);
}

.task-row {
  display: flex;
  align-items: baseline;
  gap: 10px;
}

.task-name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.task-percent {
  color: var(--text-secondary);
  font-variant-numeric: tabular-nums;
}

.task-minutes {
  min-width: 64px;
  text-align: right;
  color: var(--text-muted);
  font-size: 13px;
  font-variant-numeric: tabular-nums;
}

.task-detail {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 4px 12px;
  margin-top: 6px;
}

/* 子目标完成列表 / 进行中（勾选语言与小看板一致） */
.sg {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  font-size: 13px;
}

.sg.done {
  color: var(--color-done);
}

.sg.doing {
  color: var(--text-secondary);
}

/* 无子目标任务：线条风格百分比进度条 */
.task-bar {
  flex: 1;
}
</style>
