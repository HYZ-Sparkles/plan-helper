<template>
  <!--
    计划管理（工单 03/05：状态筛选栏 + 单列表 + 指针拖拽手动排序）。
    筛选项六项各带数量，默认「全部」；点击卡片进入计划详情页。
    拖拽（CONTEXT PlanOrdering 手动覆盖）：手柄按下跟随指针——半透明 + 顶部 3px 主色条、
    接近目标槽位 ±20px 磁吸对齐、其余卡让位；松手持久化，手动序优先于默认排序。
  -->
  <section class="plans-col">
    <h2 class="page-title">计划管理</h2>

    <div class="filter-bar">
      <button
        v-for="f in filters"
        :key="f.key"
        type="button"
        class="filter"
        :class="{ active: activeFilter === f.key }"
        @click="activeFilter = f.key"
      >
        {{ f.label }}
        <span class="count">{{ countOf(f) }}</span>
      </button>
    </div>

    <p v-if="loadError" class="load-error">{{ loadError }}</p>

    <div v-else-if="plans.length === 0" class="empty">
      <p>还没有计划。立下第一个目标，让 Oreo 陪你完成。</p>
      <RouterLink to="/control-panel/create" class="primary-btn">创建第一个计划</RouterLink>
    </div>

    <div v-else class="plan-list" :class="{ 'drag-live': drag.active }" ref="listEl">
      <div
        v-for="(p, i) in shownPlans"
        :key="p.id"
        class="plan-row"
        :class="{ dragging: drag.active && drag.index === i }"
        :style="rowStyle(i)"
      >
        <RouterLink
          class="plan-card"
          :class="`edge-${p.priority}`"
          :to="`/control-panel/plans/${p.id}`"
        >
          <!-- 拖拽手柄：指针事件起拖；click 阻止冒泡避免误触进详情 -->
          <span
            class="drag-handle"
            title="拖动调整顺序"
            @pointerdown="onPointerDown(i, $event)"
            @click.stop.prevent
          >
            <PhDotsSixVertical :size="16" />
          </span>
          <div class="plan-main">
            <div class="plan-title-row">
              <span class="plan-name">{{ p.name }}</span>
              <PriorityLabel :priority="p.priority" />
              <StatusBadge :status="p.status" />
            </div>
            <!-- 简述与名称相同（留空回退的产物）时不重复显示 -->
            <p v-if="p.summary && p.summary !== p.name" class="plan-summary">{{ p.summary }}</p>
            <p class="plan-meta">
              {{ p.tasks.length }} 个任务 · 合计 {{ totalHours(p) }} 小时
              <template v-if="p.due_date"> · 截止 {{ p.due_date }}</template>
            </p>
          </div>
        </RouterLink>
      </div>
      <p v-if="shownPlans.length === 0" class="hint">此分类暂无计划</p>
      <p v-if="reorderError" class="load-error">{{ reorderError }}</p>
    </div>
  </section>
</template>

<script setup lang="ts">
import { computed, onMounted, reactive, ref } from "vue";
import { PhDotsSixVertical } from "@phosphor-icons/vue";
import PriorityLabel from "../components/PriorityLabel.vue";
import StatusBadge from "../components/StatusBadge.vue";
import { listPlans, setPlanOrder, type PlanStatus, type PlanView } from "../lib/api";
import { hoursFromMinutes, statusLabel } from "../lib/labels";

/** 筛选项：key + 文案 + 命中状态集（「全部」不过滤）；五个状态都是一等公民 */
const filters: { key: string; label: string; statuses: PlanStatus[] | null }[] = [
  { key: "all", label: "全部", statuses: null },
  ...(["NotStarted", "Active", "Paused", "Completed", "Abandoned"] as PlanStatus[]).map(
    (s) => ({ key: s, label: statusLabel[s], statuses: [s] }),
  ),
];

const activeFilter = ref("all");
const plans = ref<PlanView[]>([]);
const loadError = ref("");
/** 手动排序持久化失败的提示（不阻断浏览，服务端顺序仍是真相） */
const reorderError = ref("");

onMounted(async () => {
  try {
    plans.value = await listPlans();
  } catch (err) {
    loadError.value = `计划列表加载失败：${err}`;
  }
});

/** 筛选项是否命中某计划（「全部」statuses 为 null 不过滤）；列表与数量统计共用 */
function matches(f: (typeof filters)[number], p: PlanView): boolean {
  return f.statuses == null || f.statuses.includes(p.status);
}

/** 当前筛选下的列表（顺序 = 全量顺序过滤，不单独重排） */
const shownPlans = computed(() => {
  const f = filters.find((x) => x.key === activeFilter.value)!;
  return plans.value.filter((p) => matches(f, p));
});

function countOf(f: (typeof filters)[number]) {
  return plans.value.filter((p) => matches(f, p)).length;
}

/** 任务预计耗时合计（小时） */
function totalHours(p: PlanView) {
  return hoursFromMinutes(p.tasks.reduce((sum, t) => sum + (t.estimated_minutes ?? 0), 0));
}

/* ---- 指针拖拽手动排序 ---- */

const listEl = ref<HTMLElement>();
/** 拖拽会话：index = 拖的行、active = 已越过起拖阈值、dy = 指针位移、drop = 预览目标槽位 */
const drag = reactive({ index: -1, active: false, dy: 0, drop: -1 });
/** 起拖时快照：各行中心（视口坐标）与高度；槽位几何在拖拽中不变（列表不重排到松手才落） */
let slotCenters: number[] = [];
let rowHeight = 0;
let rowGap = 0;
let startY = 0;
/** 起拖阈值：位移超过它才算拖拽（否则视为手柄上的普通点击，不排序不导航） */
const DRAG_THRESHOLD = 4;
/** 磁吸半径：拖动卡中心距目标槽中心 ≤ 该值时自动对齐（CONTEXT PlanOrdering ±20px） */
const SNAP_RADIUS = 20;

/** 手柄按下：快照各行几何（视口坐标）并捕获指针——后续 move/up 都回到手柄，出窗口也不丢 */
function onPointerDown(i: number, e: PointerEvent) {
  if (e.button !== 0 || !listEl.value) return;
  const rows = Array.from(listEl.value.children).filter((el) => el.classList.contains("plan-row"));
  const rects = rows.map((el) => (el as HTMLElement).getBoundingClientRect());
  slotCenters = rects.map((r) => r.top + r.height / 2);
  rowHeight = rects[i].height;
  rowGap = rects.length > 1 ? rects[1].top - (rects[0].top + rects[0].height) : 0;
  startY = e.clientY;
  drag.index = i;
  drag.drop = i;
  (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  (e.currentTarget as HTMLElement).addEventListener("pointermove", onPointerMove);
  (e.currentTarget as HTMLElement).addEventListener("pointerup", onPointerUp);
  (e.currentTarget as HTMLElement).addEventListener("pointercancel", onPointerUp);
}

/** 指针移动：越过阈值才真正起拖；拖动卡当前中心 → 最近槽位即预览目标（让位随之），≤20px 时磁吸对齐 */
function onPointerMove(e: PointerEvent) {
  drag.dy = e.clientY - startY;
  if (!drag.active && Math.abs(drag.dy) <= DRAG_THRESHOLD) return;
  drag.active = true;
  // 拖动卡当前中心 → 最近槽位即预览目标（让位随之）；距槽中心 ≤20px 时磁吸对齐
  const centerNow = slotCenters[drag.index] + drag.dy;
  let nearest = 0;
  for (let i = 1; i < slotCenters.length; i++) {
    if (Math.abs(centerNow - slotCenters[i]) < Math.abs(centerNow - slotCenters[nearest])) nearest = i;
  }
  drag.drop = nearest;
}

/** 指针抬起：摘监听、重置会话；真拖过且槽位变了才持久化新顺序 */
async function onPointerUp(e: PointerEvent) {
  const handle = e.currentTarget as HTMLElement;
  handle.removeEventListener("pointermove", onPointerMove);
  handle.removeEventListener("pointerup", onPointerUp);
  handle.removeEventListener("pointercancel", onPointerUp);
  const { index, drop, active } = drag;
  drag.index = -1;
  drag.drop = -1;
  drag.active = false;
  drag.dy = 0;
  if (!active || index === drop) return;
  await persistMove(index, drop);
}

/** 过滤视图内 from → to 的移动并回全量顺序：可见项占据全量中的若干槽位，新顺序稳定填回这些槽 */
async function persistMove(from: number, to: number) {
  const visible = [...shownPlans.value];
  const [moved] = visible.splice(from, 1);
  visible.splice(to, 0, moved);
  const visibleIds = new Set(visible.map((p) => p.id));
  let k = 0;
  const merged = plans.value.map((p) => (visibleIds.has(p.id) ? visible[k++]! : p));
  plans.value = merged; // 乐观更新，失败回滚重取
  reorderError.value = "";
  try {
    await setPlanOrder(merged.map((p) => p.id));
  } catch (err) {
    reorderError.value = `顺序保存失败：${err}`;
    try {
      plans.value = await listPlans();
    } catch {
      /* 列表本身还在，保序回滚即可 */
    }
  }
}

/** 行的拖拽视觉：被拖行跟随指针（磁吸时对齐槽位）、其余行让位平移 */
function rowStyle(i: number) {
  if (!drag.active || drag.index < 0) return undefined;
  if (i === drag.index) {
    const snapped = slotCenters[drag.drop] - slotCenters[drag.index];
    const centerNow = slotCenters[drag.index] + drag.dy;
    const translate = Math.abs(centerNow - slotCenters[drag.drop]) <= SNAP_RADIUS ? snapped : drag.dy;
    return { transform: `translateY(${translate}px)`, zIndex: 10 };
  }
  const shift = rowHeight + rowGap;
  if (drag.index < drag.drop && i > drag.index && i <= drag.drop) {
    return { transform: `translateY(${-shift}px)` };
  }
  if (drag.drop < drag.index && i >= drag.drop && i < drag.index) {
    return { transform: `translateY(${shift}px)` };
  }
  return undefined;
}
</script>

<style scoped>
/* 内容列：限宽 + 居中（计划卡内容行不长，全宽拉伸反而显得空），窄窗口自动收缩 */
.plans-col {
  max-width: 840px;
  width: 100%;
  margin: 0 auto;
}

.filter-bar {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  margin-bottom: 16px;
}

.filter {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 5px 12px;
  border: var(--border-default);
  border-radius: var(--radius-sm);
  background: var(--surface);
  font: inherit;
  font-size: 13px;
  color: var(--text-secondary);
  cursor: pointer;
}

.filter.active {
  border-color: var(--primary);
  background: var(--bg-accent-group);
  color: var(--text-primary);
}

.count {
  min-width: 20px;
  padding: 0 6px;
  border-radius: var(--radius-sm);
  background: var(--bg-group);
  border: var(--border-default);
  font-size: 12px;
  line-height: 18px;
  text-align: center;
  color: var(--text-muted);
}

.filter.active .count {
  background: var(--surface);
  border-color: var(--primary);
  color: var(--text-primary);
}

.empty {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 12px;
  padding: 32px 24px;
  border: var(--border-default);
  border-radius: var(--radius-md);
  background: var(--bg-group);
  color: var(--text-secondary);
}

.load-error {
  color: var(--color-danger);
}

.plan-list {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

/* 拖拽进行中：整列表禁选中，行内平移有过渡（被拖行除外——它跟指针，不能滞后） */
.plan-list.drag-live {
  user-select: none;
}

.plan-row {
  position: relative;
  transition: transform 0.15s;
}

.plan-row.dragging {
  transition: none;
  z-index: 10;
}

/* 被拖卡：半透明 + 顶部 3px 主色条（CONTEXT PlanOrdering 拖拽视觉） */
.plan-row.dragging .plan-card {
  opacity: 0.5;
  box-shadow: inset 0 3px 0 var(--primary);
}

.drag-handle {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  align-self: stretch;
  width: 32px;
  margin-left: 2px;
  border-radius: var(--radius-sm);
  color: var(--text-muted);
  cursor: grab;
  touch-action: none; /* 指针拖拽不被触摸滚动手势抢走 */
}

.drag-handle:hover {
  color: var(--primary);
  background: var(--bg-accent-group);
}

.plan-list.drag-live .drag-handle {
  cursor: grabbing;
}

.plan-card {
  display: flex;
  border: var(--border-default);
  border-radius: var(--radius-md);
  background: var(--surface);
  overflow: hidden;
  text-decoration: none;
  color: inherit;
  transition: border-color 0.15s;
}

.plan-card:hover {
  border-color: var(--primary);
}

/* PriorityVisuals：列表项左侧 4px 优先级边条 */
.edge-High {
  border-left: 4px solid var(--priority-high);
}

.edge-Medium {
  border-left: 4px solid var(--priority-medium);
}

.edge-Low {
  border-left: 4px solid var(--priority-low);
}

.plan-main {
  flex: 1;
  padding: 14px 16px;
}

.plan-title-row {
  display: flex;
  align-items: center;
  gap: 10px;
}

.plan-name {
  font-weight: 600;
  font-size: 15px;
}

.plan-summary {
  margin: 6px 0 4px;
  color: var(--text-secondary);
}

.plan-meta {
  margin: 0;
  color: var(--text-muted);
  font-size: 12px;
}
</style>
