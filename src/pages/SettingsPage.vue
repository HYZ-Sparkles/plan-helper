<template>
  <!--
    设置页壳（完整设置由工单 13 接管）。
    当前展示 get_app_state 返回的快照——验证 UI → Tauri command → 领域服务 → 返回
    这条接缝端到端贯通（工单 01 验收项），同时把 FirstRun 默认值可视化。
  -->
  <section>
    <h2 class="page-title">设置</h2>
    <div v-if="snapshot" class="snapshot">
      <div class="row">
        <span class="label">每日工作时间</span>
        <span>{{ snapshot.settings.daily_minutes / 60 }} 小时 / 天</span>
      </div>
      <div class="row">
        <span class="label">每周工作日</span>
        <span>{{ weekdayNames }}</span>
      </div>
      <div class="row">
        <span class="label">均分窗口</span>
        <span>{{ snapshot.settings.smoothing_workdays }} 个工作日</span>
      </div>
      <div class="row">
        <span class="label">服务端时间</span>
        <span>{{ snapshot.server_now.replace("T", " ").slice(0, 19) }}</span>
      </div>
    </div>
    <p v-else class="hint">正在读取应用状态……</p>
  </section>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { getAppState, type AppStateView } from "../lib/api";

const snapshot = ref<AppStateView | null>(null);

// 周一=1..周日=7 → 中文星期名
const weekdayNames = computed(() => {
  const names = ["一", "二", "三", "四", "五", "六", "日"];
  return (snapshot.value?.settings.workdays ?? []).map((d) => `周${names[d - 1]}`).join("、");
});

onMounted(async () => {
  snapshot.value = await getAppState();
});
</script>

<style scoped>
.snapshot {
  max-width: 420px;
  border: var(--border-default);
  border-radius: var(--radius-md);
  background: var(--bg-group);
  padding: 4px 16px;
}

.row {
  display: flex;
  justify-content: space-between;
  padding: 10px 0;
}

.row + .row {
  border-top: var(--border-default);
}

.label {
  color: var(--text-secondary);
}
</style>
