<template>
  <!-- 创建计划：CreationUI 的创建模式入口（表单本体在 CreationForm，与详情页编辑态共用） -->
  <section class="create-page">
    <h2 class="page-title">创建计划</h2>
    <CreationForm mode="create" @saved="router.push('/control-panel/plans')" />
  </section>
</template>

<script setup lang="ts">
import { useRouter } from "vue-router";
import CreationForm from "../components/CreationForm.vue";

const router = useRouter();
</script>

<style scoped>
.create-page {
  max-width: 720px;
}
</style>
