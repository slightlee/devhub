<script setup lang="ts">
// 占位页面组件：用于尚未开放模块的统一空状态展示。
interface Props {
  title: string;
  description: string;
}

defineProps<Props>();
</script>

<template>
  <section class="placeholder">
    <div class="panel">
      <div class="panel-head">
        <div>
          <h3>{{ title }}</h3>
          <p>{{ description }}</p>
        </div>
      </div>
      <div class="empty-state">模块建设中，后续版本将开放</div>
    </div>
  </section>
</template>
