<template>
  <!--
    设置页（工单 13）：每日工作时间 / 每周工作日 / 日内时间窗口 / 日期例外 / 均分窗口。
    保存走 save_settings（覆盖式）；生效时机由服务层裁决——延时字段（前三类）自下一个
    工作日生效并返回生效日，均分窗口与日期例外立即生效；保存后广播设置变更，
    桌宠重排两个定时触发、看板重取目标。
  -->
  <section>
    <div class="col">
    <h2 class="page-title">设置</h2>

    <template v-if="loaded">
      <div class="group">
        <h3 class="group-title"><PhBriefcase :size="16" /> 每日工作时间</h3>
        <div class="inline">
          <input
            v-model="dailyHours"
            class="input num"
            type="number"
            min="0.5"
            step="0.5"
            aria-label="每日工作时间（小时）"
          />
          <span class="unit">小时 / 天</span>
        </div>
        <p class="hint">决定每日推进目标；改动自下一个工作日生效</p>
      </div>

      <div class="group">
        <h3 class="group-title"><PhCalendarCheck :size="16" /> 每周工作日</h3>
        <div class="chips">
          <button
            v-for="(name, i) in WEEKDAY_NAMES"
            :key="i"
            type="button"
            class="chip"
            :class="{ on: workdays.includes(i + 1) }"
            @click="toggleWorkday(i + 1)"
          >
            周{{ name }}
          </button>
        </div>
        <p class="hint">改动自下一个工作日生效</p>
      </div>

      <div class="group">
        <h3 class="group-title"><PhClock :size="16" /> 日内时间窗口</h3>
        <div v-for="(w, i) in windows" :key="i" class="inline window-row">
          <input v-model="w.start" class="input" type="time" aria-label="窗口开始" />
          <span class="dash">–</span>
          <input v-model="w.end" class="input" type="time" aria-label="窗口结束" />
          <button
            type="button"
            class="icon-btn danger"
            title="删除这段窗口"
            @click="windows.splice(i, 1)"
          >
            <PhX :size="14" />
          </button>
        </div>
        <button type="button" class="link-btn" @click="addWindow">
          <PhPlus :size="14" /> 添加时间段
        </button>
        <p class="hint">可多段（如 10:00–11:30 与 20:00–22:00）；结束早于开始即跨午夜（如 20:00–01:00）；保存时自动合并重叠或首尾相接的段；改动自下一个工作日生效</p>
      </div>

      <div class="group">
        <h3 class="group-title"><PhCalendarPlus :size="16" /> 日期例外</h3>
        <p v-if="overrides.length === 0" class="hint">暂无例外——按日期双向覆盖周循环（假期 / 调休）</p>
        <div v-for="(o, i) in overrides" :key="o.date" class="inline override-row">
          <span class="mono">{{ o.date }}</span>
          <span class="tag" :class="o.working ? 'work' : 'rest'">
            {{ o.working ? "这天工作" : "这天不工作" }}
          </span>
          <button type="button" class="icon-btn danger" title="删除这条例外" @click="overrides.splice(i, 1)">
            <PhX :size="14" />
          </button>
        </div>
        <div class="inline add-override">
          <DatePicker v-model="newOverrideDate" />
          <div class="chips">
            <button type="button" class="chip" :class="{ on: !newOverrideWorking }" @click="newOverrideWorking = false">
              不工作
            </button>
            <button type="button" class="chip" :class="{ on: newOverrideWorking }" @click="newOverrideWorking = true">
              工作
            </button>
          </div>
          <button type="button" class="ghost-btn" :disabled="!newOverrideDate" @click="addOverride">
            添加
          </button>
        </div>
        <p class="hint">提前标注将来的日期、立即生效（同一天重复添加会覆盖原标记）</p>
      </div>

      <div class="group">
        <h3 class="group-title"><PhArrowsClockwise :size="16" /> 均分窗口</h3>
        <div class="inline">
          <input
            v-model="smoothing"
            class="input num"
            type="number"
            min="1"
            step="1"
            aria-label="均分窗口（工作日数）"
          />
          <span class="unit">个工作日</span>
        </div>
        <p class="hint">每日差额向后续工作日均分的范围；改动立即生效</p>
      </div>

      <div class="actions">
        <button type="button" class="primary-btn" :disabled="saving" @click="save">
          {{ saving ? "保存中…" : "保存设置" }}
        </button>
        <span v-if="savedHint" class="saved">{{ savedHint }}</span>
        <span v-else-if="error" class="err">{{ error }}</span>
      </div>
    </template>
    <p v-else class="hint">正在读取设置……</p>
    </div>
  </section>
</template>

<script setup lang="ts">
import { onMounted, ref } from "vue";
import { emitTo } from "@tauri-apps/api/event";
import {
  PhArrowsClockwise,
  PhBriefcase,
  PhCalendarCheck,
  PhCalendarPlus,
  PhClock,
  PhPlus,
  PhX,
} from "@phosphor-icons/vue";
import { getAppState, saveSettings, type DateOverride, type Settings } from "../lib/api";
import { MAIN_BOARD_REFRESH_EVENT, MINI_BOARD_REFRESH_EVENT, SETTINGS_CHANGED_EVENT } from "../lib/events";
import { planErrorMessage } from "../lib/labels";
import { localToday } from "../lib/validation";
import DatePicker from "../components/DatePicker.vue";

/** 周一=1..周日=7 的展示名（周循环 chips 顺序） */
const WEEKDAY_NAMES = ["一", "二", "三", "四", "五", "六", "日"];

/** 表单本地态：loaded 前 null，装载自 get_app_state 的最新设置 */
const loaded = ref(false);
const dailyHours = ref("5");
const workdays = ref<number[]>([]);
/** 窗口行用 "HH:MM" 字符串承载（time input 原生值），保存时换算分钟 */
const windows = ref<{ start: string; end: string }[]>([]);
const overrides = ref<DateOverride[]>([]);
const smoothing = ref("7");
const newOverrideDate = ref("");
const newOverrideWorking = ref(false);

const saving = ref(false);
const error = ref("");
const savedHint = ref("");

/** 从服务端装载最新设置到表单（mount 首载与保存后回读共用——保存时服务端可能
 *  已合并时间窗口、或延时字段已过生效日，回读让编辑态始终所见即所存） */
async function reload() {
  const snap = await getAppState();
  const s = snap.settings;
  dailyHours.value = String(s.daily_minutes / 60);
  workdays.value = [...s.workdays];
  windows.value = s.time_windows.map((w) => ({
    start: minutesToTime(w.start_minute),
    end: minutesToTime(w.end_minute),
  }));
  overrides.value = [...s.date_overrides];
  smoothing.value = String(s.smoothing_workdays);
  loaded.value = true;
}

onMounted(reload);

/** 切换一枚工作日 chip（已在集合中则移除，否则加入） */
function toggleWorkday(day: number) {
  const at = workdays.value.indexOf(day);
  if (at >= 0) workdays.value.splice(at, 1);
  else workdays.value.push(day);
}

/** 追加一段空白窗口行（默认常规 09:00–18:00，用户再改） */
function addWindow() {
  windows.value.push({ start: "09:00", end: "18:00" });
}

/** 添加一条日期例外：同日重复添加 = 改标记（所见即所存）；清空输入待下一条 */
function addOverride() {
  if (!newOverrideDate.value) return;
  // 同一日期只保留一条：重复添加 = 改标记（所见即所存）
  const existing = overrides.value.find((o) => o.date === newOverrideDate.value);
  if (existing) existing.working = newOverrideWorking.value;
  else overrides.value.push({ date: newOverrideDate.value, working: newOverrideWorking.value });
  overrides.value.sort((a, b) => a.date.localeCompare(b.date));
  newOverrideDate.value = "";
}

/** "HH:MM" → 当日分钟数；非法返回 null（time input 的值恒合法，兜底手改） */
function timeToMinutes(t: string): number | null {
  const m = /^(\d{1,2}):(\d{2})$/.exec(t);
  return m ? Number(m[1]) * 60 + Number(m[2]) : null;
}

/** 当日分钟数 → "HH:MM"（time input 原生值） */
function minutesToTime(v: number): string {
  return `${String(Math.floor(v / 60)).padStart(2, "0")}:${String(v % 60).padStart(2, "0")}`;
}

/** 保存整份设置（生效时机权威在服务层）：成功后广播设置变更（桌宠重排定时触发）
 *  与看板重取（均分窗口改动立即改变当日调整后目标），并按返回生效日提示 */
async function save() {
  error.value = "";
  savedHint.value = "";
  const hours = Number(dailyHours.value);
  const smooth = Number(smoothing.value);
  if (!(hours > 0) || !(smooth >= 1)) {
    error.value = "每日工作时间与均分窗口须为正数";
    return;
  }
  const parsedWindows = [];
  for (const w of windows.value) {
    const start = timeToMinutes(w.start);
    const end = timeToMinutes(w.end);
    if (start == null || end == null) {
      error.value = "时间窗口格式无效";
      return;
    }
    if (start === end) {
      error.value = "时间窗口的起止不能相同";
      return;
    }
    parsedWindows.push({ start_minute: start, end_minute: end });
  }
  const settings: Settings = {
    daily_minutes: Math.round(hours * 60),
    workdays: [...workdays.value].sort((a, b) => a - b),
    time_windows: parsedWindows,
    smoothing_workdays: Math.round(smooth),
    date_overrides: overrides.value,
  };
  saving.value = true;
  try {
    const effective = await saveSettings(settings);
    // 桌宠重排两个定时触发；看板重取目标（均分窗口改动会改变当日调整后目标）
    await emitTo("pet", SETTINGS_CHANGED_EVENT);
    await emitTo("main-board", MAIN_BOARD_REFRESH_EVENT);
    await emitTo("mini-board", MINI_BOARD_REFRESH_EVENT);
    savedHint.value =
      effective > localToday()
        ? `已保存：工作时间类改动自 ${effective} 起生效，均分窗口与日期例外已生效`
        : "已保存，设置立即生效";
    await reload(); // 回读服务端设置：重叠/相接的时间窗口已在保存时合并，列表如实反映
  } catch (e) {
    error.value = planErrorMessage(e as { kind?: string; payload?: unknown });
  } finally {
    saving.value = false;
  }
}
</script>

<style scoped>
/* 整列居中（窗口最大化时内容不再靠左）；分组块随列宽 */
.col {
  max-width: 560px;
  margin: 0 auto;
}

.group {
  border: var(--border-default);
  border-radius: var(--radius-md);
  background: var(--bg-group);
  padding: 12px 16px;
  margin-bottom: 12px;
}

.group-title {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 14px;
  font-weight: 600;
  color: var(--text-primary);
  margin: 0 0 10px;
}

.inline {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}

.inline + .inline {
  margin-top: 8px;
}

.num {
  width: 90px;
}

.unit {
  color: var(--text-secondary);
  font-size: 13px;
}

.window-row input[type="time"] {
  width: 110px;
}

.dash {
  color: var(--text-muted);
}

.override-row .mono {
  font-variant-numeric: tabular-nums;
  color: var(--text-primary);
}

.tag {
  font-size: 12px;
  padding: 1px 8px;
  border-radius: var(--radius-sm);
  border: var(--border-default);
}

.tag.work {
  color: var(--color-done);
  border-color: var(--color-done);
  background: var(--bg-group);
}

.tag.rest {
  color: var(--text-secondary);
}

.add-override {
  margin-top: 10px;
  padding-top: 10px;
  border-top: var(--border-default);
}

.chips {
  display: flex;
  gap: 6px;
  flex-wrap: wrap;
}

.chip {
  padding: 4px 12px;
  font-size: 13px;
  border: var(--border-default);
  border-radius: var(--radius-sm);
  background: var(--bg-base);
  color: var(--text-secondary);
  cursor: pointer;
}

.chip.on {
  border-color: var(--primary);
  color: var(--primary);
  background: var(--bg-accent-group);
}

.chip:focus-visible {
  outline: 2px solid var(--primary);
  outline-offset: 1px;
}

.actions {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-top: 4px;
}

.saved {
  font-size: 13px;
  color: var(--color-done);
}

.err {
  font-size: 13px;
  color: var(--color-danger);
}
</style>
