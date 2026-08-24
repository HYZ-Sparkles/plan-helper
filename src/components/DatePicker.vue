<template>
  <!--
    轻量日期选择（自制月历弹层，零依赖）：文本框可直接输入（失焦时归一/回退），
    点日历图标弹月历点选。值恒为 "" 或合法 YYYY-MM-DD——组件自己保证合法性，
    调用方（CreationUI 截止日期、工单 13 设置-日期例外）无需再校验。
  -->
  <div ref="root" class="date-picker">
    <span class="box">
      <input
        class="input"
        :value="modelValue"
        placeholder="选填 · 如 2026-12-20"
        @input="onType"
        @blur="onBlur"
        @keydown.esc="open = false"
      />
      <button v-if="modelValue" type="button" class="icon-btn" title="清除日期" @click="clear">
        <PhX :size="14" />
      </button>
      <button
        type="button"
        class="icon-btn"
        :class="{ active: open }"
        title="选择日期"
        @click="toggle"
      >
        <PhCalendarBlank :size="16" />
      </button>
    </span>

    <div v-if="open" class="popover">
      <header class="cal-head">
        <button type="button" class="icon-btn" title="上个月" @click="shiftMonth(-1)">
          <PhCaretLeft :size="14" />
        </button>
        <span class="cal-title">{{ viewMonth.getFullYear() }}年{{ viewMonth.getMonth() + 1 }}月</span>
        <button type="button" class="icon-btn" title="下个月" @click="shiftMonth(1)">
          <PhCaretRight :size="14" />
        </button>
      </header>

      <div class="cal-grid">
        <span v-for="w in weekdays" :key="w" class="cal-cell weekday">{{ w }}</span>
      </div>
      <div class="cal-grid">
        <button
          v-for="cell in grid"
          :key="cell.keyStr"
          type="button"
          class="cal-cell day"
          :class="{ dim: cell.other, today: cell.today, selected: cell.selected }"
          @click="pick(cell)"
        >
          {{ cell.day }}
        </button>
      </div>

      <footer class="cal-foot">
        <button type="button" class="link-btn" @click="pickToday">今天</button>
        <button type="button" class="link-btn" @click="clear(); open = false">清除</button>
      </footer>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from "vue";
import { PhCalendarBlank, PhCaretLeft, PhCaretRight, PhX } from "@phosphor-icons/vue";
import { normalizeDateString } from "../lib/validation";

const props = defineProps<{ modelValue: string }>();
const emit = defineEmits<{ "update:modelValue": [value: string] }>();

/** 弹层开合；打开时视图月份跳到已选日期（无则当月） */
const open = ref(false);
const root = ref<HTMLElement>();
const viewMonth = ref(startOfMonth(new Date()));

/** 列头周一在前（CONTEXT 工作日约定周一=1） */
const weekdays = ["一", "二", "三", "四", "五", "六", "日"];

function pad(n: number): string {
  return String(n).padStart(2, "0");
}
function keyOf(d: Date): string {
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}
function startOfMonth(d: Date): Date {
  return new Date(d.getFullYear(), d.getMonth(), 1);
}
function shiftMonth(n: number) {
  const next = new Date(viewMonth.value);
  next.setMonth(next.getMonth() + n);
  viewMonth.value = next;
}

/** 42 格月网格：首行补上月尾、末行补下月头，保证对齐 7 列 × 6 行 */
const grid = computed(() => {
  const first = viewMonth.value;
  const lead = (first.getDay() + 6) % 7; // 周日 getDay()=0 → 折算成第 7 列
  const start = new Date(first);
  start.setDate(first.getDate() - lead);
  const todayKey = keyOf(new Date());
  return Array.from({ length: 42 }, (_, i) => {
    const d = new Date(start);
    d.setDate(start.getDate() + i);
    return {
      keyStr: keyOf(d),
      day: d.getDate(),
      other: d.getMonth() !== first.getMonth(),
      today: keyOf(d) === todayKey,
      selected: keyOf(d) === props.modelValue,
    };
  });
});

function pick(cell: (typeof grid.value)[number]) {
  emit("update:modelValue", cell.keyStr);
  open.value = false;
}
function pickToday() {
  const now = new Date();
  viewMonth.value = startOfMonth(now);
  emit("update:modelValue", keyOf(now));
  open.value = false;
}
function clear() {
  emit("update:modelValue", "");
}

function toggle() {
  open.value = !open.value;
}

/** 最近一个合法值（含空串）——失焦时非法输入回退到它，保证表单值永不持有垃圾 */
const lastValid = ref(normalizeDateString(props.modelValue) ?? "");
watch(
  () => props.modelValue,
  (v) => {
    const n = normalizeDateString(v);
    if (n != null) lastValid.value = n;
  },
);

/** 手动输入实时透传（中间态可能是垃圾，由失焦归一/回退收口） */
function onType(e: Event) {
  emit("update:modelValue", (e.target as HTMLInputElement).value);
}
/** 失焦收口：空 = 清除；合法写法（2026-12-20 / 2026/12/20 / 20261220）归一；非法整体回退最近合法值 */
function onBlur(e: FocusEvent) {
  const input = e.target as HTMLInputElement;
  const raw = input.value.trim();
  if (raw === "") {
    emit("update:modelValue", "");
    input.value = "";
    return;
  }
  const normalized = normalizeDateString(raw);
  if (normalized != null) {
    emit("update:modelValue", normalized);
  } else {
    // 非法输入不落地：表单值与显示一并回退（此前只回退显示、表单值仍留垃圾会被提交）
    emit("update:modelValue", lastValid.value);
    input.value = lastValid.value;
  }
}

/** 点击组件外部关闭弹层 */
function onDocMouseDown(e: MouseEvent) {
  if (!root.value?.contains(e.target as Node)) open.value = false;
}
watch(open, (v) => {
  if (v) {
    // 锚点先归一：表单值可能是打字中间态垃圾，直接构造 Date 会得到 Invalid → 月历 NaN
    const valid = normalizeDateString(props.modelValue);
    viewMonth.value = startOfMonth(valid ? new Date(`${valid}T00:00:00`) : new Date());
    document.addEventListener("mousedown", onDocMouseDown);
  } else {
    document.removeEventListener("mousedown", onDocMouseDown);
  }
});
onBeforeUnmount(() => document.removeEventListener("mousedown", onDocMouseDown));
</script>

<style scoped>
.date-picker {
  position: relative;
  display: inline-flex;
}

.box {
  display: inline-flex;
  align-items: center;
  gap: 2px;
}

.box .input {
  min-width: 168px;
  padding-right: 6px;
}

/* 图标按钮形态走 tokens.css 全局 .icon-btn；这里只补日历弹层开关的激活态 */
.icon-btn.active {
  color: var(--primary);
  background: var(--bg-accent-group);
}

/* 弹层：输入框下方浮出（父容器无 overflow 裁剪，position: relative 锚定） */
.popover {
  position: absolute;
  top: calc(100% + 6px);
  left: 0;
  z-index: 50;
  width: 268px;
  padding: 10px;
  border: var(--border-default);
  border-radius: var(--radius-md);
  background: var(--surface);
  box-shadow: var(--shadow-lg);
}

.cal-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 6px;
}

.cal-title {
  font-weight: 600;
  font-size: 14px;
}

.cal-grid {
  display: grid;
  grid-template-columns: repeat(7, 1fr);
}

.cal-cell {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  height: 32px;
  border: none;
  background: transparent;
  font: inherit;
  font-size: 13px;
  color: var(--text-primary);
  cursor: pointer;
}

.cal-cell.weekday {
  color: var(--text-muted);
  font-size: 12px;
  cursor: default;
}

.cal-cell.day:hover {
  background: var(--bg-group);
  border-radius: var(--radius-sm);
}

.cal-cell.dim {
  color: var(--text-muted);
  opacity: 0.55;
}

.cal-cell.today {
  box-shadow: inset 0 0 0 1px var(--primary);
  border-radius: var(--radius-sm);
}

.cal-cell.selected {
  background: var(--primary);
  color: var(--bg-base);
  border-radius: var(--radius-sm);
}

.cal-foot {
  display: flex;
  justify-content: space-between;
  margin-top: 6px;
  padding-top: 8px;
  border-top: var(--border-default);
}

/* 文字链接按钮走 tokens.css 全局 .link-btn */
</style>
