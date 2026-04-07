// useTaskProgress 派生状态测试：校验任务统计与弹层自动关闭逻辑。
import { nextTick, ref } from "vue";
import { describe, expect, it } from "vitest";
import { useTaskProgress } from "../src/composables/useTaskProgress";
import type { Tool } from "../src/types/models";

const createTool = (id: string, status: Tool["status"], progress = 0): Tool => ({
  id,
  name: `${id}-tool`,
  vendor: "vendor",
  vendorIcon: "/assets/vendor.svg",
  status,
  currentVersion: "v1.0.0",
  latestVersion: "v1.0.0",
  path: "/usr/local/bin/tool",
  configPath: "/tmp/config",
  pathNeedsSetup: false,
  shellConfigFile: "~/.zshrc",
  progress,
});

describe("useTaskProgress", () => {
  it("应计算任务汇总并在无任务时自动关闭任务弹层", async () => {
    const tools = ref<Tool[]>([
      createTool("a", "installing", 20),
      createTool("b", "installed", 100),
      createTool("c", "update_available", 100),
    ]);
    const isTaskOpen = ref(true);

    const state = useTaskProgress(tools, isTaskOpen);

    expect(state.hasTasks.value).toBe(true);
    expect(state.summary.value).toEqual({
      installed: 1,
      updateAvailable: 1,
      notInstalled: 0,
      inProgress: 1,
    });
    expect(state.taskChipLabel.value).toContain("1 个任务");

    tools.value = tools.value.map((tool) => ({ ...tool, status: "installed", progress: 100 }));
    await nextTick();

    expect(state.hasTasks.value).toBe(false);
    expect(isTaskOpen.value).toBe(false);
  });
});
