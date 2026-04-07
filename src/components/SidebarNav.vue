<script setup lang="ts">
// 侧边导航栏：在模块间切换并高亮当前激活页面。
interface Emits {
  (event: "select", value: string): void;
}

interface Props {
  active: string;
}

defineProps<Props>();
const emit = defineEmits<Emits>();
</script>

<template>
  <aside class="sidebar">
    <button
      class="sidebar-brand"
      type="button"
      title="DevHub"
      data-tooltip="DevHub"
      aria-label="DevHub"
      @click="emit('select', 'dashboard')"
    >
      <span class="logo-mark" aria-hidden="true">
        <svg viewBox="0 0 24 24">
          <circle cx="12" cy="12" r="7.8" fill="none" stroke="currentColor" stroke-width="1.4" />
          <circle cx="12" cy="12" r="2.0" fill="currentColor" />
          <circle cx="18.2" cy="7.8" r="1.6" fill="currentColor" />
          <circle cx="12" cy="19.2" r="1.6" fill="currentColor" />
          <circle cx="5.8" cy="7.8" r="1.6" fill="currentColor" />
          <path
            d="M12 12L16.8 8.6M12 12L12 17.4M12 12L7.2 8.6"
            fill="none"
            stroke="currentColor"
            stroke-width="1.2"
            stroke-linecap="round"
          />
        </svg>
      </span>
      <div class="brand-text">
        <div class="brand-name">Dev</div>
        <div class="brand-sub">Hub</div>
      </div>
    </button>

    <nav class="sidebar-nav">
      <button
        class="nav-item"
        :class="{ active: active === 'dashboard' }"
        type="button"
        data-tooltip="主页"
        aria-label="主页"
        @click="emit('select', 'dashboard')"
      >
        <span class="nav-icon" aria-hidden="true">
          <svg viewBox="0 0 24 24">
            <path
              d="M4 10.5 12 4l8 6.5v9a1 1 0 0 1-1 1h-4.5a1 1 0 0 1-1-1v-4.5h-3v4.5a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1z"
              fill="none"
              stroke="currentColor"
              stroke-width="1.6"
              stroke-linecap="round"
              stroke-linejoin="round"
            />
          </svg>
        </span>
        <span class="nav-label">主页</span>
      </button>
      <button
        class="nav-item"
        :class="{ active: active === 'cli' }"
        type="button"
        data-tooltip="CLI 工具"
        aria-label="CLI 工具"
        @click="emit('select', 'cli')"
      >
        <span class="nav-icon" aria-hidden="true">
          <svg viewBox="0 0 24 24">
            <path
              d="M4 6h16M4 12h10M4 18h7M16 12l4 3-4 3"
              fill="none"
              stroke="currentColor"
              stroke-width="1.6"
              stroke-linecap="round"
              stroke-linejoin="round"
            />
          </svg>
        </span>
        <span class="nav-label">CLI 工具</span>
      </button>
      <button
        class="nav-item"
        :class="{ active: active === 'mcp' }"
        type="button"
        data-tooltip="MCP"
        aria-label="MCP"
        @click="emit('select', 'mcp')"
      >
        <span class="nav-icon" aria-hidden="true">
          <svg viewBox="0 0 24 24">
            <path
              d="M7 7h10a4 4 0 0 1 0 8H7a4 4 0 0 1 0-8z"
              fill="none"
              stroke="currentColor"
              stroke-width="1.6"
              stroke-linecap="round"
              stroke-linejoin="round"
            />
            <path
              d="M7 7V5m10 2V5M7 17v2m10-2v2"
              fill="none"
              stroke="currentColor"
              stroke-width="1.6"
              stroke-linecap="round"
            />
          </svg>
        </span>
        <span class="nav-label">MCP</span>
      </button>
      <button
        class="nav-item"
        :class="{ active: active === 'skills' }"
        type="button"
        data-tooltip="Skills"
        aria-label="Skills"
        @click="emit('select', 'skills')"
      >
        <span class="nav-icon" aria-hidden="true">
          <svg viewBox="0 0 24 24">
            <path
              d="M7 4h8l4 4v12a1 1 0 0 1-1 1H7a1 1 0 0 1-1-1V5a1 1 0 0 1 1-1z"
              fill="none"
              stroke="currentColor"
              stroke-width="1.6"
              stroke-linecap="round"
              stroke-linejoin="round"
            />
            <path
              d="M12 9v6m-3-3h6"
              fill="none"
              stroke="currentColor"
              stroke-width="1.6"
              stroke-linecap="round"
            />
          </svg>
        </span>
        <span class="nav-label">Skills</span>
      </button>
    </nav>

    <div class="sidebar-footer">
      <button
        class="nav-item"
        :class="{ active: active === 'settings' }"
        type="button"
        data-tooltip="设置"
        aria-label="设置"
        @click="emit('select', 'settings')"
      >
        <span class="nav-icon" aria-hidden="true">
          <svg viewBox="0 0 24 24">
            <path
              d="M12 8.5a3.5 3.5 0 1 0 0 7 3.5 3.5 0 0 0 0-7z"
              fill="none"
              stroke="currentColor"
              stroke-width="1.6"
              stroke-linecap="round"
              stroke-linejoin="round"
            />
            <path
              d="M4.5 12a7.5 7.5 0 0 1 .1-1.2l-1.9-1.1 1.6-2.8 2.2.6a7.7 7.7 0 0 1 2.1-1.2L9 3h6l.5 2.3a7.7 7.7 0 0 1 2.1 1.2l2.2-.6 1.6 2.8-1.9 1.1a7.5 7.5 0 0 1 0 2.4l1.9 1.1-1.6 2.8-2.2-.6a7.7 7.7 0 0 1-2.1 1.2L15 21h-6l-.5-2.3a7.7 7.7 0 0 1-2.1-1.2l-2.2.6-1.6-2.8 1.9-1.1A7.5 7.5 0 0 1 4.5 12z"
              fill="none"
              stroke="currentColor"
              stroke-width="1.6"
              stroke-linecap="round"
              stroke-linejoin="round"
            />
          </svg>
        </span>
        <span class="nav-label">设置</span>
      </button>
    </div>
  </aside>
</template>
