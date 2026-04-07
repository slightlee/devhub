// useSettingsState 行为测试：覆盖设置持久化队列与代理地址防抖保存。
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useSettingsState } from "../src/composables/useSettingsState";

const { invokeMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: invokeMock,
}));

describe("useSettingsState", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("应加载设置与日志目录并保存设置", async () => {
    const logs: Array<{ message: string; status?: string }> = [];
    const addLog = (message: string, status?: string) => logs.push({ message, status });

    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === "get_settings") {
        return {
          autoRefreshOnLaunch: false,
          proxyEnabled: true,
          proxyUrl: "http://127.0.0.1:7890",
          logPersistenceEnabled: true,
          logRetentionDays: 14,
        };
      }
      if (cmd === "get_logs_dir") {
        return "/tmp/devhub/logs";
      }
      return undefined;
    });

    const state = useSettingsState(addLog);
    await state.loadSettings();
    await state.loadLogsDir();

    expect(state.settings.value.proxyEnabled).toBe(true);
    expect(state.settings.value.logRetentionDays).toBe(14);
    expect(state.logsDir.value).toBe("/tmp/devhub/logs");

    const next = { ...state.settings.value, proxyEnabled: false };
    state.updateSettings(next);
    await Promise.resolve();

    expect(state.settings.value.proxyEnabled).toBe(false);
    expect(invokeMock).toHaveBeenCalledWith("save_settings", { settings: next });
    expect(logs).toHaveLength(0);
  });

  it("应串行保存设置，避免并发写入乱序", async () => {
    const addLog = vi.fn();
    let resolveFirstSave: (() => void) | null = null;

    invokeMock.mockImplementation((cmd: string) => {
      if (cmd !== "save_settings") return undefined;
      if (!resolveFirstSave) {
        return new Promise<void>((resolve) => {
          resolveFirstSave = resolve;
        });
      }
      return Promise.resolve();
    });

    const state = useSettingsState(addLog);
    const first = { ...state.settings.value, proxyEnabled: true };
    const second = { ...first, logRetentionDays: 14 };

    state.updateSettings(first);
    state.updateSettings(second);
    await Promise.resolve();

    expect(invokeMock).toHaveBeenCalledTimes(1);
    expect(invokeMock).toHaveBeenNthCalledWith(1, "save_settings", { settings: first });

    resolveFirstSave?.();
    await vi.waitFor(() => {
      expect(invokeMock).toHaveBeenCalledTimes(2);
    });
    expect(invokeMock).toHaveBeenNthCalledWith(2, "save_settings", { settings: second });
  });

  it("应对仅代理地址变更进行防抖保存", async () => {
    const addLog = vi.fn();
    invokeMock.mockResolvedValue(undefined);

    const state = useSettingsState(addLog);
    const base = state.settings.value;

    state.updateSettings({ ...base, proxyUrl: "http://127.0.0.1:7890" });
    state.updateSettings({ ...base, proxyUrl: "http://127.0.0.1:7891" });

    expect(invokeMock).not.toHaveBeenCalledWith("save_settings", expect.anything());

    vi.advanceTimersByTime(399);
    await Promise.resolve();
    expect(invokeMock).not.toHaveBeenCalledWith("save_settings", expect.anything());

    vi.advanceTimersByTime(1);
    await Promise.resolve();

    expect(invokeMock).toHaveBeenCalledWith("save_settings", {
      settings: { ...base, proxyUrl: "http://127.0.0.1:7891" },
    });
  });
});
