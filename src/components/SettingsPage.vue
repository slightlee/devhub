<script setup lang="ts">
// 设置页：管理通用选项、代理配置、日志操作与关于信息展示。
import { ref } from "vue";
import type { SettingsState } from "../types/models";

interface Props {
  settings: SettingsState;
  logsDir: string;
  appVersion: string;
}

interface Emits {
  (event: "update:settings", value: SettingsState): void;
  (event: "clear-logs"): void;
  (event: "copy-path", payload: { path: string; label: string }): void;
  (event: "open-logs-dir"): void;
}

const props = defineProps<Props>();
const emit = defineEmits<Emits>();

const updateField = <K extends keyof SettingsState>(key: K, value: SettingsState[K]) => {
  emit("update:settings", { ...props.settings, [key]: value });
};

const activeTab = ref<"general" | "proxy" | "about">("general");
</script>

<template>
  <div class="settings">
    <div class="settings-tabs">
      <button
        class="settings-tab"
        :class="{ 'is-active': activeTab === 'general' }"
        type="button"
        @click="activeTab = 'general'"
      >
        通用
      </button>
      <button
        class="settings-tab"
        :class="{ 'is-active': activeTab === 'proxy' }"
        type="button"
        @click="activeTab = 'proxy'"
      >
        代理
      </button>
      <button
        class="settings-tab"
        :class="{ 'is-active': activeTab === 'about' }"
        type="button"
        @click="activeTab = 'about'"
      >
        关于
      </button>
    </div>

    <div class="settings-group" v-if="activeTab === 'general'">
      <section class="panel">
        <div class="panel-head">
          <div>
            <h3>启动与刷新</h3>
            <p>启动阶段的状态同步策略</p>
          </div>
        </div>
        <div class="settings-rows">
          <div class="settings-row">
            <div class="settings-row-main">
              <div class="settings-row-title">启动时自动刷新状态</div>
              <div class="settings-row-sub">启动后自动同步工具状态</div>
            </div>
            <div class="settings-row-side">
              <button
                class="toggle"
                :class="{ on: settings.autoRefreshOnLaunch }"
                type="button"
                @click="updateField('autoRefreshOnLaunch', !settings.autoRefreshOnLaunch)"
                :aria-pressed="settings.autoRefreshOnLaunch"
              >
                <span class="toggle-knob"></span>
              </button>
            </div>
          </div>
        </div>
      </section>

      <section class="panel">
        <div class="panel-head">
          <div>
            <h3>日志管理</h3>
            <p>日志落盘与清理策略</p>
          </div>
        </div>
        <div class="settings-rows">
          <div class="settings-row">
            <div class="settings-row-main">
              <div class="settings-row-title">启用日志落盘</div>
              <div class="settings-row-sub">记录安装/更新日志，默认保留 7 天</div>
            </div>
            <div class="settings-row-side">
              <button
                class="toggle"
                :class="{ on: settings.logPersistenceEnabled }"
                type="button"
                @click="updateField('logPersistenceEnabled', !settings.logPersistenceEnabled)"
                :aria-pressed="settings.logPersistenceEnabled"
              >
                <span class="toggle-knob"></span>
              </button>
            </div>
          </div>
          <div class="settings-row">
            <div class="settings-row-main">
              <div class="settings-row-title">日志目录</div>
              <div class="settings-row-sub">保存完整安装与更新日志</div>
            </div>
            <div class="settings-row-side">
              <input class="row-input" type="text" :value="logsDir || '--'" disabled />
              <button
                class="btn btn-ghost"
                type="button"
                :disabled="!logsDir"
                @click="emit('copy-path', { path: logsDir, label: '日志目录' })"
              >
                复制
              </button>
              <button class="btn btn-ghost" type="button" :disabled="!logsDir" @click="emit('open-logs-dir')">
                打开
              </button>
            </div>
          </div>
          <div class="settings-row">
            <div class="settings-row-main">
              <div class="settings-row-title">保留周期</div>
              <div class="settings-row-sub">按天数自动清理历史日志</div>
            </div>
            <div class="settings-row-side">
              <div class="settings-row-value">{{ settings.logRetentionDays }} 天</div>
            </div>
          </div>
          <div class="settings-row">
            <div class="settings-row-main">
              <div class="settings-row-title">清理日志</div>
              <div class="settings-row-sub">删除已落盘日志文件</div>
            </div>
            <div class="settings-row-side">
              <button class="btn btn-danger" type="button" @click="emit('clear-logs')">删除日志文件</button>
            </div>
          </div>
        </div>
      </section>
    </div>

    <div class="settings-group" v-else-if="activeTab === 'proxy'">
      <section class="panel">
        <div class="panel-head">
          <div>
            <h3>代理</h3>
            <p>仅影响安装与更新命令执行</p>
          </div>
        </div>
        <div class="form">
          <div class="toggle-row">
            <div>
              <div class="toggle-title">启用代理</div>
              <div class="toggle-sub">启用后使用代理地址</div>
            </div>
            <button
              class="toggle"
              :class="{ on: settings.proxyEnabled }"
              type="button"
              @click="updateField('proxyEnabled', !settings.proxyEnabled)"
              :aria-pressed="settings.proxyEnabled"
            >
              <span class="toggle-knob"></span>
            </button>
          </div>
          <label class="field">
            <span>代理地址</span>
            <input
              type="text"
              placeholder="http://127.0.0.1:7890"
              :value="settings.proxyUrl"
              :disabled="!settings.proxyEnabled"
              @input="updateField('proxyUrl', ($event.target as HTMLInputElement).value)"
            />
          </label>
        </div>
      </section>
    </div>

    <div class="settings-group" v-else>
      <section class="panel">
        <div class="panel-head">
          <div>
            <h3>关于</h3>
            <p>版本与数据路径信息</p>
          </div>
        </div>
        <div class="about-list">
          <div class="about-item">
            <span>应用版本</span>
            <span class="about-value">{{ appVersion === '--' ? '--' : `v${appVersion}` }}</span>
          </div>
          <div class="about-item">
            <span>配置目录</span>
            <span class="about-value">~/.devhub</span>
          </div>
        </div>
        <div class="about-actions">
          <button class="btn btn-ghost" type="button" disabled>检查更新</button>
        </div>
      </section>
    </div>
  </div>
</template>
