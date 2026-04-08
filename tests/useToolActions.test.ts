// useToolActions 行为回归测试：覆盖并发保护、事件桥接与关键动作分支。
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useToolActions } from "../src/composables/useToolActions";

const { invokeMock, listenMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  listenMock: vi.fn(),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: invokeMock,
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: listenMock,
}));

describe("useToolActions", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    listenMock.mockReset();
  });

  it("应在卸载确认时设置 Claude 的路径清理选项", async () => {
    const addLog = vi.fn();
    const onActionFailure = vi.fn();
    const state = useToolActions(addLog, onActionFailure);
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === "get_action_commands") {
        return {
          claude: {
            uninstall: "rm -f \"$HOME/.local/bin/claude\" && rm -rf \"$HOME/.local/share/claude\"",
          },
        };
      }
      return undefined;
    });
    await state.loadActionCommands();

    const claude = state.tools.value.find((tool) => tool.id === "claude");
    if (claude) {
      claude.supportsPathFix = true;
      claude.shellConfigFile = "~/.zshrc";
    }

    state.openConfirm("uninstall", "claude");

    expect(state.pendingAction.value).toEqual({ action: "uninstall", toolId: "claude" });
    expect(state.optionVisible.value).toBe(true);
    expect(state.optionChecked.value).toBe(false);
    expect(state.optionLabel.value).toContain("清理 PATH");
    expect(state.commandForConfirm.value).toContain("rm -f");
  });

  it("命令映射缺失时应展示兜底提示", () => {
    const addLog = vi.fn();
    const onActionFailure = vi.fn();
    const state = useToolActions(addLog, onActionFailure);

    state.openConfirm("install", "codex");

    expect(state.commandForConfirm.value).toBe("");
    expect(state.commandHintForConfirm.value).toContain("命令预览暂不可用");
  });

  it("命令映射加载失败时应保留已加载数据", async () => {
    const addLog = vi.fn();
    const onActionFailure = vi.fn();
    const state = useToolActions(addLog, onActionFailure);

    invokeMock
      .mockResolvedValueOnce({
        codex: { install: "npm i -g @openai/codex@latest" },
      })
      .mockRejectedValueOnce(new Error("network"));

    await state.loadActionCommands();
    state.openConfirm("install", "codex");
    expect(state.commandForConfirm.value).toContain("@openai/codex");

    await state.loadActionCommands();
    state.openConfirm("install", "codex");
    expect(state.commandForConfirm.value).toContain("@openai/codex");
  });

  it("并发检测时应以最后一次结果为准", async () => {
    const addLog = vi.fn();
    const onActionFailure = vi.fn();
    const state = useToolActions(addLog, onActionFailure);
    let resolveFirst: ((value: unknown) => void) | undefined;
    let resolveSecond: ((value: unknown) => void) | undefined;

    invokeMock.mockImplementation((cmd: string, payload?: { toolId?: string }) => {
      if (cmd !== "check_sources") {
        return undefined;
      }
      if (payload?.toolId === "claude") {
        return new Promise((resolve) => {
          resolveFirst = resolve;
        });
      }
      if (payload?.toolId === "codex") {
        return new Promise((resolve) => {
          resolveSecond = resolve;
        });
      }
      return undefined;
    });

    state.openConfirm("install", "claude");
    state.openConfirm("install", "codex");
    expect(state.isCheckingSources.value).toBe(true);
    expect(invokeMock).toHaveBeenCalledTimes(2);

    resolveSecond?.({
      overall: "ok",
      npm: "ok",
      claude: "unknown",
      checkedAt: 2,
    });
    await Promise.resolve();
    await Promise.resolve();

    expect(state.sourceStatus.value).toBe("ok");
    expect(state.isCheckingSources.value).toBe(false);

    resolveFirst?.({
      overall: "fail",
      npm: "unknown",
      claude: "fail",
      checkedAt: 1,
    });
    await Promise.resolve();
    await Promise.resolve();

    expect(state.sourceStatus.value).toBe("ok");
    expect(state.isCheckingSources.value).toBe(false);
  });

  it("版本刷新并发返回时应忽略过期结果", async () => {
    const addLog = vi.fn();
    const onActionFailure = vi.fn();
    const state = useToolActions(addLog, onActionFailure);
    let refreshCallCount = 0;
    let resolveFirst: ((value: unknown) => void) | undefined;
    let resolveSecond: ((value: unknown) => void) | undefined;

    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === "get_tools_state") {
        return Promise.resolve(state.tools.value.map((tool) => ({ ...tool })));
      }
      if (cmd === "refresh_latest_versions") {
        refreshCallCount += 1;
        if (refreshCallCount === 1) {
          return new Promise((resolve) => {
            resolveFirst = resolve;
          });
        }
        return new Promise((resolve) => {
          resolveSecond = resolve;
        });
      }
      return Promise.resolve(undefined);
    });

    await state.refreshTools();
    await state.refreshTools();

    const codex = state.tools.value.find((tool) => tool.id === "codex");
    expect(codex).toBeTruthy();

    resolveSecond?.([{ ...codex!, latestVersion: "v2.0.0" }]);
    await Promise.resolve();
    await Promise.resolve();
    expect(state.tools.value.find((tool) => tool.id === "codex")?.latestVersion).toBe("v2.0.0");

    resolveFirst?.([{ ...codex!, latestVersion: "v1.0.0" }]);
    await Promise.resolve();
    await Promise.resolve();
    expect(state.tools.value.find((tool) => tool.id === "codex")?.latestVersion).toBe("v2.0.0");
  });

  it("应注册四个工具事件监听器", async () => {
    listenMock.mockResolvedValue(() => undefined);
    const addLog = vi.fn();
    const onActionFailure = vi.fn();
    const state = useToolActions(addLog, onActionFailure);

    const unlisteners = await state.subscribeToolEvents();

    expect(listenMock).toHaveBeenCalledTimes(4);
    expect(unlisteners).toHaveLength(4);
  });

  it("应在收到失败动作结果事件时上报错误", async () => {
    const handlers = new Map<string, (event: { payload: unknown }) => void>();
    listenMock.mockImplementation(async (eventName: string, handler: (event: { payload: unknown }) => void) => {
      handlers.set(eventName, handler);
      return () => undefined;
    });
    const addLog = vi.fn();
    const onActionFailure = vi.fn();
    const state = useToolActions(addLog, onActionFailure);

    await state.subscribeToolEvents();
    handlers.get("tool-action-result")?.({
      payload: {
        timestamp: 123,
        toolId: "codex",
        action: "update",
        success: false,
        message: "命令失败：退出码 1",
      },
    });

    expect(onActionFailure).toHaveBeenCalledWith("命令失败：退出码 1", 123);
  });

  it("batch_update 部分失败时应提示告警而非整体失败", async () => {
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === "batch_update") {
        return Promise.resolve({
          started: ["codex"],
          failed: [{ toolId: "gemini", reason: "未找到命令" }],
        });
      }
      return Promise.resolve(undefined);
    });

    const addLog = vi.fn();
    const onActionFailure = vi.fn();
    const state = useToolActions(addLog, onActionFailure);

    state.openConfirm("batch_update");
    await state.confirmAction();

    expect(addLog).toHaveBeenCalledWith("批量更新已启动 1 个，1 个启动失败。", "warn");
    expect(onActionFailure).not.toHaveBeenCalled();
  });

  it("batch_update 全部启动失败时应上报失败", async () => {
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === "batch_update") {
        return Promise.resolve({
          started: [],
          failed: [{ toolId: "codex", reason: "锁失败" }],
        });
      }
      return Promise.resolve(undefined);
    });

    const addLog = vi.fn();
    const onActionFailure = vi.fn();
    const state = useToolActions(addLog, onActionFailure);

    state.openConfirm("batch_update");
    await state.confirmAction();

    expect(onActionFailure).toHaveBeenCalledWith("批量更新启动失败：codex: 锁失败");
    expect(addLog).not.toHaveBeenCalledWith(expect.stringContaining("批量更新已启动"), "warn");
  });

  it("Claude 不支持 PATH 修复时不应展示卸载清理选项", async () => {
    const addLog = vi.fn();
    const onActionFailure = vi.fn();
    const state = useToolActions(addLog, onActionFailure);
    const claude = state.tools.value.find((tool) => tool.id === "claude");
    if (claude) {
      claude.supportsPathFix = false;
      claude.shellConfigFile = "--";
    }

    state.openConfirm("uninstall", "claude");

    expect(state.optionVisible.value).toBe(false);
    expect(state.optionLabel.value).toBe("");
    expect(state.optionHint.value).toBe("");
  });

  it("Claude 不支持 PATH 修复时 fix_path 应直接失败且不调用后端", async () => {
    const addLog = vi.fn();
    const onActionFailure = vi.fn();
    const state = useToolActions(addLog, onActionFailure);
    const claude = state.tools.value.find((tool) => tool.id === "claude");
    if (claude) {
      claude.supportsPathFix = false;
    }

    state.openConfirm("fix_path", "claude");
    await state.confirmAction();

    expect(onActionFailure).toHaveBeenCalledWith("当前平台不支持 PATH 自动写入。", undefined);
    expect(invokeMock.mock.calls.filter(([cmd]) => cmd === "apply_path_fix")).toHaveLength(0);
  });

  it("start_action 失败时应清理 PATH 后处理挂起状态", async () => {
    const handlers = new Map<string, (event: { payload: unknown }) => void>();
    listenMock.mockImplementation(async (eventName: string, handler: (event: { payload: unknown }) => void) => {
      handlers.set(eventName, handler);
      return () => undefined;
    });
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === "start_action") {
        return Promise.reject(new Error("start failed"));
      }
      return Promise.resolve(undefined);
    });

    const addLog = vi.fn();
    const onActionFailure = vi.fn();
    const state = useToolActions(addLog, onActionFailure);
    await state.subscribeToolEvents();

    const claude = state.tools.value.find((tool) => tool.id === "claude");
    if (claude) {
      claude.supportsPathFix = true;
      claude.shellConfigFile = "~/.zshrc";
    }

    state.openConfirm("install", "claude");
    await state.confirmAction();

    expect(claude).toBeTruthy();

    handlers.get("tool-progress")?.({
      payload: {
        toolId: "claude",
        progress: 55,
        status: "installing",
      },
    });
    handlers.get("tool-updated")?.({
      payload: {
        tool: {
          ...claude!,
          status: "installed",
          progress: 100,
        },
      },
    });
    await Promise.resolve();
    await Promise.resolve();

    expect(onActionFailure).toHaveBeenCalled();
    expect(invokeMock.mock.calls.filter(([cmd]) => cmd === "apply_path_fix")).toHaveLength(0);
  });
});
